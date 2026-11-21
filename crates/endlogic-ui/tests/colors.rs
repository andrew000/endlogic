//! Puzzles with more than one color: counters, pieces, hints, the colors
//! switch, links and the per-frame budget.

mod common;

use common::*;
use endlogic_core::Color;
use endlogic_core::code::encode;
use endlogic_core::solver::{DEFAULT_NODE_BUDGET, search};
use endlogic_ui::{
    App, Arrangement, CodeKind, Effect, Event, Key, Layout, NODES_PER_FRAME, PointerKind, Rect,
    Rgba, Text, frame_nodes, palette,
};

const TEAL: Color = Color(0);
const ORANGE: Color = Color(1);

/// The game's first two-color sample: orange pieces 0–2, teal pieces 3–5.
fn game() -> App {
    open(&sample("wla980-1.txt"))
}

fn fills_in(drawing: &Recorder, area: Rect, color: Rgba) -> usize {
    drawing
        .calls
        .iter()
        .filter(|c| matches!(c, Call::FillRect(r, k) if *k == color && area.contains_rect(*r)))
        .count()
}

fn outlines_in(drawing: &Recorder, area: Rect, color: Rgba) -> usize {
    drawing
        .calls
        .iter()
        .filter(|c| matches!(c, Call::StrokeRect(r, k) if *k == color && area.contains_rect(*r)))
        .count()
}

/// The part of the row band beside row `row`.
fn row_band(app: &App, row: usize) -> Rect {
    let l = app.layout();
    let cell = l.cell_rect(row, 0);
    Rect::new(l.row_counters.x, cell.y, l.row_counters.w, cell.h)
}

/// The part of the column band above column `col`.
fn col_band(app: &App, col: usize) -> Rect {
    let l = app.layout();
    let cell = l.cell_rect(0, col);
    Rect::new(cell.x, l.col_counters.y, cell.w, l.col_counters.h)
}

#[test]
fn each_color_has_its_counter_in_its_own_color() {
    let mut app = game();
    let drawing = settle(&mut app);
    let (teal, orange) = (palette(TEAL).accent, palette(ORANGE).accent);
    // Row 1 asks for 3 teal and 2 orange cells, column 0 for 6 teal only.
    assert_eq!(outlines_in(&drawing, row_band(&app, 1), teal), 3);
    assert_eq!(outlines_in(&drawing, row_band(&app, 1), orange), 2);
    assert_eq!(outlines_in(&drawing, col_band(&app, 0), teal), 6);
    assert_eq!(outlines_in(&drawing, col_band(&app, 0), orange), 0);
    // Row 3 has a teal lock: one teal tick is filled.
    assert_eq!(fills_in(&drawing, row_band(&app, 3), teal), 1);
    // Orange over teal in a row, teal left of orange in a column.
    let ticks = |band: Rect, color: Rgba| -> Vec<Rect> {
        drawing
            .calls
            .iter()
            .filter_map(|c| match c {
                Call::StrokeRect(r, k) if *k == color && band.contains_rect(*r) => Some(*r),
                _ => None,
            })
            .collect()
    };
    let (t, o) = (
        ticks(row_band(&app, 2), teal),
        ticks(row_band(&app, 2), orange),
    );
    assert!(
        o.iter().all(|a| t.iter().all(|b| a.bottom() <= b.y)),
        "orange above"
    );
    let (t, o) = (
        ticks(col_band(&app, 2), teal),
        ticks(col_band(&app, 2), orange),
    );
    assert!(
        o.iter().all(|a| t.iter().all(|b| b.right() <= a.x)),
        "teal left"
    );
    // No colored squares anywhere: every tick is a thin bar.
    for call in &drawing.calls {
        if let Call::StrokeRect(r, k) = call
            && (*k == teal || *k == orange)
            && app.layout().row_counters.contains_rect(*r)
        {
            assert!(r.h > 2.5 * r.w, "{r:?} is not a tick");
        }
    }
}

#[test]
fn three_states_per_color_and_a_red_sign_for_a_foreign_color() {
    let mut app = game();
    let o = palette(ORANGE);
    // An orange piece in the top row, whose orange clue is 0: a red sign in
    // row 0 and column 0, where orange is not drawn otherwise.
    app.place(0, 0, 0, 0, PointerKind::Mouse);
    let drawing = settle(&mut app);
    let red_signs = |band: Rect| {
        drawing
            .calls
            .iter()
            .filter(|c| matches!(c, Call::StrokeCircle(p, k) if *k == ERROR && band.contains(*p)))
            .count()
    };
    assert_eq!(red_signs(row_band(&app, 0)), 1);
    assert_eq!(red_signs(col_band(&app, 0)), 1);
    // Column 1 wants 2 orange cells and has one: one filled, one outlined.
    assert_eq!(fills_in(&drawing, col_band(&app, 1), o.accent), 1);
    assert_eq!(outlines_in(&drawing, col_band(&app, 1), o.accent), 1);
    // Row 1 wants 2 orange cells and has one; put it where the solution
    // has it and row 1's orange counter is exact: both ticks bright.
    app.drag(cell(&app, 0, 0), card(&app, 0), PointerKind::Mouse);
    app.place(0, 0, 1, 1, PointerKind::Mouse);
    let drawing = settle(&mut app);
    assert_eq!(fills_in(&drawing, row_band(&app, 1), o.bright), 2);
    // Too many orange cells in row 4 (it wants 2): both its ticks red.
    app.place(1, 0, 4, 1, PointerKind::Mouse);
    app.place(2, 0, 4, 3, PointerKind::Mouse);
    let drawing = settle(&mut app);
    let line = app.board().unwrap().rows_of(ORANGE)[4];
    assert!(line.filled > line.clue, "{line:?}");
    assert_eq!(fills_in(&drawing, row_band(&app, 4), ERROR), 2);
}

#[test]
fn numbers_show_each_color_in_its_color() {
    let mut app = game();
    app.handle(Event::Key(Key::Tab));
    let drawing = settle(&mut app);
    let texts = texts(&drawing);
    // Row 1: teal 0/3 and orange 0/2.
    assert!(texts.contains(&"0/3".to_owned()), "{texts:?}");
    assert!(texts.contains(&"0/2".to_owned()), "{texts:?}");
}

#[test]
fn pieces_locks_and_the_drop_shadow_have_their_colors() {
    let mut app = game();
    let drawing = settle(&mut app);
    let polygons = |color: Rgba| {
        drawing
            .calls
            .iter()
            .filter(|c| matches!(c, Call::Polygon(_, k) if *k == color))
            .count()
    };
    // Three orange pieces of three cells, teal pieces of 5 + 9 + 6 cells.
    assert_eq!(polygons(palette(ORANGE).fill), 9);
    assert_eq!(polygons(palette(TEAL).fill), 20);
    assert_eq!(
        count_color(&drawing, palette(TEAL).lock),
        2,
        "two teal locks"
    );
    // Dragging an orange piece over the board: an orange shadow.
    let from = card(&app, 0);
    app.start_drag(from, cell(&app, 2, 2), PointerKind::Mouse);
    let drawing = frame(&mut app, 0.0);
    assert!(count_color(&drawing, palette(ORANGE).accent.alpha(0.3)) >= 3);
}

#[test]
fn a_two_color_puzzle_is_won_only_with_every_color_right() {
    let puzzle = sample("wla980-1.txt");
    let solution = search(&puzzle, 1, DEFAULT_NODE_BUDGET).solutions.remove(0);
    let mut app = game();
    app.play(&solution);
    assert!(app.is_won());
    assert_eq!(app.board().unwrap().colors(), [TEAL, ORANGE]);
    // Two monominoes, teal wanted left and orange right: swapped, every
    // cell is filled but no color matches, so there is no win.
    let swapped = endlogic_core::format::parse(
        "endlogic 2
colors 2
grid
..
rows a 1
rows b 1
cols a 1 0
cols b 0 1
piece a
#
piece b
#
",
    )
    .unwrap();
    let mut app = open(&swapped);
    app.place(0, 0, 0, 1, PointerKind::Mouse);
    app.place(1, 0, 0, 0, PointerKind::Mouse);
    assert!(!app.is_won());
    app.drag(cell(&app, 0, 0), card(&app, 1), PointerKind::Mouse);
    app.drag(cell(&app, 0, 1), cell(&app, 0, 0), PointerKind::Mouse);
    app.place(1, 0, 0, 1, PointerKind::Mouse);
    assert!(app.is_won());
}

#[test]
fn the_silhouette_has_the_colors_of_the_pieces() {
    let puzzle = sample("wla980-1.txt");
    let mut app = game();
    app.handle(Event::Key(Key::H));
    let drawing = settle(&mut app);
    let solution = search(&puzzle, 1, DEFAULT_NODE_BUDGET).solutions.remove(0);
    let owners = solution.cell_owners(&puzzle).unwrap();
    let expected: Vec<Option<Color>> = owners
        .iter()
        .map(|o| o.map(|i| puzzle.pieces()[i].color))
        .collect();
    assert_eq!(app.silhouette(), expected.as_slice());
    let orange = owners
        .iter()
        .filter(|o| o.is_some_and(|i| puzzle.pieces()[i].color == ORANGE))
        .count();
    assert_eq!(orange, 9);
    assert_eq!(
        count_color(&drawing, palette(ORANGE).accent.alpha(0.16)),
        orange
    );
    // Hints put pieces down until the puzzle is solved, colors kept apart.
    for _ in 0..10 {
        if app.is_won() {
            break;
        }
        app.handle(Event::Key(Key::H));
        settle(&mut app);
    }
    assert!(app.is_won());
}

#[test]
fn links_to_colored_puzzles_open() {
    let puzzle = sample("wla980-2.txt");
    for kind in [CodeKind::Play, CodeKind::Custom] {
        let mut config = config(1366.0, 768.0);
        config.code = Some((kind, encode(&puzzle).unwrap()));
        let mut app = App::new(config);
        assert!(app.toast().is_none(), "{kind:?}");
        assert!(!app.is_generating());
        assert_eq!(app.board().unwrap().puzzle(), &puzzle);
        settle(&mut app);
    }
    // A changed address too.
    let mut app = generated(config(1366.0, 768.0));
    app.handle(Event::CodeChanged(CodeKind::Play, encode(&puzzle).unwrap()));
    assert!(app.toast().is_none_or(|t| t.text != Text::InvalidLink));
    assert_eq!(app.board().unwrap().puzzle(), &puzzle);
}

/// The colors switch in each arrangement, and its setting.
#[test]
fn the_colors_switch_generates_and_is_saved() {
    for (w, h, kind) in [
        (1366.0, 768.0, Arrangement::Wide),
        (844.0, 390.0, Arrangement::LowWide),
        (390.0, 844.0, Arrangement::Tall),
    ] {
        let mut config = config(w, h);
        config.touch = kind == Arrangement::Tall;
        let mut app = generated(config);
        assert_eq!(app.layout().kind, kind);
        assert_eq!(app.colors(), 1);
        assert_eq!(app.board().unwrap().colors(), [TEAL]);
        app.take_effects();
        let switch = app.layout().colors.unwrap();
        let two = if app.layout().compact {
            switch
        } else {
            Layout::halves(switch).1
        };
        app.click(two.center());
        assert_eq!(app.colors(), 2, "{kind:?}");
        assert!(app.is_generating());
        settle(&mut app);
        assert_eq!(app.board().unwrap().colors(), [TEAL, ORANGE], "{kind:?}");
        let saved: Vec<Effect> = app
            .take_effects()
            .into_iter()
            .filter(|e| matches!(e, Effect::SaveSetting { .. }))
            .collect();
        assert_eq!(
            saved,
            [Effect::SaveSetting {
                key: "colors",
                value: "2".to_owned()
            }]
        );
        // Any level: the next tab keeps two colors.
        app.click(app.layout().tabs[3].center());
        settle(&mut app);
        assert_eq!(app.board().unwrap().colors().len(), 2);
        assert_eq!(app.board().unwrap().width(), 8);
        // The compact button goes back to one color.
        if app.layout().compact {
            app.click(app.layout().colors.unwrap().center());
            assert_eq!(app.colors(), 1);
        }
    }
    // Restored from the settings.
    let mut config = config(1366.0, 768.0);
    config.settings = vec![("colors".to_owned(), "2".to_owned())];
    let app = generated(config);
    assert_eq!(app.colors(), 2);
    assert_eq!(app.board().unwrap().colors().len(), 2);
    let mut config = common::config(1366.0, 768.0);
    config.settings = vec![("colors".to_owned(), "7".to_owned())];
    assert_eq!(App::new(config).colors(), 1);
}

#[test]
fn the_custom_tab_has_no_colors_switch() {
    let mut app = generated(config(1366.0, 768.0));
    let tab = app.layout().tabs[4];
    app.click(tab.center());
    settle(&mut app);
    assert!(app.layout().colors.is_none());
    // Playing a custom puzzle: the switch is neither drawn nor tappable.
    let mut config = config(1366.0, 768.0);
    config.code = Some((CodeKind::Custom, encode(&sample("wla980-1.txt")).unwrap()));
    let mut app = App::new(config);
    let drawing = settle(&mut app);
    assert!(!texts(&drawing).contains(&"2".to_owned()));
    let switch = app.layout().colors.unwrap();
    app.click(Layout::halves(switch).1.center());
    assert_eq!(app.colors(), 1);
    assert!(!app.is_generating());
}

#[test]
fn the_frame_budget_shrinks_on_large_fields() {
    let budget = NODES_PER_FRAME;
    assert_eq!(frame_nodes(budget, 25), 20_000);
    assert_eq!(frame_nodes(budget, 64), 20_000, "8x8");
    assert_eq!(frame_nodes(budget, 144), 8_888, "12x12");
    assert_eq!(frame_nodes(budget, 256), 5_000, "16x16");
    assert_eq!(frame_nodes(budget, 1000), 2_000, "never below 2,000");
    assert_eq!(frame_nodes(500, 256), 500, "never above the shell's budget");
    // The app applies it to the field in play.
    for (n, want) in [(8, 20_000), (12, 8_888), (16, 5_000)] {
        let row = format!("{}\n", ".".repeat(n));
        let zeros = vec!["0"; n].join(" ");
        let text = format!(
            "endlogic 1\ngrid\n{}rows {zeros}\ncols {zeros}\n",
            row.repeat(n)
        );
        let app = open(&endlogic_core::format::parse(&text).unwrap());
        assert_eq!(app.frame_budget(), want, "{n}x{n}");
    }
}
