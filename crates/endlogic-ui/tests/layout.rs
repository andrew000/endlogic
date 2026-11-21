//! Layout at many screen sizes, and the drawing of line counters.

mod common;

use std::fmt::Write as _;

use common::*;
use endlogic_core::Color;
use endlogic_core::Puzzle;
use endlogic_core::code::encode;
use endlogic_core::format::parse;
use endlogic_core::generator::Difficulty;
use endlogic_ui::{
    ACROSS, App, Arrangement, BAND, COUNTER_GAP, CodeKind, Counter, CounterView, Event, Insets,
    Key, Lang, Layout, LayoutInput, Line, LineState, MIN_TARGET, MIN_TICK, Mode, Point,
    PointerKind, Rect, Rgba, TICK_LONG, TICK_PITCH, TICK_THICK, TRAY_MIN_SCALE, TRAY_SCALE,
    band_depth, line_counters,
};

// ---- layout ----

const SIZES: [(f32, f32); 10] = [
    (360.0, 640.0),
    (390.0, 844.0),
    (412.0, 915.0),
    (800.0, 895.0),
    (780.0, 300.0),
    (844.0, 390.0),
    (915.0, 412.0),
    (1366.0, 768.0),
    (1920.0, 1080.0),
    (2560.0, 1440.0),
];

/// Typical phone insets: a notch on top or on one side, a home indicator.
fn phone_insets(w: f32, h: f32) -> Insets {
    if w < h {
        Insets {
            top: 47.0,
            right: 0.0,
            bottom: 34.0,
            left: 0.0,
        }
    } else if h < 520.0 {
        Insets {
            top: 0.0,
            right: 47.0,
            bottom: 21.0,
            left: 47.0,
        }
    } else {
        Insets::default()
    }
}

fn expected_kind(w: f32, h: f32) -> Arrangement {
    if w >= h * 1.2 {
        if h < 520.0 {
            Arrangement::LowWide
        } else {
            Arrangement::Wide
        }
    } else {
        Arrangement::Tall
    }
}

fn assert_close(a: f32, b: f32, what: &str) {
    assert!((a - b).abs() < 0.5, "{what}: {a} != {b}");
}

fn check_layout(layout: &Layout, input: &LayoutInput, what: &str) {
    let (w, h) = (input.width, input.height);
    assert_eq!(layout.kind, expected_kind(w, h), "{what}: arrangement");
    let screen = Rect::new(0.0, 0.0, w, h);
    let i = input.insets;
    let safe = Rect::new(i.left, i.top, w - i.left - i.right, h - i.top - i.bottom);
    let rects = layout.static_rects();
    for (name, rect) in &rects {
        assert!(
            screen.contains_rect(*rect),
            "{what}: {name} {rect:?} off screen"
        );
        assert!(rect.w > 0.0 && rect.h > 0.0, "{what}: {name} empty");
    }
    for (k, (a, ra)) in rects.iter().enumerate() {
        for (b, rb) in &rects[k + 1..] {
            assert!(
                !ra.intersects(*rb),
                "{what}: {a} {ra:?} overlaps {b} {rb:?}"
            );
        }
    }
    // Touch zones: big enough, inside the safe area, never overlapping.
    let editing = layout.edit.is_some();
    let mut targets = layout.targets();
    if !editing {
        targets.push(("next", Layout::zone(layout.next)));
    }
    for (name, zone) in &targets {
        assert!(
            zone.w >= MIN_TARGET - 0.01 && zone.h >= MIN_TARGET - 0.01,
            "{what}: {name} {zone:?} too small to tap"
        );
        assert!(
            safe.contains_rect(*zone),
            "{what}: {name} {zone:?} in an inset"
        );
    }
    let playing = &targets[..targets.len() - usize::from(!editing)];
    for (k, (a, za)) in playing.iter().enumerate() {
        for (b, zb) in &playing[k + 1..] {
            assert!(
                !za.intersects(*zb),
                "{what}: zones {a} {za:?} and {b} {zb:?}"
            );
        }
    }
    if let Some(panel) = layout.panel {
        let mut inner = vec![layout.title, layout.puzzle_id];
        inner.extend(layout.subtitle.iter().map(|b| b.rect));
        for row in &layout.help {
            inner.push(row.icon);
            inner.push(row.text.rect);
        }
        for rect in inner {
            assert!(
                panel.contains_rect(rect),
                "{what}: {rect:?} outside the panel"
            );
        }
    }
    check_board(layout, input, what);
    check_tray(layout, input, what);
    if editing {
        check_edit(layout, input, safe, what);
        return;
    }
    check_banner(layout, &rects, what);
    match layout.kind {
        Arrangement::Wide => check_wide(layout, w, what),
        Arrangement::Tall => check_tall(layout, safe, what),
        Arrangement::LowWide => check_low_wide(layout, what),
    }
    assert!(layout.cell >= 12.0, "{what}: cell {}", layout.cell);
}

/// The editor: its areas on screen and clear of the board, and the
/// controls of every overlay apart from each other and from what stays
/// usable while it is open (the header and the tabs).
fn check_edit(layout: &Layout, input: &LayoutInput, safe: Rect, what: &str) {
    let e = layout.edit.unwrap();
    let block = layout.board_block();
    if let Some(panel) = layout.panel {
        for rect in [
            e.width.minus,
            e.width.plus,
            e.height.minus,
            e.height.plus,
            e.status,
        ] {
            assert!(
                panel.contains_rect(rect),
                "{what}: {rect:?} outside the panel"
            );
        }
    }
    for (name, area) in [("strip", e.strip), ("piece", e.piece), ("answer", e.result)] {
        assert!(
            safe.contains_rect(area),
            "{what}: {name} {area:?} off the safe area"
        );
        assert!(!area.intersects(block), "{what}: {name} over the board");
        for tab in layout.tabs {
            assert!(
                !area.intersects(Layout::zone(tab)),
                "{what}: {name} over a tab"
            );
        }
    }
    let header: Vec<Rect> = layout
        .targets()
        .into_iter()
        .filter(|(name, _)| {
            [
                "easy", "normal", "hard", "expert", "custom", "lang", "en", "ua", "share",
            ]
            .contains(name)
        })
        .map(|(_, zone)| zone)
        .collect();
    let apart = |zones: &[(String, Rect)], area: Rect, what: &str| {
        check_apart(zones, area, &header, safe, what);
    };
    // The digit strip for the longest line.
    let len = input.grid_width.max(input.grid_height);
    let keys: Vec<(String, Rect)> = e
        .digit_keys(len + 1)
        .into_iter()
        .enumerate()
        .map(|(k, r)| (format!("key {k}"), Layout::zone(r)))
        .collect();
    apart(&keys, e.strip, &format!("{what} strip"));
    check_overlays(layout, input, &apart, what);
    assert!(layout.cell >= 8.0, "{what}: cell {}", layout.cell);
    if input.grid_width.max(input.grid_height) <= 8 {
        assert!(layout.cell >= 12.0, "{what}: cell {}", layout.cell);
    }
}

/// The grid is on the axis, with a counter band on the left as deep as the
/// input asks ([`BAND`] when ticks would be too thin) and an equal
/// empty band on the right.
fn check_board(layout: &Layout, input: &LayoutInput, what: &str) {
    let c = layout.cell;
    assert!(layout.frame.contains_rect(layout.cells), "{what}: cells");
    assert_close(
        layout.cells.center().x,
        layout.axis,
        &format!("{what}: grid axis"),
    );
    let (rows, cols) = if layout.numbers_only {
        (BAND, BAND)
    } else {
        (input.row_band, input.col_band)
    };
    if layout.edit.is_none() && !layout.numbers_only {
        assert!(TICK_THICK * layout.cell >= MIN_TICK, "{what}: thin ticks");
    }
    assert_close(layout.row_counters.w, rows * c, what);
    assert_close(layout.col_counters.h, cols * c, what);
    assert_close(layout.right_band.w, layout.row_counters.w, what);
    let gap = COUNTER_GAP * c;
    assert_close(layout.row_counters.right(), layout.cells.x - gap, what);
    assert_close(layout.col_counters.bottom(), layout.cells.y - gap, what);
    assert_close(layout.right_band.x, layout.cells.right() + gap, what);
}

fn check_tray(layout: &Layout, input: &LayoutInput, what: &str) {
    assert_eq!(layout.cards.len(), layout.per_page, "{what}: cards");
    assert!(layout.per_page <= input.pieces.max(1), "{what}: page");
    assert_eq!(
        layout.pager.is_some(),
        layout.per_page < input.pieces,
        "{what}: pager"
    );
    if let Some(pager) = layout.pager {
        for rect in [pager.prev, pager.label, pager.next] {
            assert!(layout.tray.contains_rect(rect), "{what}: pager outside");
            for card in &layout.cards {
                assert!(!card.intersects(rect), "{what}: pager over a card");
            }
        }
    }
    let first = layout.cards[0];
    for (k, card) in layout.cards.iter().enumerate() {
        assert!(
            layout.tray.contains_rect(*card),
            "{what}: card outside the tray"
        );
        assert_close(card.w, first.w, what);
        assert_close(card.h, first.h, what);
        for other in &layout.cards[k + 1..] {
            assert!(!card.intersects(*other), "{what}: cards overlap");
        }
    }
    assert!(
        layout.tray_cell <= layout.cell * TRAY_SCALE + 0.01,
        "{what}: tray cell {} board cell {}",
        layout.tray_cell,
        layout.cell
    );
    let longest = f32::from(u8::try_from(input.max_piece_side).unwrap());
    assert!(
        layout.tray_cell * longest <= first.w,
        "{what}: piece wider than its card"
    );
}

fn check_banner(layout: &Layout, rects: &[(&str, Rect)], what: &str) {
    assert!(layout.safe.contains_rect(layout.banner), "{what}: banner");
    assert!(layout.banner.contains_rect(layout.next), "{what}: next");
    for (name, rect) in rects {
        let on_tray = *name == "tray" && layout.kind != Arrangement::Wide;
        if *name != "banner" && !on_tray {
            assert!(
                !layout.banner.intersects(*rect),
                "{what}: banner overlaps {name}"
            );
        }
    }
}

fn check_wide(layout: &Layout, w: f32, what: &str) {
    // Hint next to reset, bottom left.
    assert_close(layout.hint.y, layout.reset.y, what);
    assert!(
        layout.hint.x > layout.reset.right(),
        "{what}: hint after reset"
    );
    assert!(
        layout.hint.right() < layout.axis,
        "{what}: hint on the left"
    );
    assert!(
        layout.tray.w <= w * 0.24 + 0.01,
        "{what}: tray {} of {w}",
        layout.tray.w
    );
    // The colors switch right of the tabs, on their row.
    let colors = layout.colors.unwrap();
    assert!(
        colors.x > layout.tabs[4].right(),
        "{what}: colors after tabs"
    );
    assert_close(colors.y, layout.tabs[0].y, what);
    for (name, rect) in [
        ("tabs", layout.tab_bar()),
        ("counters", layout.counters),
        ("banner", layout.banner),
    ] {
        assert_close(
            rect.center().x,
            layout.axis,
            &format!("{what}: {name} axis"),
        );
    }
}

fn check_tall(layout: &Layout, safe: Rect, what: &str) {
    let header_top = layout.title.y.min(layout.lang.y);
    let header_bottom = layout
        .puzzle_id
        .bottom()
        .max(layout.lang.bottom())
        .max(layout.share.bottom());
    assert!(header_bottom - header_top <= 48.0, "{what}: header");
    assert!(layout.tabs.iter().all(|t| t.h <= 36.0), "{what}: tabs");
    // Header, from the left: the colors, the counter display, the
    // language, the link.
    assert!(layout.compact, "{what}: compact switches");
    let colors = layout.colors.unwrap();
    assert!(colors.right() < layout.counters.x, "{what}: header order");
    assert!(colors.h <= 30.0 && colors.y >= header_top, "{what}: header");
    assert!(
        layout.counters.right() < layout.lang.x,
        "{what}: header order"
    );
    assert!(layout.lang.right() < layout.share.x, "{what}: header order");
    for button in [layout.counters, layout.lang, layout.share] {
        assert!(
            button.h <= 30.0 && button.bottom() <= header_bottom,
            "{what}: header"
        );
    }
    // Bottom row: reset, hint, new.
    assert!(layout.reset.right() < layout.hint.x, "{what}: bottom order");
    assert!(layout.hint.right() < layout.new.x, "{what}: bottom order");
    for button in [layout.reset, layout.hint, layout.new] {
        assert!((38.0..=42.0).contains(&button.h), "{what}: bottom button");
        assert!(
            button.bottom() <= safe.bottom() - 12.0 + 0.01,
            "{what}: above inset"
        );
    }
    let content = safe.w - 32.0;
    assert_close(layout.tray.w, content, &format!("{what}: tray width"));
    assert!(
        layout.tray.w >= layout.cells.w,
        "{what}: tray narrower than grid"
    );
    assert!(
        layout.tray_cell >= layout.cell * TRAY_MIN_SCALE - 0.01,
        "{what}: tray cell {} below a third of {}",
        layout.tray_cell,
        layout.cell
    );
    assert!(
        layout.tray.y >= layout.board_block().bottom(),
        "{what}: tray under board"
    );
}

fn check_low_wide(layout: &Layout, what: &str) {
    // Hint in the left column, above reset when the column is tall enough.
    assert!(
        layout.hint.right() <= layout.board_block().x,
        "{what}: hint left"
    );
    if layout.safe.h >= 340.0 {
        assert!(
            layout.hint.bottom() <= layout.reset.y,
            "{what}: hint above reset"
        );
    }
    // No top or bottom row: every control is beside the board.
    let block = layout.board_block();
    let mut controls = vec![
        layout.colors.unwrap(),
        layout.lang,
        layout.share,
        layout.reset,
        layout.hint,
        layout.counters,
        layout.new,
        layout.tray,
    ];
    controls.extend(layout.tabs);
    for rect in controls {
        assert!(
            rect.right() <= block.x + 0.01 || rect.x >= block.right() - 0.01,
            "{what}: {rect:?} above or below the board"
        );
    }
    assert!(
        layout.panel.is_none() && layout.help.is_empty(),
        "{what}: help"
    );
}

fn input_for(w: f32, h: f32, difficulty: Difficulty, side: usize, lang: Lang) -> LayoutInput {
    let params = difficulty.params();
    LayoutInput {
        width: w,
        height: h,
        insets: Insets::default(),
        touch: false,
        lang,
        grid_width: params.width,
        grid_height: params.height,
        pieces: params.pieces.max,
        max_piece_side: side,
        mode: Mode::Play,
        row_band: band_depth(params.width),
        col_band: band_depth(params.height),
    }
}

#[test]
fn layout_fits_every_size_and_difficulty() {
    for (w, h) in SIZES {
        for difficulty in Difficulty::ALL {
            for lang in [Lang::En, Lang::Ua] {
                for touch in [false, true] {
                    for insets in [Insets::default(), phone_insets(w, h)] {
                        for side in [3, 4, 5] {
                            let input = LayoutInput {
                                touch,
                                insets,
                                ..input_for(w, h, difficulty, side, lang)
                            };
                            let what = format!(
                                "{w}x{h} {difficulty:?} {lang:?} touch={touch} side={side} {insets:?}"
                            );
                            let layout = Layout::compute(&input);
                            check_layout(&layout, &input, &what);
                            if touch || layout.kind != Arrangement::Wide {
                                assert!(layout.help.is_empty(), "{what}: help");
                            }
                            if !touch && layout.kind == Arrangement::Wide && h >= 700.0 {
                                assert_eq!(layout.help.len(), 3, "{what}: no help");
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn phones_on_their_side_get_a_big_board() {
    for (w, h, least) in [(844.0, 390.0, 40.0), (780.0, 300.0, 30.0)] {
        for insets in [Insets::default(), phone_insets(w, h)] {
            let input = LayoutInput {
                insets,
                ..input_for(w, h, Difficulty::Normal, 4, Lang::Ua)
            };
            let layout = Layout::compute(&input);
            assert_eq!(layout.kind, Arrangement::LowWide);
            assert!(
                layout.cell >= least,
                "{w}x{h} {insets:?}: cell {}",
                layout.cell
            );
        }
    }
}

#[test]
fn a_big_screen_gives_easy_a_big_board() {
    let layout = Layout::compute(&input_for(1920.0, 1080.0, Difficulty::Easy, 3, Lang::En));
    assert!(layout.cell > 64.0, "cell {}", layout.cell);
}

#[test]
fn help_icons_share_an_axis_and_center_on_their_text() {
    for (w, h) in [(1366.0, 768.0), (1920.0, 1080.0), (2560.0, 1440.0)] {
        for lang in [Lang::En, Lang::Ua] {
            let layout = Layout::compute(&input_for(w, h, Difficulty::Normal, 4, lang));
            let what = format!("{w}x{h} {lang:?}");
            assert_eq!(layout.help.len(), 3, "{what}");
            let axis = layout.help[0].icon.center().x;
            let mut gaps = Vec::new();
            for (k, row) in layout.help.iter().enumerate() {
                assert_close(row.icon.center().x, axis, &what);
                assert_close(row.icon.w, layout.help[0].icon.w, &what);
                assert_close(row.icon.center().y, row.text.rect.center().y, &what);
                assert_ne!(row.text.lines, Vec::<String>::new());
                if let Some(next) = layout.help.get(k + 1) {
                    let bottom = row.text.rect.bottom().max(row.icon.bottom());
                    let top = next.text.rect.y.min(next.icon.y);
                    gaps.push(top - bottom);
                }
            }
            assert_close(gaps[0], gaps[1], &what);
            let subtitle = layout.subtitle.as_ref().unwrap();
            let first = &layout.help[0];
            let above = first.text.rect.y.min(first.icon.y) - subtitle.rect.bottom();
            assert!(
                above <= gaps[0] * 1.5 + 0.5,
                "{what}: {above} vs {}",
                gaps[0]
            );
            let panel = layout.panel.unwrap();
            let last = &layout.help[2];
            let bottom = last.text.rect.bottom().max(last.icon.bottom());
            assert!(
                panel.bottom() - bottom < layout.font * 1.5,
                "{what}: panel too tall"
            );
        }
    }
}

/// A 16×16 board with clues from 1 to 16 and eight 4×4 pieces.
fn sixteen() -> Puzzle {
    let mut text = String::from("endlogic 1\ngrid\n");
    for _ in 0..16 {
        text.push_str(&".".repeat(16));
        text.push('\n');
    }
    let clues: Vec<String> = (1..=16).rev().map(|n: u32| n.to_string()).collect();
    let clues = clues.join(" ");
    writeln!(text, "rows {clues}\ncols {clues}").unwrap();
    for _ in 0..8 {
        text.push_str("piece\n####\n####\n####\n####\n");
    }
    parse(&text).unwrap()
}

#[test]
fn a_sixteen_by_sixteen_puzzle_fits() {
    for (w, h) in [(390.0, 844.0), (1920.0, 1080.0)] {
        let mut config = config(w, h);
        config.code = Some((CodeKind::Play, encode(&sixteen()).unwrap()));
        config.touch = w < h;
        let mut app = App::new(config);
        assert!(!app.is_generating());
        let input = app.layout_input();
        check_layout(app.layout(), &input, &format!("16x16 at {w}x{h}"));
        let drawing = settle(&mut app);
        let screen = Rect::new(-1.0, -1.0, w + 2.0, h + 2.0);
        for call in &drawing.calls {
            if let Call::FillRect(r, _) | Call::StrokeRect(r, _) = call {
                assert!(screen.contains_rect(*r), "{w}x{h}: {call:?}");
            }
        }
    }
}

#[test]
fn layout_of_real_puzzles_fits() {
    for (w, h) in SIZES {
        for (k, difficulty) in Difficulty::ALL.into_iter().enumerate() {
            let mut config = config(w, h);
            config.touch = w < h || h < 520.0;
            config.insets = phone_insets(w, h);
            config.entropy = u64::try_from(k).unwrap();
            config.settings = vec![("difficulty".to_owned(), difficulty.name().to_owned())];
            let mut app = generated(config);
            let input = app.layout_input();
            check_layout(app.layout(), &input, &format!("{w}x{h} {difficulty:?}"));
            // Everything drawn stays on screen.
            let drawing = settle(&mut app);
            let screen = Rect::new(-1.0, -1.0, w + 2.0, h + 2.0);
            for call in &drawing.calls {
                if let Call::FillRect(r, _) | Call::StrokeRect(r, _) = call {
                    assert!(screen.contains_rect(*r), "{w}x{h}: {call:?}");
                }
            }
        }
    }
}

#[test]
fn any_window_size_draws_without_panicking() {
    let puzzle = sample("wln006-2.txt");
    let mut app = open(&puzzle);
    let mut sizes = vec![(800.0, 600.0), (1.0, 1.0), (200.0, 120.0), (3840.0, 2160.0)];
    for w in (240..=2600).step_by(173) {
        for h in (200..=1600).step_by(139) {
            sizes.push((
                f32::from(u16::try_from(w).unwrap()),
                f32::from(u16::try_from(h).unwrap()),
            ));
        }
    }
    for (w, h) in sizes {
        app.handle(Event::Resize {
            width: w,
            height: h,
            insets: Insets::default(),
        });
        settle(&mut app);
        let from = card(&app, 0);
        app.start_drag(from, cell(&app, 1, 1), PointerKind::Touch);
        app.frame(0.016, &mut Recorder::default());
        app.handle(Event::Key(Key::Escape));
        app.handle(Event::PointerUp(pointer(FINGER, from, PointerKind::Touch)));
        settle(&mut app);
    }
    // Ordinary desktop windows still get a usable board.
    for (w, h) in [(800.0, 600.0), (1024.0, 768.0), (1280.0, 720.0)] {
        app.handle(Event::Resize {
            width: w,
            height: h,
            insets: Insets::default(),
        });
        assert!(
            app.layout().cell >= 24.0,
            "{w}x{h}: cell {}",
            app.layout().cell
        );
    }
}

#[test]
fn a_piece_grows_when_taken_from_the_tray_and_shrinks_back() {
    let mut app = open(&sample("wlh9000-1.txt"));
    let tray_cell = app.layout().tray_cell;
    let board_cell = app.layout().cell;
    assert!(tray_cell <= board_cell * 0.5);
    assert_eq!(app.piece_visual(0).1, tray_cell);
    let from = card(&app, 0);
    let id = app.start_drag(from, Point::new(from.x - 30.0, from.y), PointerKind::Mouse);
    assert_eq!(
        app.piece_visual(0).1,
        tray_cell,
        "growth starts at tray size"
    );
    app.frame(0.04, &mut Recorder::default());
    let middle = app.piece_visual(0).1;
    assert!(tray_cell < middle && middle < board_cell, "{middle}");
    app.frame(0.05, &mut Recorder::default());
    assert_eq!(app.piece_visual(0).1, board_cell);
    // Dropped off the board it shrinks back on its way to the tray.
    app.handle(Event::PointerUp(pointer(
        id,
        Point::new(2.0, 2.0),
        PointerKind::Mouse,
    )));
    app.frame(0.09, &mut Recorder::default());
    let shrinking = app.piece_visual(0).1;
    assert!(
        tray_cell < shrinking && shrinking < board_cell,
        "{shrinking}"
    );
    settle(&mut app);
    assert_eq!(app.piece_visual(0).1, tray_cell);
}

// ---- counters ----

/// The band part beside row 0 of a grid at (100, 100) with cells of `cell`,
/// for counters of up to `ticks` ticks.
fn row_area(cell: f32, ticks: usize) -> Rect {
    let band = band_depth(ticks) * cell;
    Rect::new(100.0 - COUNTER_GAP * cell - band, 100.0, band, cell)
}

/// The band part above column 0 of the same grid.
fn col_area(cell: f32, ticks: usize) -> Rect {
    let band = band_depth(ticks) * cell;
    Rect::new(100.0, 100.0 - COUNTER_GAP * cell - band, cell, band)
}

fn line(filled: usize, clue: usize) -> Line {
    Line { filled, clue }
}

fn ticks(counter: &Counter) -> Vec<Rect> {
    match counter.view {
        CounterView::Ticks { .. } => counter.view.marks().collect(),
        other @ CounterView::Empty { .. } => panic!("expected ticks, got {other:?}"),
    }
}

/// The colors `a` to `d` with these lines.
fn colored(lines: &[Line]) -> Vec<(Color, Line)> {
    lines
        .iter()
        .enumerate()
        .map(|(k, &l)| (Color(u8::try_from(k).unwrap()), l))
        .collect()
}

#[test]
fn a_counter_is_one_tick_per_cell_of_a_fixed_size() {
    let cell = 60.0;
    for row in [true, false] {
        for n in 1..=16 {
            let area = if row {
                row_area(cell, n)
            } else {
                col_area(cell, n)
            };
            let counters = line_counters(&colored(&[line(0, n)]), area, row, cell);
            assert_eq!(counters.len(), 1);
            let marks = ticks(&counters[0]);
            assert_eq!(marks.len(), n, "clue {n}");
            for m in &marks {
                assert!(
                    area.contains_rect(*m),
                    "clue {n} row={row}: {m:?} in {area:?}"
                );
                // Vertical ticks for a row, horizontal for a column, the
                // same size whatever the clue.
                let (thick, long) = if row { (m.w, m.h) } else { (m.h, m.w) };
                assert!((thick - TICK_THICK * cell).abs() < 0.01, "clue {n}");
                assert!((long - TICK_LONG * cell).abs() < 0.01, "clue {n}");
            }
            // One line of ticks, nearest to the grid first, a pitch apart.
            let along = |m: &Rect| if row { m.center().x } else { m.center().y };
            let across = |m: &Rect| if row { m.center().y } else { m.center().x };
            assert!(
                marks
                    .iter()
                    .all(|m| (across(m) - across(&marks[0])).abs() < 0.01)
            );
            for pair in marks.windows(2) {
                let step = along(&pair[0]) - along(&pair[1]);
                assert!((step - TICK_PITCH * cell).abs() < 0.01, "clue {n}: {step}");
            }
            // The first tick starts at the band's edge next to the grid.
            let edge = if row { area.right() } else { area.bottom() };
            let first = if row {
                marks[0].right()
            } else {
                marks[0].bottom()
            };
            assert!(edge - first < TICK_PITCH * cell, "clue {n}");
        }
    }
}

#[test]
fn a_line_of_zero_clues_shows_one_grey_sign() {
    let cell = 50.0;
    for row in [true, false] {
        let area = if row {
            row_area(cell, 0)
        } else {
            col_area(cell, 0)
        };
        for k in 1..=4 {
            let lines = colored(&vec![line(0, 0); k]);
            let counters = line_counters(&lines, area, row, cell);
            assert_eq!(counters.len(), 1, "{k} colors");
            match counters[0].view {
                CounterView::Empty { center, radius } => {
                    let sign = Rect::centered(center, 2.0 * radius, 2.0 * radius);
                    assert!(area.contains_rect(sign), "{sign:?} in {area:?}");
                }
                ref other @ CounterView::Ticks { .. } => panic!("{other:?}"),
            }
        }
    }
}

/// With one to four colors in a line the counters split it evenly across:
/// rows stack the first color at the bottom, columns put it on the left.
#[test]
fn colors_share_a_line_evenly() {
    let cell = 60.0;
    for row in [true, false] {
        for k in 1..=4 {
            let lines = colored(&(1..=k).map(|n| line(0, n + 1)).collect::<Vec<_>>());
            let area = if row {
                row_area(cell, k + 1)
            } else {
                col_area(cell, k + 1)
            };
            let counters = line_counters(&lines, area, row, cell);
            assert_eq!(counters.len(), k);
            let across = |r: &Rect| if row { r.center().y } else { r.center().x };
            let slot = ACROSS * cell / to_f32(k);
            for (j, counter) in counters.iter().enumerate() {
                assert_eq!(counter.color, Color(u8::try_from(j).unwrap()));
                assert_eq!(ticks(counter).len(), j + 2);
                let width = if row { counter.slot.h } else { counter.slot.w };
                assert!((width - slot).abs() < 0.01, "{k} colors");
                assert!(area.contains_rect(counter.slot), "{k} colors: slot");
                for m in &ticks(counter) {
                    assert!(counter.slot.contains_rect(*m), "{k} colors: tick in slot");
                    let long = if row { m.h } else { m.w };
                    assert!(long <= TICK_LONG * cell + 0.01 && long <= slot, "{k}");
                }
            }
            // In order across the line, centered on it.
            for pair in counters.windows(2) {
                let (a, b) = (across(&pair[0].slot), across(&pair[1].slot));
                if row {
                    assert!(a > b, "rows: the first color at the bottom");
                } else {
                    assert!(a < b, "columns: the first color on the left");
                }
            }
            let middle: f32 = counters.iter().map(|c| across(&c.slot)).sum::<f32>() / to_f32(k);
            assert!((middle - across(&area)).abs() < 0.01, "{k} colors centered");
        }
    }
    // Ticks get shorter only when the slot is narrower than a whole tick.
    let two = line_counters(
        &colored(&[line(0, 2), line(0, 2)]),
        row_area(cell, 2),
        true,
        cell,
    );
    let four = line_counters(&colored(&[line(0, 2); 4]), row_area(cell, 2), true, cell);
    assert!(ticks(&four[0])[0].h < ticks(&two[0])[0].h);
}

#[test]
fn a_zero_clue_is_left_out_or_shows_a_red_sign() {
    let cell = 60.0;
    let area = row_area(cell, 3);
    // Teal 3, orange 0: orange is not drawn, teal has the line alone.
    let counters = line_counters(&colored(&[line(0, 3), line(0, 0)]), area, true, cell);
    assert_eq!(counters.len(), 1);
    assert_eq!(counters[0].color, Color(0));
    assert!((counters[0].slot.h - ACROSS * cell).abs() < 0.01);
    // Orange 0 with an orange cell in the line: a sign in orange's place.
    let counters = line_counters(&colored(&[line(0, 3), line(1, 0)]), area, true, cell);
    assert_eq!(counters.len(), 2);
    assert!(matches!(counters[1].view, CounterView::Empty { .. }));
    assert_eq!(counters[1].color, Color(1));
    // Teal 0, orange 2: orange alone.
    let counters = line_counters(&colored(&[line(0, 0), line(0, 2)]), area, false, cell);
    assert_eq!(counters.len(), 1);
    assert_eq!(counters[0].color, Color(1));
}

fn to_f32(n: usize) -> f32 {
    f32::from(u16::try_from(n).unwrap())
}

/// An 8×2 board whose rows ask for 6 and 7 cells: long counters.
fn long_rows() -> Puzzle {
    parse("endlogic 1\ngrid\n........\n........\nrows 6 7\ncols 2 2 2 2 2 2 1 0\npiece\n#######\npiece\n######\n").unwrap()
}

fn fills(drawing: &Recorder, color: Rgba) -> usize {
    drawing
        .calls
        .iter()
        .filter(|c| matches!(c, Call::FillRect(_, k) if *k == color))
        .count()
}

#[test]
fn counters_show_the_three_states() {
    let mut app = open(&long_rows());
    let accent = Rgba::hex(0x19_E3C0);
    let outlines = |d: &Recorder| {
        d.calls
            .iter()
            .filter(|c| matches!(c, Call::StrokeRect(_, k) if *k == accent))
            .count()
    };
    // Nothing placed: 6 + 7 outlined ticks in the row band.
    let drawing = settle(&mut app);
    assert!(outlines(&drawing) >= 13);
    assert_eq!(fills(&drawing, ERROR), 0);
    // Seven cells in the row that wants six: all six ticks red.
    app.place(0, 0, 0, 0, PointerKind::Mouse);
    assert_eq!(app.board().unwrap().rows()[0].state(), LineState::Over);
    let drawing = settle(&mut app);
    assert!(fills(&drawing, ERROR) >= 6, "{}", fills(&drawing, ERROR));
    // In the row that wants seven: all seven bright.
    let press = cell(&app, 0, 3);
    let to = release_point(&app, 0, press, 1, 0, PointerKind::Mouse);
    app.drag(press, to, PointerKind::Mouse);
    assert_eq!(at(&app, 0), Some((1, 0)));
    let drawing = settle(&mut app);
    assert!(fills(&drawing, BRIGHT) >= 7);
    assert_eq!(fills(&drawing, ERROR), 0);
}

/// The bands are as deep as the longest counter of the puzzle.
#[test]
fn bands_follow_the_longest_counter() {
    let mut app = open(&long_rows());
    settle(&mut app);
    let l = app.layout().clone();
    assert!((l.row_counters.w - band_depth(7) * l.cell).abs() < 0.5);
    assert!((l.col_counters.h - band_depth(2) * l.cell).abs() < 0.5);
    assert!((l.right_band.w - l.row_counters.w).abs() < 0.5);
    assert!(
        (l.cells.center().x - l.axis).abs() < 0.5,
        "grid on the axis"
    );
    // Every tick inside its band.
    let drawing = settle(&mut app);
    let accent = Rgba::hex(0x19_E3C0);
    let mut in_rows = 0;
    for call in &drawing.calls {
        if let Call::StrokeRect(r, k) = call
            && *k == accent
            && l.row_counters.contains_rect(*r)
        {
            in_rows += 1;
        }
    }
    assert_eq!(in_rows, 13);
}

/// Cells too small for ticks of 1.5 px: the whole puzzle shows numbers.
#[test]
fn ticks_too_thin_become_numbers() {
    let mut app = open(&dominoes());
    // 16 cells across a 390 px screen: cells of about 20 px.
    app.handle(Event::Resize {
        width: 390.0,
        height: 844.0,
        insets: Insets::default(),
    });
    settle(&mut app);
    let l = app.layout().clone();
    assert!(l.numbers_only, "cell {}", l.cell);
    assert!((l.row_counters.w - BAND * l.cell).abs() < 0.5);
    let texts = app.drawn_texts();
    assert!(texts.iter().any(|t| t == "4" || t == "0/4"), "{texts:?}");
    // The four levels keep their ticks at every size checked.
    for (w, h) in SIZES {
        for difficulty in Difficulty::ALL {
            let layout = Layout::compute(&input_for(w, h, difficulty, 5, Lang::Ua));
            assert!(!layout.numbers_only, "{w}x{h} {difficulty:?}");
        }
    }
}

/// A 16×16 board with 32 dominoes.
fn dominoes() -> Puzzle {
    let mut text = String::from("endlogic 1\ngrid\n");
    for _ in 0..16 {
        text.push_str(&".".repeat(16));
        text.push('\n');
    }
    let clues = vec!["4"; 16].join(" ");
    writeln!(text, "rows {clues}\ncols {clues}").unwrap();
    for _ in 0..32 {
        text.push_str("piece\n##\n");
    }
    parse(&text).unwrap()
}

#[test]
fn many_pieces_split_the_tray_into_pages() {
    for (w, h) in SIZES {
        let mut config = config(w, h);
        config.code = Some((CodeKind::Play, encode(&dominoes()).unwrap()));
        config.touch = w < h || h < 520.0;
        let mut app = App::new(config);
        settle(&mut app);
        let layout = app.layout().clone();
        let what = format!("{w}x{h}");
        let input = app.layout_input();
        check_layout(&layout, &input, &what);
        let pages = app.tray_pages();
        if pages == 1 {
            // Big screens hold all 32 cards.
            assert!(w >= 1366.0, "{what}: one page");
            continue;
        }
        assert!(layout.pager.is_some(), "{what}: no pager");
        // Every piece shows on exactly one page, and paging cycles.
        let mut seen = vec![0; 32];
        for page in 0..pages {
            assert_eq!(app.tray_page(), page, "{what}");
            for (i, n) in seen.iter_mut().enumerate() {
                if app.card_of(i).is_some() {
                    *n += 1;
                }
            }
            app.click(layout.pager.unwrap().next.center());
        }
        assert_eq!(app.tray_page(), 0, "{what}: wraps around");
        assert!(seen.iter().all(|&n| n == 1), "{what}: {seen:?}");
        app.click(layout.pager.unwrap().prev.center());
        assert_eq!(app.tray_page(), pages - 1, "{what}: back wraps");
    }
    // At 390×844 the tray of the tall arrangement shows big cards.
    let mut config = config(390.0, 844.0);
    config.code = Some((CodeKind::Play, encode(&dominoes()).unwrap()));
    config.touch = true;
    let mut app = App::new(config);
    settle(&mut app);
    assert!(app.layout().cards[0].w >= MIN_TARGET);
    // A piece from the last page can be placed, and comes back to its page.
    app.click(app.layout().pager.unwrap().prev.center());
    let last = 31;
    let from = card(&app, last);
    let to = release_point(&app, last, from, 0, 0, PointerKind::Touch);
    app.drag(from, to, PointerKind::Touch);
    assert_eq!(app.board().unwrap().piece(last).at, Some((0, 0)));
    app.click(app.layout().pager.unwrap().next.center());
    assert_eq!(app.tray_page(), 0);
    app.drag(cell(&app, 0, 0), Point::new(3.0, 3.0), PointerKind::Touch);
    assert_eq!(app.board().unwrap().piece(last).at, None);
    assert_eq!(
        app.tray_page(),
        app.tray_pages() - 1,
        "shows the returned piece"
    );
}

#[test]
fn the_four_levels_need_no_tray_pages() {
    for (w, h) in SIZES {
        for difficulty in Difficulty::ALL {
            for side in [3, 4, 5] {
                let layout = Layout::compute(&input_for(w, h, difficulty, side, Lang::Ua));
                assert!(layout.pager.is_none(), "{w}x{h} {difficulty:?} {side}");
            }
        }
    }
}

#[test]
fn the_editor_fits_every_size() {
    let boards = [(5, 5, 1, 1), (7, 7, 6, 4), (8, 6, 9, 6), (16, 16, 33, 3)];
    for (w, h) in SIZES {
        for (gw, gh, cards, side) in boards {
            for lang in [Lang::En, Lang::Ua] {
                for insets in [Insets::default(), phone_insets(w, h)] {
                    let input = LayoutInput {
                        width: w,
                        height: h,
                        insets,
                        touch: true,
                        lang,
                        grid_width: gw,
                        grid_height: gh,
                        pieces: cards,
                        max_piece_side: side,
                        mode: Mode::Edit,
                        row_band: BAND,
                        col_band: BAND,
                    };
                    let what = format!("edit {w}x{h} {gw}x{gh} {cards} cards {lang:?} {insets:?}");
                    let layout = Layout::compute(&input);
                    check_layout(&layout, &input, &what);
                }
            }
        }
    }
}

/// Zones that must be big enough, inside the safe area and `area`, apart
/// from each other and from the header.
fn check_apart(zones: &[(String, Rect)], area: Rect, header: &[Rect], safe: Rect, what: &str) {
    for (k, (a, za)) in zones.iter().enumerate() {
        assert!(
            za.w >= MIN_TARGET - 0.01 && za.h >= MIN_TARGET - 0.01,
            "{what}: {a} small"
        );
        assert!(safe.contains_rect(*za), "{what}: {a} {za:?} in an inset");
        assert!(
            area.inset(-4.0).contains_rect(*za),
            "{what}: {a} {za:?} outside {area:?}"
        );
        for (b, zb) in &zones[k + 1..] {
            assert!(
                !za.intersects(*zb),
                "{what}: zones {a} {za:?} and {b} {zb:?}"
            );
        }
        for zh in header {
            assert!(!za.intersects(*zh), "{what}: {a} over the header");
        }
    }
}

/// Checks a group of touch zones inside an area.
type Apart<'a> = dyn Fn(&[(String, Rect)], Rect, &str) + 'a;

/// The piece editor and the solver's answer.
fn check_overlays(layout: &Layout, input: &LayoutInput, apart: &Apart, what: &str) {
    let e = layout.edit.unwrap();
    // The piece editor, at its usual size and its largest.
    for side in [6, 16] {
        let parts = e.piece_editor(side);
        let what = format!("{what} piece editor {side}");
        assert!(e.piece.contains_rect(parts.grid), "{what}: grid");
        assert!(parts.grid.w >= 60.0, "{what}: grid {}", parts.grid.w);
        assert!(
            !parts.grid.intersects(parts.reason),
            "{what}: reason over the grid"
        );
        let buttons: Vec<(String, Rect)> = [
            ("done", parts.done),
            ("cancel", parts.cancel),
            ("duplicate", parts.duplicate),
            ("delete", parts.delete),
            ("teal", parts.colors[0]),
            ("orange", parts.colors[1]),
            ("violet", parts.colors[2]),
            ("lime", parts.colors[3]),
        ]
        .into_iter()
        .map(|(n, r)| {
            assert!(!r.intersects(parts.grid), "{what}: {n} over the grid");
            assert!(
                !Layout::zone(r).intersects(parts.grid),
                "{what}: {n} zone over the grid"
            );
            (n.to_owned(), Layout::zone(r))
        })
        .collect();
        apart(&buttons, e.piece, &what);
    }
    // The solver's answer.
    let panel = e.result_panel(input.lang, layout.font);
    let buttons: Vec<(String, Rect)> = [
        ("prev", panel.prev),
        ("next", panel.next),
        ("action", panel.action),
    ]
    .into_iter()
    .map(|(n, r)| (n.to_owned(), Layout::zone(r)))
    .collect();
    apart(&buttons, e.result, &format!("{what} answer"));
    for text in [panel.title, panel.detail] {
        assert!(e.result.contains_rect(text), "{what}: answer text");
        for (_, zone) in &buttons {
            assert!(
                !text.intersects(*zone),
                "{what}: answer text under a button"
            );
        }
    }
}

#[test]
fn counts_too_wide_for_their_band_show_the_clue_alone() {
    // A 16x16 board on a phone: "0/16" does not fit a 15-pixel band even at
    // the smallest size, so the band shows "16"; a larger screen keeps
    // "a/b" where it fits.
    let mut config = config(390.0, 844.0);
    config.code = Some((CodeKind::Play, encode(&sixteen()).unwrap()));
    config.touch = true;
    config.settings = vec![("counters".to_owned(), "numbers".to_owned())];
    let mut app = App::new(config);
    let phone = texts(&settle(&mut app));
    assert!(phone.contains(&"16".to_owned()), "{phone:?}");
    assert!(!phone.iter().any(|t| t.ends_with("/16")), "{phone:?}");
    let mut config = common::config(1920.0, 1080.0);
    config.code = Some((CodeKind::Play, encode(&sixteen()).unwrap()));
    config.settings = vec![("counters".to_owned(), "numbers".to_owned())];
    let mut app = App::new(config);
    let wide = texts(&settle(&mut app));
    assert!(wide.contains(&"0/9".to_owned()), "{wide:?}");
}
