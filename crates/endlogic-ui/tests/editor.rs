//! The custom tab: editing a puzzle, solving it, playing it.

mod common;

use std::fmt::Write as _;

use common::*;
use endlogic_core::code::{decode, encode};
use endlogic_core::diagnose::Problem;
use endlogic_core::format::parse;
use endlogic_core::solver::{DEFAULT_NODE_BUDGET, StopReason, search};
use endlogic_core::{Axis, Cell, Color, Puzzle};
use endlogic_ui::{
    App, Budgets, ClueRef, CodeKind, Config, DraftError, Editor, Effect, Event, Key, Lang,
    PointerKind, Rect, SETTING_EDITOR, SOLUTION_LIMIT, SolveState, problem_text,
};

/// An app with `puzzle` open in the editor.
fn editing(puzzle: &Puzzle, w: f32, h: f32) -> App {
    let mut config = config(w, h);
    config.code = Some((CodeKind::Edit, encode(puzzle).unwrap()));
    with(config)
}

fn with(config: Config) -> App {
    let mut app = App::new(config);
    settle(&mut app);
    assert!(app.is_editing());
    app
}

fn edit(app: &App) -> endlogic_ui::EditLayout {
    app.layout().edit.unwrap()
}

/// Clicking the middle of a rectangle, as a method so the argument may
/// read the app.
trait Press {
    fn press(&mut self, rect: Rect);
}

impl Press for App {
    fn press(&mut self, rect: Rect) {
        self.click(rect.center());
        settle(self);
    }
}

fn row_counter(app: &App, row: usize) -> Rect {
    let l = app.layout();
    let cell = l.cell_rect(row, 0);
    Rect::new(l.row_counters.x, cell.y, l.row_counters.w, cell.h)
}

fn col_counter(app: &App, col: usize) -> Rect {
    let l = app.layout();
    let cell = l.cell_rect(0, col);
    Rect::new(cell.x, l.col_counters.y, cell.w, l.col_counters.h)
}

fn editor(app: &App) -> Editor {
    app.editor().unwrap().clone()
}

fn last_code(app: &mut App, kind: CodeKind) -> Option<String> {
    app.take_effects().into_iter().rev().find_map(|e| match e {
        Effect::SetCode(k, code) if k == kind => Some(code),
        _ => None,
    })
}

/// Taps the digit strip's key `k` (the last key is "Done").
fn key(app: &mut App, k: usize) {
    let clue = app.selected_clue().unwrap();
    let ed = editor(app);
    let len = if clue.row { ed.width() } else { ed.height() };
    let keys = edit(app).digit_keys(len + 1);
    app.press(keys[k.min(keys.len() - 1)]);
}

fn draft_cell(app: &mut App, r: usize, c: usize) {
    let side = app.draft().unwrap().side;
    let parts = edit(app).piece_editor(side);
    let cell = Rect::new(
        parts.grid.x + f32::from(u8::try_from(c).unwrap()) * parts.cell,
        parts.grid.y + f32::from(u8::try_from(r).unwrap()) * parts.cell,
        parts.cell,
        parts.cell,
    );
    app.press(cell);
}

fn parts(app: &App) -> endlogic_ui::PieceEditor {
    edit(app).piece_editor(app.draft().unwrap().side)
}

fn answer(app: &App) -> endlogic_ui::ResultPanel {
    edit(app).result_panel(app.lang(), app.layout().font)
}

fn solve(app: &mut App) {
    app.press(edit(app).solve);
}

#[test]
fn the_custom_tab_opens_an_empty_editor_and_remembers_it() {
    let mut app = App::new(config(1366.0, 768.0));
    settle(&mut app);
    assert!(!app.is_editing());
    app.take_effects();
    app.press(app.layout().tabs[4]);
    assert!(app.is_editing() && app.is_custom());
    let ed = editor(&app);
    assert_eq!(ed, Editor::default());
    assert_eq!((ed.width(), ed.height()), (5, 5));
    assert_eq!(ed.pieces(), &[] as &[endlogic_core::Shape]);
    assert!((0..5).all(|i| ed.clue(true, i) == 0 && ed.clue(false, i) == 0));
    assert!(app.puzzle_id().unwrap().starts_with("Δ-C-"));
    // Every change is saved and shown in the address.
    app.press(app.layout().cell_rect(0, 0));
    let effects = app.take_effects();
    let saved = effects
        .iter()
        .rev()
        .find_map(|e| match e {
            Effect::SaveSetting { key, value } if *key == SETTING_EDITOR => Some(value.clone()),
            _ => None,
        })
        .unwrap();
    assert!(effects.contains(&Effect::SetCode(CodeKind::Edit, saved.clone())));
    // A level tab leaves; the custom tab brings the same puzzle back.
    app.press(app.layout().tabs[0]);
    assert!(!app.is_custom());
    app.press(app.layout().tabs[4]);
    assert_eq!(editor(&app).cell(0, 0), Cell::Lock(Color::DEFAULT));
    // So does a new start with the saved setting.
    let mut config = config(1366.0, 768.0);
    config.settings = vec![(SETTING_EDITOR.to_owned(), saved)];
    let mut again = App::new(config);
    settle(&mut again);
    again.press(again.layout().tabs[4]);
    assert_eq!(editor(&again), editor(&app));
}

#[test]
fn resizing_keeps_what_stays_inside() {
    let puzzle = sample("wln006-2.txt");
    let mut app = editing(&puzzle, 1366.0, 768.0);
    let before = editor(&app);
    app.press(edit(&app).width.minus);
    let ed = editor(&app);
    assert_eq!((ed.width(), ed.height()), (6, 7));
    for r in 0..7 {
        for c in 0..6 {
            assert_eq!(ed.cell(r, c), before.cell(r, c));
        }
        // Row 1 asked for 7; six cells are all it has now.
        assert_eq!(ed.clue(true, r), before.clue(true, r).min(6));
    }
    for c in 0..6 {
        assert_eq!(ed.clue(false, c), before.clue(false, c));
    }
    assert_eq!(ed.pieces(), before.pieces());
    app.press(edit(&app).width.plus);
    app.press(edit(&app).height.plus);
    let ed = editor(&app);
    assert_eq!((ed.width(), ed.height()), (7, 8));
    assert_eq!(ed.cell(0, 6), Cell::Free);
    assert_eq!((ed.clue(false, 6), ed.clue(true, 7)), (0, 0));
    assert_eq!(ed.cell(0, 1), before.cell(0, 1));
    // Sizes stay within 1..=16.
    for _ in 0..20 {
        app.press(edit(&app).height.minus);
    }
    assert_eq!(editor(&app).height(), 1);
}

#[test]
fn a_cell_turns_free_lock_dead_free() {
    let mut app = editing(&Editor::default().puzzle(), 390.0, 844.0);
    let cell = app.layout().cell_rect(2, 3);
    let mut seen = Vec::new();
    for _ in 0..3 {
        app.press(cell);
        seen.push(editor(&app).cell(2, 3));
    }
    assert_eq!(
        seen,
        vec![Cell::Lock(Color::DEFAULT), Cell::Dead, Cell::Free]
    );
}

#[test]
fn clues_take_one_tap_each_and_move_on() {
    let mut app = editing(&Editor::default().puzzle(), 390.0, 844.0);
    app.press(row_counter(&app, 0));
    assert_eq!(
        app.selected_clue(),
        Some(ClueRef {
            row: true,
            index: 0
        })
    );
    // Rows top to bottom, then columns left to right.
    let values = [1, 2, 3, 4, 5, 0, 1, 2, 3, 4];
    for v in values {
        key(&mut app, v);
    }
    let ed = editor(&app);
    assert_eq!(
        (0..5).map(|r| ed.clue(true, r)).collect::<Vec<_>>(),
        [1, 2, 3, 4, 5]
    );
    assert_eq!(
        (0..5).map(|c| ed.clue(false, c)).collect::<Vec<_>>(),
        [0, 1, 2, 3, 4]
    );
    // The strip stays open, back at the first row.
    assert_eq!(
        app.selected_clue(),
        Some(ClueRef {
            row: true,
            index: 0
        })
    );
    // Another counter takes the selection.
    app.press(col_counter(&app, 3));
    assert_eq!(
        app.selected_clue(),
        Some(ClueRef {
            row: false,
            index: 3
        })
    );
    // "Done" closes it.
    key(&mut app, usize::MAX);
    assert_eq!(app.selected_clue(), None);
    // So does a tap elsewhere, which does nothing else.
    app.press(row_counter(&app, 2));
    let before = editor(&app);
    app.press(app.layout().cell_rect(1, 1));
    assert_eq!(app.selected_clue(), None);
    assert_eq!(editor(&app), before);
    // Escape too.
    app.press(row_counter(&app, 2));
    app.handle(Event::Key(Key::Escape));
    assert_eq!(app.selected_clue(), None);
}

#[test]
fn pieces_are_added_changed_duplicated_and_deleted() {
    let mut app = editing(&Editor::default().puzzle(), 390.0, 844.0);
    // The only card adds a piece.
    app.press(app.card_of(0).unwrap());
    let draft = app.draft().unwrap().clone();
    assert_eq!((draft.side, draft.index), (6, None));
    assert_eq!(draft.shape(), Err(DraftError::Empty));
    // "Done" waits for a real piece and says why.
    app.press(parts(&app).done);
    assert!(app.draft().is_some());
    let drawing = settle(&mut app);
    assert!(texts(&drawing).contains(&"Add at least one cell".to_owned()));
    draft_cell(&mut app, 0, 0);
    draft_cell(&mut app, 1, 1);
    assert_eq!(app.draft().unwrap().shape(), Err(DraftError::Disconnected));
    let drawing = settle(&mut app);
    assert!(texts(&drawing).contains(&"Cells must be connected".to_owned()));
    draft_cell(&mut app, 0, 1);
    app.press(parts(&app).done);
    assert!(app.draft().is_none());
    assert_eq!(editor(&app).pieces().len(), 1);
    assert_eq!(editor(&app).pieces()[0].area(), 3);
    // Changing it.
    app.press(app.card_of(0).unwrap());
    assert_eq!(app.draft().unwrap().index, Some(0));
    draft_cell(&mut app, 2, 1);
    app.press(parts(&app).done);
    assert_eq!(editor(&app).pieces()[0].area(), 4);
    // Cancel keeps it as it was.
    app.press(app.card_of(0).unwrap());
    draft_cell(&mut app, 0, 0);
    app.press(parts(&app).cancel);
    assert_eq!(editor(&app).pieces()[0].area(), 4);
    // Duplicate.
    app.press(app.card_of(0).unwrap());
    app.press(parts(&app).duplicate);
    let pieces = editor(&app).pieces().to_vec();
    assert_eq!(pieces.len(), 2);
    assert_eq!(pieces[0], pieces[1]);
    // Delete.
    app.press(app.card_of(1).unwrap());
    app.press(parts(&app).delete);
    assert_eq!(editor(&app).pieces().len(), 1);
}

#[test]
fn the_status_line_names_the_first_problem() {
    let cases = [
        (
            "grid\n..\n..\nrows 2 1\ncols 1 1\npiece\n##\n",
            Problem::LineSums {
                color: Color::DEFAULT,
                rows: 3,
                cols: 2,
            },
            "Rows give 3, columns give 2",
        ),
        (
            "grid\n#.\n..\nrows 2 1\ncols 2 1\npiece\n##\npiece\n#\n",
            Problem::Area {
                color: Color::DEFAULT,
                clues: 3,
                cells: 4,
            },
            "Clues need 3 cells, pieces and locks give 4",
        ),
        (
            "grid\nx..\n...\nrows 3 0\ncols 1 1 1\npiece\n###\n",
            Problem::LineTooLong {
                color: Color::DEFAULT,
                axis: Axis::Row,
                index: 0,
                clue: 3,
                room: 2,
            },
            "Row 1 needs more than fits",
        ),
        (
            "grid\n#.\n#.\nrows 1 2\ncols 1 2\npiece\n#\n",
            Problem::TooManyLocks {
                color: Color::DEFAULT,
                axis: Axis::Column,
                index: 0,
                clue: 1,
                locks: 2,
            },
            "Column 1 has more locks than its clue",
        ),
        (
            "grid\n..\n..\nrows 0 0\ncols 0 0\n",
            Problem::NoPieces,
            "Add a piece",
        ),
    ];
    for (text, problem, words) in cases {
        let puzzle = parse(&format!("endlogic 1\n{text}")).unwrap();
        let mut app = editing(&puzzle, 1366.0, 768.0);
        assert_eq!(app.first_problem(), Some(problem));
        assert_eq!(problem_text(Lang::En, &problem), words);
        let drawing = settle(&mut app);
        let shown = texts(&drawing).join(" ");
        assert!(shown.contains(words), "{words}: {shown}");
        assert_ne!(problem_text(Lang::Ua, &problem), "");
    }
    let mut app = editing(&sample("wln006-2.txt"), 1366.0, 768.0);
    assert_eq!(app.first_problem(), None);
    let drawing = settle(&mut app);
    assert!(!texts(&drawing).iter().any(|t| t.contains("give")));
}

#[test]
fn clear_asks_twice() {
    let mut app = editing(&sample("wln006-2.txt"), 390.0, 844.0);
    let clear = edit(&app).clear;
    app.click(clear.center());
    assert!(app.clear_armed());
    let drawing = settle(&mut app);
    assert!(!app.clear_armed(), "the question goes after three seconds");
    let _ = drawing;
    app.click(clear.center());
    let mut drawing = Recorder::default();
    app.frame(0.1, &mut drawing);
    drawing.texts = app.drawn_texts();
    assert!(texts(&drawing).contains(&"Sure?".to_owned()));
    assert_eq!(editor(&app), Editor::from_puzzle(&sample("wln006-2.txt")));
    app.click(clear.center());
    settle(&mut app);
    assert_eq!(editor(&app), Editor::empty(7, 7));
}

#[test]
fn edit_and_play_codes_go_both_ways() {
    let wln = sample("wln006-2.txt");
    let code = encode(&wln).unwrap();
    // An edit link opens the editor.
    let mut app = editing(&wln, 1366.0, 768.0);
    assert_eq!(editor(&app).puzzle(), wln);
    // Sharing from the editor gives an edit link.
    app.take_effects();
    app.press(app.layout().share);
    assert!(
        app.take_effects()
            .contains(&Effect::CopyLink(CodeKind::Edit, code.clone()))
    );
    // A changed address switches puzzles.
    let other = sample("wlh9000-1.txt");
    app.handle(Event::CodeChanged(CodeKind::Edit, encode(&other).unwrap()));
    assert_eq!(editor(&app).puzzle(), other);
    // Play: a custom play code, and the custom tab stays.
    app.take_effects();
    app.press(edit(&app).play);
    assert!(!app.is_editing() && app.is_custom());
    let played = last_code(&mut app, CodeKind::Custom).unwrap();
    assert_eq!(decode(&played).unwrap(), other);
    assert!(app.puzzle_id().unwrap().starts_with("Δ-C-"));
    // "Edit" goes back with the same puzzle.
    app.press(app.layout().new);
    assert!(app.is_editing());
    assert_eq!(editor(&app).puzzle(), other);
    assert!(last_code(&mut app, CodeKind::Edit).is_some());
    // A play link of an unusual size opens the custom tab, playing.
    let odd =
        parse("endlogic 1\ngrid\n...\n...\nrows 3 1\ncols 2 1 1\npiece\n###\npiece\n#\n").unwrap();
    let mut config = config(390.0, 844.0);
    config.code = Some((CodeKind::Play, encode(&odd).unwrap()));
    let mut app = App::new(config);
    settle(&mut app);
    assert!(app.is_custom() && !app.is_editing());
    assert_eq!(app.board().unwrap().puzzle(), &odd);
    assert!(last_code(&mut app, CodeKind::Custom).is_some());
    // A bad edit link is refused.
    let mut bad = common::config(390.0, 844.0);
    bad.code = Some((CodeKind::Edit, "nope".to_owned()));
    let mut app = App::new(bad);
    settle(&mut app);
    assert!(!app.is_custom());
}

#[test]
fn playing_a_custom_puzzle_has_hints_and_edit_returns() {
    let wln = sample("wln006-2.txt");
    let mut app = editing(&wln, 390.0, 844.0);
    app.press(edit(&app).play);
    for _ in 0..10 {
        if app.is_won() {
            break;
        }
        app.handle(Event::Key(Key::H));
        settle(&mut app);
    }
    assert!(app.is_won());
    // The banner's button goes back to the editor.
    app.press(app.layout().next);
    assert!(app.is_editing());
    assert_eq!(editor(&app).puzzle(), wln);
}

#[test]
fn solving_finds_the_one_solution() {
    let wln = sample("wln006-2.txt");
    let mut app = editing(&wln, 1366.0, 768.0);
    solve(&mut app);
    assert_eq!(
        app.solve_state(),
        Some(SolveState::Finished(StopReason::Exhausted))
    );
    let expected = search(&wln, usize::MAX, DEFAULT_NODE_BUDGET).solutions;
    assert_eq!(app.solutions().0, expected.as_slice());
    let drawing = settle(&mut app);
    assert!(texts(&drawing).contains(&"Solution 1 of 1".to_owned()));
    // "Done" closes the answer.
    app.press(answer(&app).action);
    assert_eq!(app.solve_state(), None);
}

#[test]
fn several_solutions_can_be_paged() {
    let mut app = editing(&sample("wlh9000-1.txt"), 390.0, 844.0);
    solve(&mut app);
    let (solutions, view) = app.solutions();
    assert_eq!((solutions.len(), view), (5, 0));
    let first = app.shown_solution().unwrap().clone();
    app.press(answer(&app).next);
    assert_eq!(app.solutions().1, 1);
    assert_ne!(app.shown_solution().unwrap(), &first);
    let drawing = settle(&mut app);
    assert!(texts(&drawing).contains(&"Solution 2 of 5".to_owned()));
    app.press(answer(&app).prev);
    app.press(answer(&app).prev);
    assert_eq!(app.solutions().1, 4, "wraps around");
}

#[test]
fn no_solution_says_so_with_the_reason() {
    let mut app = editing(&sample("no-solution.txt"), 844.0, 390.0);
    solve(&mut app);
    assert_eq!(
        app.solve_state(),
        Some(SolveState::Finished(StopReason::Exhausted))
    );
    assert_eq!(app.solutions().0, []);
    let drawing = settle(&mut app);
    assert!(texts(&drawing).contains(&"No solution".to_owned()));
    // With a structural problem, the reason follows.
    let bad = parse("endlogic 1\ngrid\n..\n..\nrows 2 1\ncols 1 1\npiece\n##\n").unwrap();
    let mut app = editing(&bad, 844.0, 390.0);
    solve(&mut app);
    let drawing = settle(&mut app);
    assert!(texts(&drawing).contains(&"Rows give 3, columns give 2".to_owned()));
}

#[test]
fn the_list_stops_at_fifty() {
    // Five monominoes, one per row and column: 120 solutions.
    let five = parse(
        "endlogic 1\ngrid\n.....\n.....\n.....\n.....\n.....\nrows 1 1 1 1 1\ncols 1 1 1 1 1\npiece\n#\npiece\n#\npiece\n#\npiece\n#\npiece\n#\n",
    )
    .unwrap();
    let mut app = editing(&five, 1366.0, 768.0);
    solve(&mut app);
    assert_eq!(
        app.solve_state(),
        Some(SolveState::Finished(StopReason::SolutionLimit))
    );
    assert_eq!(app.solutions().0.len(), SOLUTION_LIMIT);
    let drawing = settle(&mut app);
    assert!(texts(&drawing).contains(&"Solution 1 of 50+".to_owned()));
}

#[test]
fn a_spent_budget_stops_and_can_continue() {
    let mut config = config(390.0, 844.0);
    config.code = Some((CodeKind::Edit, encode(&sample("wlh9000-1.txt")).unwrap()));
    config.budgets = Budgets {
        per_frame: 10,
        solve: 30,
        ..Budgets::default()
    };
    let mut app = with(config);
    solve(&mut app);
    assert_eq!(app.solve_state(), Some(SolveState::Stopped));
    let drawing = settle(&mut app);
    assert!(texts(&drawing).contains(&"Too complex, stopped".to_owned()));
    let mut rounds = 0;
    while app.solve_state() == Some(SolveState::Stopped) {
        rounds += 1;
        assert!(rounds < 100);
        app.press(answer(&app).action);
    }
    assert_eq!(
        app.solve_state(),
        Some(SolveState::Finished(StopReason::Exhausted))
    );
    assert_eq!(app.solutions().0.len(), 5);
    assert!(rounds > 1);
}

#[test]
fn solving_can_be_cancelled_and_any_change_clears_the_answer() {
    let mut config = config(390.0, 844.0);
    config.code = Some((CodeKind::Edit, encode(&sample("wlh9000-1.txt")).unwrap()));
    config.budgets = Budgets {
        per_frame: 1,
        ..Budgets::default()
    };
    let mut app = with(config);
    app.click(edit(&app).solve.center());
    app.frame(0.01, &mut Recorder::default());
    assert_eq!(app.solve_state(), Some(SolveState::Running));
    let mut drawing = Recorder::default();
    app.frame(0.01, &mut drawing);
    drawing.texts = app.drawn_texts();
    assert!(texts(&drawing).contains(&"Solving…".to_owned()));
    app.click(answer(&app).action.center());
    assert_eq!(app.solve_state(), None);
    // A finished answer goes with the next change.
    let mut app = editing(&sample("wlh9000-1.txt"), 390.0, 844.0);
    solve(&mut app);
    assert!(app.solve_state().is_some());
    app.press(app.layout().cell_rect(4, 4));
    assert_eq!(app.solve_state(), None);
    assert_eq!(app.shown_solution(), None);
    let _ = PointerKind::Mouse;
}

#[test]
fn thirty_two_pieces_fill_the_tray_pages() {
    let mut text = String::from("endlogic 1\ngrid\n");
    for _ in 0..16 {
        text.push_str(&".".repeat(16));
        text.push('\n');
    }
    let clues = vec!["0"; 16].join(" ");
    writeln!(text, "rows {clues}\ncols {clues}").unwrap();
    for _ in 0..31 {
        text.push_str("piece\n##\n");
    }
    let puzzle = parse(&text).unwrap();
    let mut app = editing(&puzzle, 390.0, 844.0);
    // 31 pieces and the card that adds one, over several pages.
    assert!(app.tray_pages() > 1);
    let pager = app.layout().pager.unwrap();
    while app.card_of(31).is_none() {
        app.press(pager.next);
    }
    app.press(app.card_of(31).unwrap());
    assert_eq!(app.draft().unwrap().index, None);
    draft_cell(&mut app, 0, 0);
    app.press(parts(&app).done);
    assert_eq!(editor(&app).pieces().len(), 32);
    // No more room: no adding card, no duplicating.
    while app.card_of(0).is_none() {
        app.press(pager.next);
    }
    let mut cards = 0;
    for _ in 0..app.tray_pages() {
        cards += (0..40).filter(|&i| app.card_of(i).is_some()).count();
        app.press(pager.next);
    }
    assert_eq!(cards, 32);
    app.press(app.card_of(0).unwrap());
    let before = editor(&app).pieces().len();
    app.press(parts(&app).duplicate);
    assert_eq!(editor(&app).pieces().len(), before);
}

#[test]
fn every_editor_state_draws_at_every_size() {
    let sizes = [
        (360.0, 640.0),
        (390.0, 844.0),
        (800.0, 895.0),
        (780.0, 300.0),
        (844.0, 390.0),
        (1366.0, 768.0),
        (1920.0, 1080.0),
    ];
    for (w, h) in sizes {
        let mut app = editing(&sample("wlh9000-1.txt"), w, h);
        settle(&mut app);
        solve(&mut app);
        assert!(app.shown_solution().is_some(), "{w}x{h}");
        app.press(row_counter(&app, 1));
        assert!(app.selected_clue().is_some(), "{w}x{h}");
        settle(&mut app);
        key(&mut app, usize::MAX);
        app.press(answer(&app).action);
        app.press(app.card_of(0).unwrap());
        assert!(app.draft().is_some(), "{w}x{h}");
        let drawing = settle(&mut app);
        assert!(texts(&drawing).contains(&"Duplicate".to_owned()), "{w}x{h}");
        app.press(parts(&app).cancel);
        // Strange sizes do not panic either.
        app.handle(Event::Resize {
            width: 10.0,
            height: 10.0,
            insets: endlogic_ui::Insets::default(),
        });
        settle(&mut app);
    }
}

/// The kind of every `SetCode` and `CopyLink` since the last call.
fn kinds(app: &mut App) -> (Vec<CodeKind>, Vec<CodeKind>) {
    let mut set = Vec::new();
    let mut copied = Vec::new();
    for effect in app.take_effects() {
        match effect {
            Effect::SetCode(kind, _) => set.push(kind),
            Effect::CopyLink(kind, _) => copied.push(kind),
            _ => {}
        }
    }
    (set, copied)
}

#[test]
fn the_three_code_kinds_go_both_ways() {
    // wln006-2 is 7×7, the size of the hard level.
    let wln = sample("wln006-2.txt");
    let code = encode(&wln).unwrap();
    let open = |kind: CodeKind| {
        let mut config = config(1366.0, 768.0);
        config.code = Some((kind, code.clone()));
        let mut app = App::new(config);
        settle(&mut app);
        app
    };
    // Play: the hard level, with "New"; the address and the link say play.
    let mut app = open(CodeKind::Play);
    assert!(!app.is_custom());
    assert_eq!(app.puzzle_id().unwrap().chars().nth(2), Some('H'));
    let drawing = settle(&mut app);
    assert!(texts(&drawing).contains(&"New".to_owned()));
    assert_eq!(kinds(&mut app).0, vec![CodeKind::Play]);
    app.press(app.layout().share);
    assert_eq!(kinds(&mut app).1, vec![CodeKind::Play]);
    // Custom: the custom tab playing, with "Edit", though the size is
    // standard.
    let mut app = open(CodeKind::Custom);
    assert!(app.is_custom() && !app.is_editing());
    assert!(app.puzzle_id().unwrap().starts_with("Δ-C-"));
    assert_eq!(app.board().unwrap().puzzle(), &wln);
    let drawing = settle(&mut app);
    assert!(texts(&drawing).contains(&"Edit".to_owned()));
    assert_eq!(kinds(&mut app).0, vec![CodeKind::Custom]);
    app.press(app.layout().share);
    assert_eq!(kinds(&mut app).1, vec![CodeKind::Custom]);
    // "Edit" opens the editor: an edit code; sharing there gives an edit
    // link; "Play" goes back with a custom code.
    app.press(app.layout().new);
    assert!(app.is_editing());
    assert_eq!(kinds(&mut app).0.last(), Some(&CodeKind::Edit));
    app.press(app.layout().share);
    assert_eq!(kinds(&mut app).1, vec![CodeKind::Edit]);
    app.press(edit(&app).play);
    assert_eq!(kinds(&mut app).0, vec![CodeKind::Custom]);
    // A changed address of each kind is followed.
    let other = sample("wlh9000-1.txt");
    let other_code = encode(&other).unwrap();
    app.handle(Event::CodeChanged(CodeKind::Play, other_code.clone()));
    assert!(!app.is_custom(), "5×5 is the easy level");
    app.handle(Event::CodeChanged(CodeKind::Custom, other_code.clone()));
    assert!(app.is_custom() && !app.is_editing());
    app.handle(Event::CodeChanged(CodeKind::Edit, other_code));
    assert!(app.is_editing());
    assert_eq!(editor(&app).puzzle(), other);
    // Edit codes open the editor directly.
    let app = open(CodeKind::Edit);
    assert!(app.is_editing());
}
