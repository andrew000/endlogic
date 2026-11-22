//! Reading a puzzle from a screenshot into the editor: from any tab, with a
//! question before replacing a puzzle, doubtful places marked, failures
//! reported. The pictures are drawn in memory in the game's style.

mod common;

use common::*;
use endlogic_core::Puzzle;
use endlogic_core::code::encode;
use endlogic_core::format::parse;
use endlogic_ui::{App, CodeKind, Doubt, Editor, Effect, Event, Place, Reason, Rect, Text, text};
use endlogic_vision::synth::{Scene, draw};

/// The event carrying a screen of `puzzle` as the game draws it.
fn screen_of(scene: &Scene) -> Event {
    Event::Image {
        width: scene.width,
        height: scene.height,
        rgba: draw(scene),
    }
}

fn screen(puzzle: &Puzzle) -> Event {
    screen_of(&Scene::new(puzzle.clone(), 1280, 720))
}

fn press(app: &mut App, rect: Rect) {
    app.click(rect.center());
    settle(app);
}

fn edit(app: &App) -> endlogic_ui::EditLayout {
    app.layout().edit.unwrap()
}

/// The effects other than cursor changes.
fn effects(app: &mut App) -> Vec<Effect> {
    app.take_effects()
        .into_iter()
        .filter(|e| !matches!(e, Effect::SetCursor(_)))
        .collect()
}

/// An app in the editor with `puzzle`, idle.
fn editing(puzzle: &Puzzle) -> App {
    let mut config = config(1366.0, 768.0);
    config.code = Some((CodeKind::Edit, encode(puzzle).unwrap()));
    let mut app = App::new(config);
    settle(&mut app);
    app
}

fn blank() -> App {
    editing(&Editor::default().puzzle())
}

#[test]
fn a_screenshot_fills_a_blank_editor() {
    let mut app = blank();
    let puzzle = sample("wla980-1.txt");
    app.handle(screen(&puzzle));
    // The first frame says it reads; the next one reads.
    assert!(app.is_recognizing());
    let first = frame(&mut app, 0.016);
    assert!(texts(&first).iter().any(|t| t == "Recognizing…"));
    assert!(app.editor().unwrap().is_blank());
    frame(&mut app, 0.016);
    assert!(!app.is_recognizing());
    assert_eq!(app.editor().unwrap().puzzle(), puzzle);
    assert_eq!(app.doubts(), []);
    assert_eq!(
        app.toast_label().as_deref(),
        Some("Recognized: 6×6, 6 pieces, 2 colors")
    );
    // The code of the read puzzle is published like any other.
    let code = encode(&puzzle).unwrap();
    assert!(
        app.take_effects()
            .contains(&Effect::SetCode(CodeKind::Edit, code))
    );
}

#[test]
fn the_toast_counts_in_ukrainian() {
    assert_eq!(
        endlogic_ui::recognized(endlogic_ui::Lang::Ua, 6, 6, 7, 2),
        "Розпізнано: 6×6, фігур: 7, кольорів: 2"
    );
    assert_eq!(
        endlogic_ui::recognized(endlogic_ui::Lang::En, 5, 5, 1, 1),
        "Recognized: 5×5, 1 piece, 1 color"
    );
}

#[test]
fn a_screenshot_pasted_while_playing_opens_the_editor() {
    let mut app = generated(config(1366.0, 768.0));
    assert!(!app.is_custom());
    let puzzle = sample("wlh9000-1.txt");
    app.handle(screen(&puzzle));
    assert!(app.is_editing());
    settle(&mut app);
    // The editor had the default blank puzzle.
    assert_eq!(app.editor().unwrap().puzzle(), puzzle);
}

#[test]
fn a_puzzle_in_the_editor_is_replaced_only_after_yes() {
    let old = sample("wln006-2.txt");
    let new = sample("wla980-2.txt");
    let mut app = editing(&old);
    app.handle(screen(&new));
    settle(&mut app);
    assert!(app.asks_replace());
    assert_eq!(app.editor().unwrap().puzzle(), old);
    let drawing = settle(&mut app);
    assert!(
        texts(&drawing)
            .iter()
            .any(|t| t == "Replace the current puzzle?")
    );
    let prompt = edit(&app).prompt(app.lang(), app.layout().font);
    // While it asks, nothing else reacts.
    let at = edit(&app).clear;
    press(&mut app, at);
    assert!(!app.clear_armed());
    // No keeps the puzzle.
    press(&mut app, prompt.no);
    assert!(!app.asks_replace());
    assert_eq!(app.editor().unwrap().puzzle(), old);
    // Yes replaces it.
    app.handle(screen(&new));
    settle(&mut app);
    press(&mut app, prompt.yes);
    assert!(!app.asks_replace());
    assert_eq!(app.editor().unwrap().puzzle(), new);
}

#[test]
fn failures_leave_the_editor_alone_and_say_why() {
    let old = sample("wln006-2.txt");
    let puzzle = sample("wla980-1.txt");
    let base = Scene::new(puzzle, 1280, 720);
    let mut numbers = base.clone();
    numbers.numbers = true;
    let mut placed = base.clone();
    placed.on_grid = Some(0);
    let mut nothing = Scene::new(
        parse("endlogic 1\ngrid\n.\nrows 0\ncols 0\n").unwrap(),
        1280,
        720,
    );
    nothing.pitch = 0.0;
    let cases = [
        (screen_of(&numbers), Text::NumbersMode),
        (screen_of(&placed), Text::PiecesOnBoard),
        (screen_of(&nothing), Text::NotGame),
        (
            Event::Image {
                width: 100,
                height: 100,
                rgba: vec![0; 100 * 100 * 4],
            },
            Text::Unreadable,
        ),
        (
            Event::Image {
                width: 100,
                height: 100,
                rgba: vec![0; 12],
            },
            Text::Unreadable,
        ),
        (Event::ImageFailed, Text::Unreadable),
    ];
    for (event, message) in cases {
        let mut app = editing(&old);
        app.handle(event);
        frame(&mut app, 0.016);
        frame(&mut app, 0.016);
        assert!(!app.asks_replace());
        assert_eq!(app.editor().unwrap().puzzle(), old);
        let toast = app.toast().expect("a message");
        assert!(toast.error);
        assert_eq!(toast.text, message);
    }
    // Every message has both languages.
    for t in [
        Text::NotGame,
        Text::NumbersMode,
        Text::PiecesOnBoard,
        Text::Unreadable,
    ] {
        assert_ne!(
            text(endlogic_ui::Lang::En, t),
            text(endlogic_ui::Lang::Ua, t)
        );
    }
}

#[test]
fn the_button_asks_the_shell_for_a_picture() {
    let mut app = blank();
    app.take_effects();
    let at = edit(&app).import;
    press(&mut app, at);
    assert_eq!(effects(&mut app), [Effect::PickImage]);
    // On a phone too.
    let mut config = config(390.0, 844.0);
    config.code = Some((CodeKind::Edit, encode(&Editor::default().puzzle()).unwrap()));
    config.touch = true;
    let mut app = App::new(config);
    settle(&mut app);
    app.take_effects();
    let import = edit(&app).import;
    assert!(import.right() <= 390.0 && import.bottom() <= 844.0);
    press(&mut app, import);
    assert_eq!(effects(&mut app), [Effect::PickImage]);
}

/// A puzzle whose two flat pieces could each be a single cell or a 2×2
/// square: both get doubts.
fn ambiguous() -> Scene {
    let text = "endlogic 1\ngrid\n....\n....\n....\n....\nrows 2 2 1 0\ncols 2 2 1 0\npiece\n##\n##\npiece\n#\n";
    let mut scene = Scene::new(parse(text).unwrap(), 1280, 720);
    scene.shaded = false;
    scene
}

#[test]
fn doubtful_places_are_marked_until_changed_or_closed() {
    let mut app = blank();
    app.handle(screen_of(&ambiguous()));
    let drawing = settle(&mut app);
    let unclear = |i| Doubt {
        place: Place::Piece(i),
        reason: Reason::Unclear,
    };
    assert_eq!(app.doubts(), [unclear(0), unclear(1)]);
    assert!(
        texts(&drawing)
            .iter()
            .any(|t| t.starts_with("Check the highlighted spots"))
    );
    assert!(count_color(&drawing, endlogic_ui::Rgba::hex(0xFF_D51F)) >= 2);
    // Changing a piece settles its doubt.
    let at = app.card_of(1).unwrap();
    press(&mut app, at);
    let done = edit(&app).piece_editor(app.draft().unwrap().side).done;
    press(&mut app, done);
    assert_eq!(app.doubts(), [unclear(0)]);
    // Closing the marks drops the rest.
    let at = edit(&app).status;
    press(&mut app, at);
    assert_eq!(app.doubts(), []);
    let drawing = settle(&mut app);
    assert!(
        !texts(&drawing)
            .iter()
            .any(|t| t.starts_with("Check the highlighted spots"))
    );
}

#[test]
fn doubts_follow_the_pieces() {
    let mut app = blank();
    app.handle(screen_of(&ambiguous()));
    settle(&mut app);
    // Deleting the first piece moves the second one's doubt up.
    let at = app.card_of(0).unwrap();
    press(&mut app, at);
    let delete = edit(&app).piece_editor(app.draft().unwrap().side).delete;
    press(&mut app, delete);
    assert_eq!(
        app.doubts(),
        [Doubt {
            place: Place::Piece(0),
            reason: Reason::Unclear,
        }]
    );
}

#[test]
fn the_picture_is_not_kept() {
    let mut app = blank();
    app.handle(screen(&sample("wlh9000-1.txt")));
    settle(&mut app);
    // Only codes and settings leave the app, never pixels; the code is the
    // puzzle's.
    for effect in app.take_effects() {
        match effect {
            Effect::SetCode(_, code) | Effect::SaveSetting { value: code, .. } => {
                assert!(code.len() < 200, "{code}");
            }
            Effect::SetCursor(_) => {}
            other => panic!("{other:?}"),
        }
    }
}
