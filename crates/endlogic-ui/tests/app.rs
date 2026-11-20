//! Behaviour of the trainer, driven by synthetic events and a recording
//! painter: no browser and no window.

mod common;

use common::*;
use endlogic_core::code::encode;
use endlogic_core::generator::Difficulty;
use endlogic_core::solver::{DEFAULT_NODE_BUDGET, search};
use endlogic_ui::{
    App, Arrangement, CodeKind, Config, CounterMode, Cursor, Effect, Event, Insets, Key, Lang,
    Layout, LineState, Point, PointerKind, TOUCH_DRAG_THRESHOLD, Text,
};

// ---- dragging ----

#[test]
fn dragging_from_the_tray_places_a_piece() {
    let mut app = open(&sample("wlh9000-1.txt"));
    app.place(0, 0, 1, 0, PointerKind::Mouse);
    assert_eq!(at(&app, 0), Some((1, 0)));
    assert_eq!(app.board().unwrap().owner(2, 0), Some(0));
    // By touch, with the piece floating above the finger.
    app.place(1, 0, 3, 0, PointerKind::Touch);
    assert_eq!(at(&app, 1), Some((3, 0)));
}

#[test]
fn an_invalid_drop_returns_the_piece_to_the_tray() {
    let mut app = open(&sample("wlh9000-1.txt"));
    // (0, 0) is dead.
    app.place(3, 0, 0, 0, PointerKind::Mouse);
    assert_eq!(at(&app, 3), None);
    // Overlapping another piece.
    app.place(0, 0, 1, 0, PointerKind::Mouse);
    app.place(1, 0, 1, 0, PointerKind::Mouse);
    assert_eq!(at(&app, 1), None);
    // Far from the board.
    app.drag(card(&app, 2), Point::new(5.0, 300.0), PointerKind::Mouse);
    assert_eq!(at(&app, 2), None);
    assert_eq!(at(&app, 0), Some((1, 0)));
}

#[test]
fn the_dropped_piece_animates_then_stops() {
    let mut app = open(&sample("wlh9000-1.txt"));
    let from = card(&app, 0);
    let to = release_point(&app, 0, from, 1, 0, PointerKind::Mouse);
    let id = app.start_drag(from, to, PointerKind::Mouse);
    assert_eq!(app.dragged_piece(), Some(0));
    app.handle(Event::PointerUp(pointer(id, to, PointerKind::Mouse)));
    assert!(app.frame(0.01, &mut Recorder::default()), "snap animation");
    settle(&mut app);
    let home = app.piece_home(0);
    assert_eq!(app.piece_visual(0), home);
}

#[test]
fn moving_a_placed_piece() {
    let mut app = open(&sample("wlh9000-1.txt"));
    app.place(0, 0, 1, 0, PointerKind::Mouse);
    let press = cell(&app, 1, 0);
    let to = release_point(&app, 0, press, 3, 3, PointerKind::Mouse);
    app.drag(press, to, PointerKind::Mouse);
    assert_eq!(at(&app, 0), Some((3, 3)));
    assert_eq!(app.board().unwrap().owner(1, 0), None);
    // Dragged off the board, it goes back to the tray.
    let press = cell(&app, 3, 3);
    app.drag(press, Point::new(3.0, 3.0), PointerKind::Mouse);
    assert_eq!(at(&app, 0), None);
}

#[test]
fn a_small_movement_is_not_a_drag() {
    let mut app = open(&sample("wlh9000-1.txt"));
    let from = card(&app, 0);
    app.handle(Event::PointerDown(pointer(MOUSE, from, PointerKind::Mouse)));
    let near = Point::new(from.x + 5.0, from.y);
    app.handle(Event::PointerMove(pointer(MOUSE, near, PointerKind::Mouse)));
    assert_eq!(app.dragged_piece(), None);
    app.handle(Event::PointerUp(pointer(MOUSE, near, PointerKind::Mouse)));
    // A mouse click does not rotate.
    assert_eq!(turns(&app, 0), 0);
}

#[test]
fn a_second_pointer_is_ignored() {
    let mut app = open(&sample("wlh9000-1.txt"));
    let from = card(&app, 0);
    let to = release_point(&app, 0, from, 1, 0, PointerKind::Touch);
    let id = app.start_drag(from, to, PointerKind::Touch);
    let other = card(&app, 1);
    app.handle(Event::PointerDown(pointer(
        id + 1,
        other,
        PointerKind::Touch,
    )));
    app.handle(Event::PointerMove(pointer(
        id + 1,
        Point::new(1.0, 1.0),
        PointerKind::Touch,
    )));
    app.handle(Event::PointerUp(pointer(
        id + 1,
        Point::new(1.0, 1.0),
        PointerKind::Touch,
    )));
    assert_eq!(app.dragged_piece(), Some(0));
    app.handle(Event::PointerUp(pointer(id, to, PointerKind::Touch)));
    assert_eq!(at(&app, 0), Some((1, 0)));
    assert_eq!(at(&app, 1), None);
}

// ---- rotation ----

#[test]
fn r_rotates_the_dragged_piece() {
    let mut app = open(&sample("wlh9000-1.txt"));
    let from = card(&app, 3);
    let id = app.start_drag(from, Point::new(from.x - 40.0, from.y), PointerKind::Mouse);
    app.handle(Event::Key(Key::R));
    assert_eq!(turns(&app, 3), 1);
    assert_eq!(app.dragged_piece(), Some(3));
    // The turned piece (2 wide, 3 tall) lands where it is released.
    let press = Point::new(from.x - 40.0, from.y);
    let to = release_point(&app, 3, press, 2, 3, PointerKind::Mouse);
    app.handle(Event::PointerMove(pointer(id, to, PointerKind::Mouse)));
    app.handle(Event::PointerUp(pointer(id, to, PointerKind::Mouse)));
    settle(&mut app);
    assert_eq!(at(&app, 3), Some((2, 3)));
    assert_eq!(turns(&app, 3), 1);
}

#[test]
fn r_rotates_the_piece_under_the_cursor() {
    let mut app = open(&sample("wlh9000-1.txt"));
    app.hover(card(&app, 3));
    app.handle(Event::Key(Key::R));
    assert_eq!(turns(&app, 3), 1);
    assert!(app.frame(0.01, &mut Recorder::default()), "turn animation");
    settle(&mut app);

    app.place(0, 0, 2, 1, PointerKind::Mouse);
    app.hover(cell(&app, 2, 1));
    app.handle(Event::Key(Key::R));
    assert_eq!(turns(&app, 0), 1);
    assert!(at(&app, 0).is_some());

    // Nothing under the cursor: nothing turns.
    app.hover(Point::new(2.0, 2.0));
    let before = app.board().unwrap().clone();
    app.handle(Event::Key(Key::R));
    assert_eq!(app.board().unwrap(), &before);
}

#[test]
fn a_tap_rotates() {
    let mut app = open(&sample("wlh9000-1.txt"));
    app.tap(card(&app, 3));
    assert_eq!(turns(&app, 3), 1);
    app.tap(card(&app, 3));
    assert_eq!(turns(&app, 3), 2);
    app.place(0, 0, 2, 1, PointerKind::Touch);
    app.tap(cell(&app, 2, 1));
    assert_eq!(turns(&app, 0), 1);
    assert!(at(&app, 0).is_some());
}

#[test]
fn a_piece_that_cannot_turn_in_place_stays_and_shakes() {
    let mut app = open(&strip());
    app.place(0, 0, 0, 0, PointerKind::Mouse);
    assert_eq!(at(&app, 0), Some((0, 0)));
    app.hover(cell(&app, 0, 1));
    app.handle(Event::Key(Key::R));
    assert_eq!(at(&app, 0), Some((0, 0)));
    assert_eq!(turns(&app, 0), 0);
    // The shake is short and then the app goes idle.
    assert!(app.frame(0.05, &mut Recorder::default()), "shaking");
    let before = app.board().unwrap().clone();
    settle(&mut app);
    assert_eq!(app.board().unwrap(), &before);
    // A tap does the same.
    app.tap(cell(&app, 0, 1));
    assert_eq!((at(&app, 0), turns(&app, 0)), (Some((0, 0)), 0));
}

// ---- cancelling ----

#[test]
fn escape_cancels_a_drag() {
    let mut app = open(&sample("wlh9000-1.txt"));
    app.place(0, 0, 1, 0, PointerKind::Mouse);
    let press = cell(&app, 1, 0);
    let away = release_point(&app, 0, press, 3, 3, PointerKind::Mouse);
    let id = app.start_drag(press, away, PointerKind::Mouse);
    app.handle(Event::Key(Key::R));
    assert_eq!(at(&app, 0), None);
    app.handle(Event::Key(Key::Escape));
    assert_eq!(app.dragged_piece(), None);
    assert_eq!(at(&app, 0), Some((1, 0)));
    assert_eq!(turns(&app, 0), 0);
    // The rest of the press does nothing.
    app.handle(Event::PointerMove(pointer(id, away, PointerKind::Mouse)));
    app.handle(Event::PointerUp(pointer(id, away, PointerKind::Mouse)));
    settle(&mut app);
    assert_eq!(at(&app, 0), Some((1, 0)));

    // A cancelled drag from the tray goes back to the tray.
    let from = card(&app, 1);
    app.start_drag(from, cell(&app, 3, 3), PointerKind::Mouse);
    app.handle(Event::Key(Key::Escape));
    assert_eq!(at(&app, 1), None);
    // So does a drag the platform cancels.
    let id = app.start_drag(from, cell(&app, 3, 3), PointerKind::Touch);
    app.handle(Event::PointerCancel(pointer(
        id,
        cell(&app, 3, 3),
        PointerKind::Touch,
    )));
    assert_eq!(app.dragged_piece(), None);
    assert_eq!(at(&app, 1), None);
}

// ---- counters ----

#[test]
fn counters_show_under_exact_and_over() {
    let mut app = open(&sample("wlh9000-1.txt"));
    let board = app.board().unwrap();
    let rows = board.rows();
    assert_eq!((rows[1].filled, rows[1].clue), (0, 4));
    assert_eq!(rows[1].state(), LineState::Under);
    // Row 0 has a zero clue and nothing in it: a grey sign.
    assert_eq!((rows[0].filled, rows[0].clue), (0, 0));
    let drawing = settle(&mut app);
    assert_eq!(count_color(&drawing, ERROR), 0);
    assert!(count_color(&drawing, MUTED) > 0);

    app.place(0, 0, 1, 0, PointerKind::Mouse); // (1,0) (1,1) (2,0)
    app.place(1, 0, 1, 2, PointerKind::Mouse); // (1,2) (1,3) (2,2)
    let rows = app.board().unwrap().rows();
    assert_eq!((rows[1].filled, rows[1].state()), (4, LineState::Exact));
    assert_eq!((rows[2].filled, rows[2].state()), (2, LineState::Under));
    let cols = app.board().unwrap().cols();
    assert_eq!((cols[0].filled, cols[0].state()), (2, LineState::Exact));
    let drawing = settle(&mut app);
    assert!(count_color(&drawing, BRIGHT) > 0);
    assert_eq!(count_color(&drawing, ERROR), 0);

    // Column 0 (clue 2) gets two more cells.
    app.place(2, 0, 3, 0, PointerKind::Mouse); // (3,0) (3,1) (4,0)
    let cols = app.board().unwrap().cols();
    assert_eq!((cols[0].filled, cols[0].state()), (4, LineState::Over));
    let drawing = settle(&mut app);
    assert!(count_color(&drawing, ERROR) > 0);

    // The numbers view shows filled/clue.
    app.handle(Event::Key(Key::Tab));
    assert_eq!(app.counter_mode(), CounterMode::Numbers);
    let drawing = settle(&mut app);
    let texts = texts(&drawing);
    for label in ["4/4", "2/5", "4/2", "0/0"] {
        assert!(texts.iter().any(|t| t == label), "{label} in {texts:?}");
    }
}

#[test]
fn a_filled_zero_clue_turns_red() {
    let mut app = open(&sample("wlh9000-1.txt"));
    // Turned three times the L is `#.` over `##`; at (0, 2) it fills the
    // only free cell of row 0, whose clue is 0.
    app.place(0, 3, 0, 2, PointerKind::Mouse);
    let rows = app.board().unwrap().rows();
    assert_eq!((rows[0].filled, rows[0].state()), (1, LineState::Over));
    let drawing = settle(&mut app);
    assert!(
        drawing
            .calls
            .iter()
            .any(|c| matches!(c, Call::StrokeCircle(_, k) if *k == ERROR)),
        "red empty sign"
    );
}

// ---- winning ----

#[test]
fn any_layout_matching_the_counters_wins() {
    let puzzle = sample("wlh9000-1.txt");
    let solutions = search(&puzzle, usize::MAX, DEFAULT_NODE_BUDGET).solutions;
    assert_eq!(solutions.len(), 5);
    for solution in [&solutions[0], &solutions[3], &solutions[4]] {
        let mut app = open(&puzzle);
        app.play(solution);
        assert!(app.is_won(), "{solution:?}");
        let drawing = settle(&mut app);
        assert!(texts(&drawing).iter().any(|t| t == "REPAIR COMPLETE"));
    }
}

#[test]
fn no_win_while_a_piece_is_missing_or_a_line_is_off() {
    let puzzle = sample("wlh9000-1.txt");
    let solution = &search(&puzzle, 1, DEFAULT_NODE_BUDGET).solutions[0];
    let mut app = open(&puzzle);
    let mut partial = solution.clone();
    partial.placements.pop();
    for (i, p) in partial.placements.iter().enumerate() {
        let turns = u8::try_from(p.rotation.quarter_turns()).unwrap();
        app.place(i, turns, p.row, p.col, PointerKind::Mouse);
    }
    assert!(!app.is_won());
}

#[test]
fn after_a_win_pieces_stay_and_next_starts_a_new_puzzle() {
    let puzzle = sample("wlh9000-1.txt");
    let solution = &search(&puzzle, 1, DEFAULT_NODE_BUDGET).solutions[0];
    let mut app = open(&puzzle);
    app.play(solution);
    assert!(app.is_won());
    let board = app.board().unwrap().clone();
    let first = solution.placements[0];
    let press = cell(&app, first.row + 1, first.col);
    let press = if app.board().unwrap().owner(first.row + 1, first.col) == Some(0) {
        press
    } else {
        cell(&app, first.row, first.col + 1)
    };
    app.drag(press, Point::new(5.0, 5.0), PointerKind::Mouse);
    app.hover(press);
    app.handle(Event::Key(Key::R));
    app.tap(press);
    app.click(app.layout().reset.center());
    assert_eq!(app.board().unwrap(), &board);
    assert!(app.is_won());

    let old = app.code().unwrap().to_owned();
    app.take_effects();
    app.click(app.layout().next.center());
    assert!(app.is_generating());
    settle(&mut app);
    assert!(!app.is_won());
    let new = app.code().unwrap().to_owned();
    assert_ne!(new, old);
    assert_eq!(app.difficulty(), Difficulty::Easy, "5×5 opens as easy");
    assert!(
        app.take_effects()
            .contains(&Effect::SetCode(CodeKind::Play, new))
    );
}

#[test]
fn reset_returns_every_piece() {
    let mut app = open(&sample("wlh9000-1.txt"));
    app.place(0, 0, 1, 0, PointerKind::Mouse);
    app.place(3, 1, 2, 3, PointerKind::Mouse);
    assert!(at(&app, 0).is_some() && at(&app, 3).is_some());
    app.click(app.layout().reset.center());
    assert!(app.frame(0.01, &mut Recorder::default()), "pieces fly back");
    settle(&mut app);
    for i in 0..4 {
        assert_eq!(at(&app, i), None);
    }
    // Orientation is kept.
    assert_eq!(turns(&app, 3), 1);
}

// ---- codes, generation and effects ----

#[test]
fn a_code_opens_its_puzzle() {
    let puzzle = sample("wln006-2.txt");
    let code = encode(&puzzle).unwrap();
    let mut config = config(390.0, 844.0);
    config.code = Some((CodeKind::Play, format!(" {code} ")));
    let mut app = App::new(config);
    assert!(!app.is_generating());
    assert_eq!(app.board().unwrap().puzzle(), &puzzle);
    assert_eq!(app.code(), Some(code.as_str()));
    assert_eq!(app.puzzle_id().unwrap(), format!("Δ-H-{}", &code[..6]));
    assert_eq!(app.difficulty(), Difficulty::Hard);
    assert!(
        app.take_effects()
            .contains(&Effect::SetCode(CodeKind::Play, code))
    );
    assert!(app.toast().is_none());
    let drawing = settle(&mut app);
    assert!(texts(&drawing).iter().any(|t| t.starts_with("Δ-H-AWYAG")));
}

#[test]
fn a_bad_code_shows_a_message_and_generates() {
    let mut config = config(1366.0, 768.0);
    config.code = Some((CodeKind::Play, "not-a-code!".to_owned()));
    let mut app = App::new(config);
    let toast = app.toast().unwrap();
    assert_eq!((toast.text, toast.error), (Text::InvalidLink, true));
    assert!(app.is_generating());
    let drawing = settle(&mut app);
    assert!(app.board().is_some());
    assert!(app.toast().is_none(), "toast faded");
    drop(drawing);
    let mut app = App::new(Config {
        code: Some((CodeKind::Play, "AQ".to_owned())),
        ..config_with_lang("uk-UA")
    });
    let mut recorder = Recorder::default();
    app.frame(0.0, &mut recorder);
    recorder.texts = app.drawn_texts();
    assert!(texts(&recorder).iter().any(|t| t == "Некоректне посилання"));
}

fn config_with_lang(lang: &str) -> Config {
    Config {
        system_language: lang.to_owned(),
        ..config(1366.0, 768.0)
    }
}

#[test]
fn a_changed_address_opens_the_new_code() {
    let mut app = generated(config(1366.0, 768.0));
    let first = app.code().unwrap().to_owned();
    let code = encode(&sample("wlh9000-1.txt")).unwrap();
    app.handle(Event::CodeChanged(CodeKind::Play, code.clone()));
    assert_eq!(app.code(), Some(code.as_str()));
    app.take_effects();
    app.handle(Event::CodeChanged(CodeKind::Play, "garbage".to_owned()));
    assert_eq!(app.code(), Some(code.as_str()));
    assert_eq!(app.toast().unwrap().text, Text::InvalidLink);
    assert_eq!(
        app.take_effects(),
        vec![Effect::SetCode(CodeKind::Play, code)]
    );
    assert_ne!(first, app.code().unwrap());
}

#[test]
fn generated_puzzles_publish_their_code() {
    let mut app = App::new(config(1366.0, 768.0));
    assert_eq!(app.take_effects(), Vec::new());
    let drawing = settle(&mut app);
    let code = app.code().unwrap().to_owned();
    assert!(
        app.take_effects()
            .contains(&Effect::SetCode(CodeKind::Play, code.clone()))
    );
    assert_eq!(app.difficulty(), Difficulty::Normal);
    assert_eq!(app.puzzle_id().unwrap(), format!("Δ-N-{}", &code[..6]));
    assert!(!texts(&drawing).iter().any(|t| t == "Generating…"));

    app.click(app.layout().new.center());
    let mut recorder = Recorder::default();
    assert!(app.is_generating());
    // The first frame may already finish; the indicator shows while it runs.
    let busy = app.frame(0.0, &mut recorder);
    recorder.texts = app.drawn_texts();
    if app.is_generating() {
        assert!(busy);
        assert!(texts(&recorder).iter().any(|t| t == "Generating…"));
    }
    settle(&mut app);
    assert_ne!(app.code().unwrap(), code);
}

#[test]
fn share_copies_a_link() {
    let mut app = open(&sample("wlh9000-1.txt"));
    let code = app.code().unwrap().to_owned();
    app.take_effects();
    app.click(app.layout().share.center());
    let copied: Vec<Effect> = app
        .take_effects()
        .into_iter()
        .filter(|e| matches!(e, Effect::CopyLink(..)))
        .collect();
    assert_eq!(copied, vec![Effect::CopyLink(CodeKind::Play, code.clone())]);
    // No toast until the shell reports the result.
    assert_eq!(app.toast(), None);
    app.handle(Event::CopyFinished(true));
    let toast = app.toast().unwrap();
    assert_eq!((toast.text, toast.error), (Text::LinkCopied, false));
    settle(&mut app);
    assert!(app.toast().is_none());

    app.click(app.layout().share.center());
    app.handle(Event::CopyFinished(false));
    let toast = app.toast().unwrap();
    assert_eq!((toast.text, toast.error), (Text::CopyFailed, true));
    let mut recorder = Recorder::default();
    app.frame(0.2, &mut recorder);
    recorder.texts = app.drawn_texts();
    assert!(
        texts(&recorder)
            .iter()
            .any(|t| t == "Couldn't copy the link")
    );
    let mut ua = config_with_lang("uk-UA");
    ua.code = Some((CodeKind::Play, code));
    let mut app = App::new(ua);
    app.handle(Event::CopyFinished(false));
    let mut recorder = Recorder::default();
    app.frame(0.2, &mut recorder);
    recorder.texts = app.drawn_texts();
    assert!(
        texts(&recorder)
            .iter()
            .any(|t| t == "Не вдалося скопіювати посилання")
    );
}

#[test]
fn settings_are_saved_and_restored() {
    let mut app = generated(config(1366.0, 768.0));
    assert_eq!(app.lang(), Lang::En);
    app.take_effects();
    let (_, ua) = Layout::halves(app.layout().lang);
    app.click(ua.center());
    assert_eq!(app.lang(), Lang::Ua);
    let (grid, numbers) = Layout::halves(app.layout().counters);
    app.click(numbers.center());
    app.handle(Event::Key(Key::Tab));
    app.handle(Event::Key(Key::Tab));
    app.click(grid.center());
    app.click(app.layout().tabs[3].center());
    assert_eq!(app.difficulty(), Difficulty::Expert);
    let saves: Vec<(&str, String)> = app
        .take_effects()
        .into_iter()
        .filter_map(|e| match e {
            Effect::SaveSetting { key, value } => Some((key, value)),
            _ => None,
        })
        .collect();
    let expected = [
        ("lang", "ua"),
        ("counters", "numbers"),
        ("counters", "grid"),
        ("counters", "numbers"),
        ("counters", "grid"),
        ("difficulty", "expert"),
    ];
    assert_eq!(saves, expected.map(|(k, v)| (k, v.to_owned())).to_vec());
    let drawing = settle(&mut app);
    assert!(texts(&drawing).iter().any(|t| t == "ЕКСПЕРТ"));
    assert!(app.puzzle_id().unwrap().starts_with("Δ-X-"));
    assert_eq!(app.board().unwrap().width(), 8);

    let mut config = config(1366.0, 768.0);
    config.settings = vec![
        ("lang".to_owned(), "ua".to_owned()),
        ("counters".to_owned(), "numbers".to_owned()),
        ("difficulty".to_owned(), "hard".to_owned()),
        ("unknown".to_owned(), "x".to_owned()),
    ];
    let app = generated(config);
    assert_eq!(app.lang(), Lang::Ua);
    assert_eq!(app.counter_mode(), CounterMode::Numbers);
    assert_eq!(app.difficulty(), Difficulty::Hard);
    assert_eq!(app.board().unwrap().width(), 7);
}

#[test]
fn the_system_language_picks_the_initial_language() {
    for (tag, lang) in [
        ("uk-UA", Lang::Ua),
        ("uk", Lang::Ua),
        ("en-GB", Lang::En),
        ("de-DE", Lang::En),
        ("", Lang::En),
    ] {
        assert_eq!(App::new(config_with_lang(tag)).lang(), lang, "{tag}");
    }
    let mut config = config_with_lang("uk-UA");
    config.settings = vec![("lang".to_owned(), "en".to_owned())];
    assert_eq!(App::new(config).lang(), Lang::En);
}

#[test]
fn the_cursor_follows_what_is_under_it() {
    let mut app = open(&sample("wlh9000-1.txt"));
    app.take_effects();
    app.hover(card(&app, 0));
    app.hover(app.layout().new.center());
    app.hover(Point::new(1.0, app.layout().frame.center().y));
    let from = card(&app, 0);
    app.start_drag(from, cell(&app, 2, 2), PointerKind::Mouse);
    let cursors: Vec<Cursor> = app
        .take_effects()
        .into_iter()
        .filter_map(|e| match e {
            Effect::SetCursor(c) => Some(c),
            _ => None,
        })
        .collect();
    assert_eq!(
        cursors,
        [
            Cursor::Grab,
            Cursor::Pointer,
            Cursor::Default,
            Cursor::Grab,
            Cursor::Grabbing
        ]
    );
}

// ---- frames ----

#[test]
fn idle_frames_are_not_requested() {
    let mut app = open(&sample("wlh9000-1.txt"));
    assert!(!app.frame(0.016, &mut Recorder::default()));
    app.hover(card(&app, 1));
    assert!(!app.frame(0.016, &mut Recorder::default()));
    let from = card(&app, 0);
    app.start_drag(from, cell(&app, 2, 2), PointerKind::Mouse);
    // The piece grows to board size, then nothing moves while it is held.
    assert!(app.frame(0.016, &mut Recorder::default()), "growing");
    settle(&mut app);
    assert_eq!(app.dragged_piece(), Some(0));
    assert!(
        !app.frame(0.016, &mut Recorder::default()),
        "a held piece does not animate"
    );
    app.handle(Event::Resize {
        width: 500.0,
        height: 900.0,
        insets: Insets::default(),
    });
    assert_eq!(app.layout().kind, Arrangement::Tall);
    assert!(!app.frame(0.016, &mut Recorder::default()));
}

#[test]
fn same_entropy_and_events_give_the_same_state() {
    let script = |app: &mut App| {
        settle(app);
        app.click(app.layout().tabs[2].center());
        settle(app);
        let from = card(app, 0);
        app.drag(from, cell(app, 3, 3), PointerKind::Mouse);
        app.hover(card(app, 1));
        app.handle(Event::Key(Key::R));
        app.tap(card(app, 2));
        app.click(app.layout().new.center());
        settle(app);
        let mut recorder = Recorder::default();
        app.frame(0.0, &mut recorder);
        recorder.texts = app.drawn_texts();
        (format!("{app:?}"), recorder.calls, app.take_effects())
    };
    let mut a = App::new(config(844.0, 390.0));
    let mut b = App::new(config(844.0, 390.0));
    let first = script(&mut a);
    assert_eq!(first, script(&mut b));
    let mut other = config(844.0, 390.0);
    other.entropy += 1;
    let mut c = App::new(other);
    assert_ne!(first.0, script(&mut c).0);
}

// ---- controls ----

#[test]
fn a_bad_drop_over_the_grid_returns_a_board_piece_to_its_cell() {
    let mut app = open(&sample("wlh9000-1.txt"));
    app.place(0, 0, 1, 0, PointerKind::Mouse);
    app.place(1, 0, 3, 3, PointerKind::Mouse);
    // Take piece 0, turn it, and drop it onto piece 1: invalid, but over
    // the grid, so it goes back to (1, 0) in its old turn.
    let press = cell(&app, 1, 0);
    let id = app.start_drag(
        press,
        Point::new(press.x + 20.0, press.y),
        PointerKind::Mouse,
    );
    app.handle(Event::Key(Key::R));
    let to = release_point(&app, 0, press, 3, 3, PointerKind::Mouse);
    app.handle(Event::PointerMove(pointer(id, to, PointerKind::Mouse)));
    app.handle(Event::PointerUp(pointer(id, to, PointerKind::Mouse)));
    assert_eq!((at(&app, 0), turns(&app, 0)), (Some((1, 0)), 0));
    assert!(app.frame(0.01, &mut Recorder::default()), "flies back");
    settle(&mut app);
    // Dropped half off the grid at a dead cell row: still over the grid.
    let press = cell(&app, 1, 0);
    let to = release_point(&app, 0, press, 0, 0, PointerKind::Mouse);
    app.drag(press, to, PointerKind::Mouse);
    assert_eq!(at(&app, 0), Some((1, 0)));
    // Dropped clear of the grid: to the tray.
    app.drag(cell(&app, 1, 0), Point::new(3.0, 3.0), PointerKind::Mouse);
    assert_eq!(at(&app, 0), None);
    // From the tray to a bad spot: back to the tray.
    app.place(2, 0, 3, 3, PointerKind::Mouse);
    assert_eq!(at(&app, 2), None);
}

#[test]
fn a_finger_needs_a_longer_move_to_drag() {
    let mut app = open(&sample("wlh9000-1.txt"));
    let from = card(&app, 0);
    let short = Point::new(from.x + TOUCH_DRAG_THRESHOLD - 3.0, from.y);
    app.handle(Event::PointerDown(pointer(
        FINGER,
        from,
        PointerKind::Touch,
    )));
    app.handle(Event::PointerMove(pointer(
        FINGER,
        short,
        PointerKind::Touch,
    )));
    assert_eq!(app.dragged_piece(), None);
    app.handle(Event::PointerUp(pointer(FINGER, short, PointerKind::Touch)));
    assert_eq!(turns(&app, 0), 1, "still a tap");
    let far = Point::new(from.x + TOUCH_DRAG_THRESHOLD + 1.0, from.y);
    app.handle(Event::PointerDown(pointer(
        FINGER,
        from,
        PointerKind::Touch,
    )));
    app.handle(Event::PointerMove(pointer(FINGER, far, PointerKind::Touch)));
    assert_eq!(app.dragged_piece(), Some(0));
    app.handle(Event::Key(Key::Escape));
    app.handle(Event::PointerUp(pointer(FINGER, far, PointerKind::Touch)));
    // The mouse keeps its short threshold.
    let mouse = Point::new(from.x + TOUCH_DRAG_THRESHOLD - 3.0, from.y);
    app.handle(Event::PointerDown(pointer(MOUSE, from, PointerKind::Mouse)));
    app.handle(Event::PointerMove(pointer(
        MOUSE,
        mouse,
        PointerKind::Mouse,
    )));
    assert_eq!(app.dragged_piece(), Some(0));
}

#[test]
fn a_finger_may_miss_a_piece_by_a_little() {
    let mut app = open(&sample("wlh9000-1.txt"));
    // Piece 0 covers (1, 0), (1, 1) and (2, 0); (2, 1) is empty.
    app.place(0, 0, 1, 0, PointerKind::Mouse);
    let l = app.layout().clone();
    let near = Point::new(l.cells.x + 1.3 * l.cell, l.cells.y + 2.6 * l.cell);
    let far = Point::new(l.cells.x + 1.5 * l.cell, l.cells.y + 2.6 * l.cell);
    // The mouse must hit the piece.
    app.hover(near);
    app.handle(Event::Key(Key::R));
    assert_eq!(turns(&app, 0), 0);
    // Half a cell away a finger misses too.
    app.tap(far);
    assert_eq!(turns(&app, 0), 0);
    // A finger 0.3 cell away still taps it.
    app.tap(near);
    assert_eq!(turns(&app, 0), 1);
}

#[test]
fn the_tall_header_switches_language_and_counters_with_one_button_each() {
    let mut config = config(390.0, 844.0);
    config.touch = true;
    config.code = Some((CodeKind::Play, encode(&sample("wlh9000-1.txt")).unwrap()));
    let mut app = App::new(config);
    settle(&mut app);
    let layout = app.layout().clone();
    assert!(layout.compact);
    let drawing = settle(&mut app);
    assert!(texts(&drawing).contains(&"EN".to_owned()));
    app.take_effects();
    app.tap(layout.lang.center());
    assert_eq!(app.lang(), Lang::Ua);
    app.tap(layout.lang.center());
    assert_eq!(app.lang(), Lang::En);
    assert_eq!(app.counter_mode(), CounterMode::Grid);
    app.tap(app.layout().counters.center());
    assert_eq!(app.counter_mode(), CounterMode::Numbers);
    let drawing = settle(&mut app);
    assert!(texts(&drawing).contains(&"123".to_owned()));
    app.tap(app.layout().counters.center());
    assert_eq!(app.counter_mode(), CounterMode::Grid);
    let saved: Vec<Effect> = app
        .take_effects()
        .into_iter()
        .filter(|e| matches!(e, Effect::SaveSetting { .. }))
        .collect();
    assert_eq!(saved.len(), 4);
}
