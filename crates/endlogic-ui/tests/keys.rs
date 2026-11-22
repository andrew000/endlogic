//! Typing clue values in the editor: digits, two-digit values on long
//! lines, moving between fields with arrows and backspace, enter.

mod common;

use common::*;
use endlogic_core::Color;
use endlogic_core::code::encode;
use endlogic_ui::{App, ClueRef, CodeKind, Editor, Event, Key, Rect};

fn editing(editor: &Editor) -> App {
    let mut config = config(1366.0, 768.0);
    config.code = Some((CodeKind::Edit, encode(&editor.puzzle()).unwrap()));
    let mut app = App::new(config);
    settle(&mut app);
    assert!(app.is_editing());
    app
}

fn press_key(app: &mut App, key: Key) {
    app.handle(Event::Key(key));
}

fn digits(app: &mut App, text: &str) {
    for c in text.chars() {
        let d = u8::try_from(c.to_digit(10).unwrap()).unwrap();
        press_key(app, Key::Digit(d));
    }
}

fn clue(color: Color, row: bool, index: usize) -> ClueRef {
    ClueRef { color, row, index }
}

const TEAL: Color = Color(0);
const ORANGE: Color = Color(1);

fn value(app: &App, c: ClueRef) -> usize {
    app.editor().unwrap().clue(c.color, c.row, c.index)
}

#[test]
fn a_digit_without_a_field_types_into_the_first_one() {
    let mut app = editing(&Editor::empty(5, 5));
    assert_eq!(app.selected_clue(), None);
    digits(&mut app, "3");
    assert_eq!(value(&app, clue(TEAL, true, 0)), 3);
    assert_eq!(app.selected_clue(), Some(clue(TEAL, true, 1)));
    // Through the rows, then the columns, then back to the first row.
    digits(&mut app, "1234");
    assert_eq!(app.selected_clue(), Some(clue(TEAL, false, 0)));
    digits(&mut app, "54321");
    assert_eq!(app.selected_clue(), Some(clue(TEAL, true, 0)));
    let ed = app.editor().unwrap();
    let rows: Vec<usize> = (0..5).map(|i| ed.clue(TEAL, true, i)).collect();
    let cols: Vec<usize> = (0..5).map(|i| ed.clue(TEAL, false, i)).collect();
    assert_eq!(rows, [3, 1, 2, 3, 4]);
    assert_eq!(cols, [5, 4, 3, 2, 1]);
}

#[test]
fn arrows_and_backspace_move_without_changing_values() {
    let mut app = editing(&Editor::empty(5, 5));
    press_key(&mut app, Key::Right);
    assert_eq!(app.selected_clue(), Some(clue(TEAL, true, 0)));
    for (key, at) in [
        (Key::Right, clue(TEAL, true, 1)),
        (Key::Down, clue(TEAL, true, 2)),
        (Key::Left, clue(TEAL, true, 1)),
        (Key::Up, clue(TEAL, true, 0)),
        // Before the first field comes the last one.
        (Key::Backspace, clue(TEAL, false, 4)),
        (Key::Right, clue(TEAL, true, 0)),
    ] {
        press_key(&mut app, key);
        assert_eq!(app.selected_clue(), Some(at), "{key:?}");
    }
    assert!(app.editor().unwrap().is_blank());
    // Backspace goes back a field, keeping its value.
    digits(&mut app, "2");
    press_key(&mut app, Key::Backspace);
    assert_eq!(app.selected_clue(), Some(clue(TEAL, true, 0)));
    assert_eq!(value(&app, clue(TEAL, true, 0)), 2);
}

#[test]
fn enter_closes_the_digit_strip_and_escape_too() {
    let mut app = editing(&Editor::empty(5, 5));
    digits(&mut app, "2");
    assert!(app.selected_clue().is_some());
    press_key(&mut app, Key::Enter);
    assert_eq!(app.selected_clue(), None);
    press_key(&mut app, Key::Down);
    press_key(&mut app, Key::Escape);
    assert_eq!(app.selected_clue(), None);
}

#[test]
fn a_digit_over_the_line_length_shakes_the_field() {
    let mut app = editing(&Editor::empty(5, 4));
    // A row is 5 long, a column 4.
    digits(&mut app, "6");
    assert_eq!(app.selected_clue(), Some(clue(TEAL, true, 0)));
    assert_eq!(value(&app, clue(TEAL, true, 0)), 0);
    let (shaking, _) = app.shaking().unwrap();
    assert_eq!(shaking, clue(TEAL, true, 0));
    // The shake shows: the field moves sideways for a moment.
    let field = |app: &App| {
        app.clue_fields()
            .into_iter()
            .find(|(c, _)| *c == clue(TEAL, true, 0))
            .unwrap()
            .1
    };
    let still = field(&app);
    frame(&mut app, 0.04);
    let drawing = frame(&mut app, 0.04);
    assert!(drawing.texts.iter().any(|t| t == "0"));
    settle(&mut app);
    assert!(app.shaking().is_none());
    assert_eq!(field(&app), still);
    // 5 fits a row, not a column.
    digits(&mut app, "5");
    assert_eq!(value(&app, clue(TEAL, true, 0)), 5);
    press_key(&mut app, Key::Backspace);
    press_key(&mut app, Key::Backspace);
    assert_eq!(app.selected_clue(), Some(clue(TEAL, false, 4)));
    digits(&mut app, "5");
    assert_eq!(value(&app, clue(TEAL, false, 4)), 0);
    assert_eq!(app.selected_clue(), Some(clue(TEAL, false, 4)));
}

#[test]
fn two_digit_values_on_long_lines() {
    let mut app = editing(&Editor::empty(12, 12));
    // 1 can start 10, 11 or 12: it waits; the second digit finishes it.
    digits(&mut app, "1");
    assert_eq!(app.selected_clue(), Some(clue(TEAL, true, 0)));
    assert_eq!(value(&app, clue(TEAL, true, 0)), 1);
    digits(&mut app, "2");
    assert_eq!(value(&app, clue(TEAL, true, 0)), 12);
    assert_eq!(app.selected_clue(), Some(clue(TEAL, true, 1)));
    // A pause leaves a single digit.
    // Frames advance at most 0.1 s each.
    digits(&mut app, "1");
    for _ in 0..5 {
        frame(&mut app, 0.1);
    }
    assert_eq!(app.selected_clue(), Some(clue(TEAL, true, 1)));
    frame(&mut app, 0.1);
    frame(&mut app, 0.1);
    assert_eq!(app.selected_clue(), Some(clue(TEAL, true, 2)));
    assert_eq!(value(&app, clue(TEAL, true, 1)), 1);
    // So does enter.
    digits(&mut app, "1");
    press_key(&mut app, Key::Enter);
    assert_eq!(value(&app, clue(TEAL, true, 2)), 1);
    assert_eq!(app.selected_clue(), Some(clue(TEAL, true, 3)));
    // 2 cannot start a value up to 12, nor can 0: they act at once.
    digits(&mut app, "20");
    assert_eq!(value(&app, clue(TEAL, true, 3)), 2);
    assert_eq!(value(&app, clue(TEAL, true, 4)), 0);
    assert_eq!(app.selected_clue(), Some(clue(TEAL, true, 5)));
    // 15 is too long: the field shakes and still waits for a fitting
    // second digit.
    digits(&mut app, "15");
    assert!(app.shaking().is_some());
    assert_eq!(app.selected_clue(), Some(clue(TEAL, true, 5)));
    digits(&mut app, "0");
    assert_eq!(value(&app, clue(TEAL, true, 5)), 10);
    assert_eq!(app.selected_clue(), Some(clue(TEAL, true, 6)));
    // An arrow finishes the waiting digit where it is.
    digits(&mut app, "1");
    press_key(&mut app, Key::Right);
    assert_eq!(value(&app, clue(TEAL, true, 6)), 1);
    assert_eq!(app.selected_clue(), Some(clue(TEAL, true, 7)));
}

#[test]
fn the_digit_strip_still_works_alongside() {
    let mut app = editing(&Editor::empty(5, 5));
    digits(&mut app, "4");
    let e = app.layout().edit.unwrap();
    let keys = e.digit_keys(6);
    app.click(keys[2].center());
    settle(&mut app);
    assert_eq!(value(&app, clue(TEAL, true, 1)), 2);
    assert_eq!(app.selected_clue(), Some(clue(TEAL, true, 2)));
}

#[test]
fn the_clues_of_wla980_1_by_keyboard_alone() {
    let want = sample("wla980-1.txt");
    let mut app = editing(&Editor::empty(6, 6));
    // Orange gets fields once it is the active color.
    let mark: Rect = app.layout().edit.unwrap().palette[1];
    app.click(mark.center());
    settle(&mut app);
    assert_eq!(app.clue_colors(), [TEAL, ORANGE]);
    let line = |color: Color, row: bool| -> String {
        (0..6)
            .map(|i| {
                let v = if row {
                    want.row_clue(color, i)
                } else {
                    want.col_clue(color, i)
                };
                char::from_digit(u32::try_from(v).unwrap(), 10).unwrap()
            })
            .collect()
    };
    for color in [TEAL, ORANGE] {
        digits(&mut app, &line(color, true));
        digits(&mut app, &line(color, false));
    }
    let ed = app.editor().unwrap();
    for color in [TEAL, ORANGE] {
        for i in 0..6 {
            assert_eq!(ed.clue(color, true, i), want.row_clue(color, i));
            assert_eq!(ed.clue(color, false, i), want.col_clue(color, i));
        }
    }
}

#[test]
fn digits_do_nothing_while_playing() {
    let mut app = open(&sample("wln006-2.txt"));
    let before = app.board().unwrap().clone();
    digits(&mut app, "123");
    press_key(&mut app, Key::Right);
    press_key(&mut app, Key::Enter);
    settle(&mut app);
    assert_eq!(app.board().unwrap(), &before);
}
