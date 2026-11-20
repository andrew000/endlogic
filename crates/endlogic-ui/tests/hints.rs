//! Hints: the silhouette, placing pieces, pointing out a wrong one.

mod common;

use common::*;
use endlogic_core::code::encode;
use endlogic_core::solver::{DEFAULT_NODE_BUDGET, search};
use endlogic_core::{Puzzle, Solution};
use endlogic_ui::{App, Budgets, CodeKind, Event, Key, PointerKind, Text};

/// Cells covered in `solution`, row-major.
fn coverage(puzzle: &Puzzle, solution: &Solution) -> Vec<bool> {
    solution
        .cell_owners(puzzle)
        .unwrap()
        .iter()
        .map(Option::is_some)
        .collect()
}

fn all(puzzle: &Puzzle) -> Vec<Solution> {
    search(puzzle, usize::MAX, DEFAULT_NODE_BUDGET).solutions
}

fn hint(app: &mut App) {
    app.handle(Event::Key(Key::H));
    settle(app);
}

fn placed(app: &App) -> Vec<Option<(usize, usize)>> {
    let board = app.board().unwrap();
    (0..board.piece_count())
        .map(|i| board.piece(i).at)
        .collect()
}

#[test]
fn the_first_hint_shows_the_silhouette() {
    let puzzle = sample("wln006-2.txt");
    let mut app = open(&puzzle);
    assert_eq!(app.silhouette(), &[] as &[bool]);
    hint(&mut app);
    assert_eq!(app.hints_used(), 1);
    assert_eq!(app.silhouette(), coverage(&puzzle, &all(&puzzle)[0]));
    assert!(placed(&app).iter().all(Option::is_none), "nothing moves");
    // It is drawn as a faint fill on exactly those cells.
    let drawing = settle(&mut app);
    let faint = endlogic_ui::Rgba::hex(0x19_E3C0).alpha(0.16);
    let cells = drawing
        .calls
        .iter()
        .filter(|c| matches!(c, Call::FillRect(_, k) if *k == faint))
        .count();
    assert_eq!(cells, app.silhouette().iter().filter(|&&on| on).count());
}

#[test]
fn later_hints_place_pieces_until_the_puzzle_is_solved() {
    let puzzle = sample("wln006-2.txt");
    let solution = all(&puzzle).remove(0);
    let mut app = open(&puzzle);
    hint(&mut app);
    app.handle(Event::Key(Key::H));
    // The piece travels to its place.
    assert!(app.frame(0.05, &mut Recorder::default()));
    settle(&mut app);
    assert_eq!(app.hints_used(), 2);
    let moved: Vec<usize> = (0..5).filter(|&i| placed(&app)[i].is_some()).collect();
    assert_eq!(moved.len(), 1);
    let i = moved[0];
    let at = solution.placements[i];
    assert_eq!(placed(&app)[i], Some((at.row, at.col)));
    // A piece put down by a hint is an ordinary piece.
    let (r, c) = app.board().unwrap().cells_of(i)[0];
    app.drag(
        cell(&app, r, c),
        endlogic_ui::Point::new(3.0, 3.0),
        PointerKind::Mouse,
    );
    assert_eq!(placed(&app)[i], None);
    // Hints finish the puzzle: the last piece placed by a hint wins too.
    for _ in 0..10 {
        if app.is_won() {
            break;
        }
        hint(&mut app);
    }
    assert!(app.is_won());
    let drawing = settle(&mut app);
    // The silhouette goes with the win.
    let faint = endlogic_ui::Rgba::hex(0x19_E3C0).alpha(0.16);
    assert!(
        !drawing
            .calls
            .iter()
            .any(|c| matches!(c, Call::FillRect(_, k) if *k == faint))
    );
    let line = format!("Hints: {}", app.hints_used());
    assert!(texts(&drawing).contains(&line), "{:?}", texts(&drawing));
    // The button shows the count.
    assert!(
        texts(&drawing)
            .iter()
            .any(|t| t == &format!("Hint · {}", app.hints_used()))
    );
}

#[test]
fn a_wrong_piece_is_pointed_out_and_nothing_moves() {
    let puzzle = sample("wln006-2.txt");
    let mut app = open(&puzzle);
    hint(&mut app);
    // The square at the bottom puts two cells into a row that asks for one.
    app.place(3, 0, 5, 2, PointerKind::Mouse);
    assert_eq!(placed(&app)[3], Some((5, 2)));
    let before = placed(&app);
    // The overfilled row's counter is red already.
    let base = count_color(&settle(&mut app), ERROR);
    app.handle(Event::Key(Key::H));
    let mut red = false;
    for _ in 0..10 {
        let mut drawing = Recorder::default();
        app.frame(0.05, &mut drawing);
        red |= count_color(&drawing, ERROR) > base;
    }
    settle(&mut app);
    assert!(red, "the piece turns red");
    assert_eq!(placed(&app), before, "nothing moves");
    assert_eq!(app.hints_used(), 2);
    // Red does not last.
    let drawing = settle(&mut app);
    assert_eq!(count_color(&drawing, ERROR), base);
}

#[test]
fn hints_follow_the_solution_the_player_is_building() {
    let puzzle = sample("wlh9000-1.txt");
    let solutions = all(&puzzle);
    assert_eq!(solutions.len(), 5);
    let mut app = open(&puzzle);
    hint(&mut app);
    let first = app.silhouette().to_vec();
    // Build most of a solution with a different silhouette.
    let other = solutions
        .iter()
        .find(|s| coverage(&puzzle, s) != first)
        .expect("solutions with different silhouettes")
        .clone();
    let last = other.placements.len() - 1;
    for (i, at) in other.placements.iter().enumerate().take(last) {
        let turns = u8::try_from(at.rotation.quarter_turns()).unwrap();
        app.place(i, turns, at.row, at.col, PointerKind::Mouse);
    }
    hint(&mut app);
    assert_eq!(app.silhouette(), coverage(&puzzle, &other));
    assert!(app.is_won(), "the hint put the last piece down");
    assert_eq!(app.hints_used(), 2);
}

#[test]
fn reset_keeps_hints_and_a_new_puzzle_clears_them() {
    let puzzle = sample("wln006-2.txt");
    let mut app = open(&puzzle);
    hint(&mut app);
    hint(&mut app);
    assert_eq!(app.hints_used(), 2);
    app.click(app.layout().reset.center());
    settle(&mut app);
    assert!(placed(&app).iter().all(Option::is_none));
    assert_eq!(app.hints_used(), 2);
    assert_ne!(app.silhouette(), &[] as &[bool]);
    app.click(app.layout().new.center());
    settle(&mut app);
    assert_eq!(app.hints_used(), 0);
    assert_eq!(app.silhouette(), &[] as &[bool]);
}

#[test]
fn the_hint_button_waits_and_reports_a_spent_budget() {
    let puzzle = sample("wln006-2.txt");
    let mut config = config(1366.0, 768.0);
    config.code = Some((CodeKind::Play, encode(&puzzle).unwrap()));
    config.budgets = Budgets {
        per_frame: 1,
        hint: 3,
        ..Budgets::default()
    };
    let mut app = App::new(config);
    settle(&mut app);
    app.click(app.layout().hint.center());
    assert!(app.is_hinting());
    let mut drawing = Recorder::default();
    app.frame(0.0, &mut drawing);
    drawing.texts = app.drawn_texts();
    assert!(texts(&drawing).contains(&"Hint…".to_owned()));
    // Pieces wait for the hint.
    let from = card(&app, 0);
    app.drag(from, cell(&app, 3, 3), PointerKind::Mouse);
    settle(&mut app);
    assert!(!app.is_hinting());
    assert_eq!(app.hints_used(), 0);
    assert_eq!(app.toast().map(|t| t.text), None, "the toast has faded");
    // The toast was shown.
    let mut app2 = {
        let mut config = endlogic_ui::Config {
            budgets: Budgets {
                per_frame: 1,
                hint: 3,
                ..Budgets::default()
            },
            ..common::config(1366.0, 768.0)
        };
        config.code = Some((CodeKind::Play, encode(&puzzle).unwrap()));
        App::new(config)
    };
    settle(&mut app2);
    app2.handle(Event::Key(Key::H));
    for _ in 0..5 {
        app2.frame(0.01, &mut Recorder::default());
    }
    assert_eq!(app2.toast().map(|t| t.text), Some(Text::HintFailed));
    assert_eq!(app2.hints_used(), 0);
}
