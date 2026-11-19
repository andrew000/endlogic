//! Layout at many screen sizes, and the drawing of line counters.

mod common;

use std::fmt::Write as _;

use common::*;
use endlogic_core::Puzzle;
use endlogic_core::code::encode;
use endlogic_core::format::parse;
use endlogic_core::generator::Difficulty;
use endlogic_ui::{
    App, Arrangement, BAND, COUNTER_GAP, CounterView, Event, Insets, Key, Lang, Layout,
    LayoutInput, Line, LineState, MIN_SQUARE, MIN_TARGET, Point, PointerKind, Rect, Rgba,
    TRAY_MIN_SCALE, TRAY_SCALE, counter_view,
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
    let mut targets = layout.targets();
    targets.push(("next", Layout::zone(layout.next)));
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
    let playing = &targets[..targets.len() - 1];
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
    check_board(layout, what);
    check_tray(layout, input, what);
    check_banner(layout, &rects, what);
    match layout.kind {
        Arrangement::Wide => check_wide(layout, w, what),
        Arrangement::Tall => check_tall(layout, safe, what),
        Arrangement::LowWide => check_low_wide(layout, what),
    }
    assert!(layout.cell >= 12.0, "{what}: cell {}", layout.cell);
}

/// The grid is on the axis, with a counter band on the left and an equal
/// empty band on the right.
fn check_board(layout: &Layout, what: &str) {
    let c = layout.cell;
    assert!(layout.frame.contains_rect(layout.cells), "{what}: cells");
    assert_close(
        layout.cells.center().x,
        layout.axis,
        &format!("{what}: grid axis"),
    );
    assert_close(layout.row_counters.w, BAND * c, what);
    assert_close(layout.col_counters.h, BAND * c, what);
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
    // Header, from the left: the counter display, the language, the link.
    assert!(layout.compact, "{what}: compact switches");
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
        config.code = Some(encode(&sixteen()).unwrap());
        config.touch = w < h;
        let mut app = App::new(config);
        assert!(!app.is_generating());
        let board = app.board().unwrap();
        let input = LayoutInput {
            width: w,
            height: h,
            insets: Insets::default(),
            touch: w < h,
            lang: Lang::En,
            grid_width: board.width(),
            grid_height: board.height(),
            pieces: board.piece_count(),
            max_piece_side: board.max_piece_side(),
        };
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
            let board = app.board().unwrap();
            let input = LayoutInput {
                width: w,
                height: h,
                insets: phone_insets(w, h),
                touch: w < h || h < 520.0,
                lang: Lang::En,
                grid_width: board.width(),
                grid_height: board.height(),
                pieces: board.piece_count(),
                max_piece_side: board.max_piece_side(),
            };
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

/// The band part beside row 0 of a grid at (100, 100) with cells of `cell`.
fn row_area(cell: f32) -> Rect {
    let band = BAND * cell;
    Rect::new(100.0 - COUNTER_GAP * cell - band, 100.0, band, cell)
}

/// The band part above column 0 of the same grid.
fn col_area(cell: f32) -> Rect {
    let band = BAND * cell;
    Rect::new(100.0, 100.0 - COUNTER_GAP * cell - band, cell, band)
}

fn marks(view: CounterView) -> (Vec<Rect>, bool) {
    match view {
        CounterView::Marks { marks, ticks } => (marks, ticks),
        other => panic!("expected marks, got {other:?}"),
    }
}

fn line(filled: usize, clue: usize) -> Line {
    Line { filled, clue }
}

/// Distinct values, rounded to half a pixel.
fn distinct(values: impl Iterator<Item = f32>) -> usize {
    // Screen coordinates, far inside `i64`.
    #[allow(clippy::cast_possible_truncation)]
    let mut v: Vec<i64> = values.map(|x| (x * 2.0).round() as i64).collect();
    v.sort_unstable();
    v.dedup();
    v.len()
}

#[test]
fn counter_shapes_follow_the_clue() {
    let cell = 60.0;
    for row in [true, false] {
        let area = if row { row_area(cell) } else { col_area(cell) };
        match counter_view(line(0, 0), area, row, cell, 0) {
            CounterView::Empty { center, radius } => {
                assert!(area.contains_rect(Rect::centered(center, 2.0 * radius, 2.0 * radius)));
            }
            other => panic!("zero clue: {other:?}"),
        }
        for n in 1..=16 {
            let (marks, ticks) = marks(counter_view(line(0, n), area, row, cell, n));
            assert_eq!(marks.len(), n, "clue {n}");
            assert_eq!(ticks, n <= 5, "clue {n}");
            for m in &marks {
                assert!(
                    area.contains_rect(*m),
                    "clue {n} row={row}: {m:?} outside {area:?}"
                );
            }
            // Distance from the grid, and position across the band.
            let along = |m: &Rect| if row { -m.center().x } else { -m.center().y };
            let across = |m: &Rect| if row { m.center().y } else { m.center().x };
            let lines = distinct(marks.iter().map(across));
            let groups = distinct(marks.iter().map(along));
            match n {
                1..=5 => assert_eq!((lines, groups), (1, n), "clue {n}"),
                6..=10 => assert_eq!((lines, groups), (2, n.div_ceil(2)), "clue {n}"),
                _ => assert_eq!((lines, groups), (3, n.div_ceil(3)), "clue {n}"),
            }
            // Marks come nearest-first; inside a group top to bottom (rows)
            // or left to right (columns).
            for pair in marks.windows(2) {
                let (a, b) = (&pair[0], &pair[1]);
                let same_group = (along(a) - along(b)).abs() < 0.5;
                if same_group {
                    assert!(across(a) < across(b), "clue {n}: order inside a group");
                } else {
                    assert!(along(a) < along(b), "clue {n}: groups grow outwards");
                }
            }
            // A remainder sits in the farthest group.
            if n > 5 {
                let size = if n <= 10 { 2 } else { 3 };
                let farthest = marks.iter().map(along).fold(f32::MIN, f32::max);
                let last = marks
                    .iter()
                    .filter(|m| (along(m) - farthest).abs() < 0.5)
                    .count();
                let expected = if n % size == 0 { size } else { n % size };
                assert_eq!(last, expected, "clue {n}: remainder");
            }
        }
    }
}

#[test]
fn squares_too_small_become_a_number() {
    // At a 20 px cell sixteen squares would be under 3 px.
    let cell = 20.0;
    assert_eq!(
        counter_view(line(0, 16), row_area(cell), true, cell, 16),
        CounterView::Number
    );
    // Small clues still get marks, and every mark is at least 3 px.
    for n in 1..=16 {
        if let CounterView::Marks {
            marks,
            ticks: false,
        } = counter_view(line(0, n), row_area(cell), true, cell, n)
        {
            assert!(marks.iter().all(|m| m.w >= MIN_SQUARE), "clue {n}");
        }
    }
    let (marks, _) = marks(counter_view(line(0, 6), col_area(60.0), false, 60.0, 6));
    assert!(marks.iter().all(|m| m.w >= MIN_SQUARE));
}

/// An 8×2 board whose rows ask for 6 and 7 cells: square counters.
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
fn square_counters_show_the_three_states() {
    let mut app = open(&long_rows());
    let accent = Rgba::hex(0x19_E3C0);
    let outlines = |d: &Recorder| {
        d.calls
            .iter()
            .filter(|c| matches!(c, Call::StrokeRect(_, k) if *k == accent))
            .count()
    };
    // Nothing placed: 6 + 7 outlined squares.
    let drawing = settle(&mut app);
    assert!(outlines(&drawing) >= 13);
    assert_eq!(fills(&drawing, ERROR), 0);
    // Seven cells in the row that wants six: all six squares red.
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

#[test]
fn square_counters_of_one_puzzle_share_a_size() {
    let cell = 40.0;
    let largest = 7;
    let side = |n: usize| {
        let (marks, ticks) = marks(counter_view(
            line(0, n),
            row_area(cell),
            true,
            cell,
            largest,
        ));
        assert!(!ticks);
        let pitch = marks[2].center().x - marks[0].center().x;
        (marks[0].w, pitch.abs())
    };
    assert_eq!(side(6), side(7));
    // Drawn: every outlined square of the 6 and the 7 is the same size.
    let mut app = open(&long_rows());
    let drawing = settle(&mut app);
    let band = app.layout().row_counters;
    let accent = Rgba::hex(0x19_E3C0);
    let sizes: Vec<(f32, f32)> = drawing
        .calls
        .iter()
        .filter_map(|c| match c {
            Call::StrokeRect(r, k) if *k == accent && band.contains_rect(*r) => Some((r.w, r.h)),
            _ => None,
        })
        .collect();
    assert_eq!(sizes.len(), 13);
    let same = |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).abs() < 0.01 && (a.1 - b.1).abs() < 0.01;
    assert!(sizes.iter().all(|&s| same(s, sizes[0])), "{sizes:?}");
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
        config.code = Some(encode(&dominoes()).unwrap());
        config.touch = w < h || h < 520.0;
        let mut app = App::new(config);
        settle(&mut app);
        let layout = app.layout().clone();
        let what = format!("{w}x{h}");
        let input = LayoutInput {
            width: w,
            height: h,
            insets: Insets::default(),
            touch: w < h || h < 520.0,
            lang: app.lang(),
            grid_width: 16,
            grid_height: 16,
            pieces: 32,
            max_piece_side: 2,
        };
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
    config.code = Some(encode(&dominoes()).unwrap());
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
