//! Behaviour of the trainer, driven by synthetic events and a recording
//! painter: no browser and no window.

use endlogic_core::code::encode;
use endlogic_core::format::parse;
use endlogic_core::generator::Difficulty;
use endlogic_core::solver::{DEFAULT_NODE_BUDGET, search};
use endlogic_core::{Puzzle, Solution};
use endlogic_ui::{
    Align, App, Config, CounterMode, Cursor, Effect, Event, Font, Key, Lang, Layout, LayoutInput,
    LineState, MIN_TARGET, Painter, Point, Pointer, PointerKind, Rect, Rgba, Text,
};

// ---- recording painter ----

#[derive(Debug, Clone, PartialEq)]
enum Call {
    FillRect(Rect, Rgba),
    StrokeRect(Rect, Rgba),
    Line(Point, Point, Rgba),
    FillCircle(Point, Rgba),
    StrokeCircle(Point, Rgba),
    Polygon(Vec<Point>, Rgba),
    Text(String, Point, Rgba),
    PushClip(Rect),
    PopClip,
}

#[derive(Debug, Default)]
struct Recorder {
    calls: Vec<Call>,
    depth: i32,
}

impl Painter for Recorder {
    fn fill_rect(&mut self, rect: Rect, _radius: f32, color: Rgba) {
        self.calls.push(Call::FillRect(rect, color));
    }
    fn stroke_rect(&mut self, rect: Rect, _radius: f32, _width: f32, color: Rgba) {
        self.calls.push(Call::StrokeRect(rect, color));
    }
    fn line(&mut self, from: Point, to: Point, _width: f32, color: Rgba) {
        self.calls.push(Call::Line(from, to, color));
    }
    fn fill_circle(&mut self, center: Point, _radius: f32, color: Rgba) {
        self.calls.push(Call::FillCircle(center, color));
    }
    fn stroke_circle(&mut self, center: Point, _radius: f32, _width: f32, color: Rgba) {
        self.calls.push(Call::StrokeCircle(center, color));
    }
    fn fill_polygon(&mut self, points: &[Point], color: Rgba) {
        self.calls.push(Call::Polygon(points.to_vec(), color));
    }
    fn text(&mut self, text: &str, anchor: Point, _font: Font, _align: Align, color: Rgba) {
        self.calls.push(Call::Text(text.to_owned(), anchor, color));
    }
    fn text_width(&mut self, text: &str, font: Font) -> f32 {
        // A stand-in: every character half an em wide.
        f32::from(u16::try_from(text.chars().count()).unwrap()) * font.size * 0.55
    }
    fn push_clip(&mut self, rect: Rect) {
        self.depth += 1;
        self.calls.push(Call::PushClip(rect));
    }
    fn pop_clip(&mut self) {
        self.depth -= 1;
        assert!(self.depth >= 0, "pop_clip without push_clip");
        self.calls.push(Call::PopClip);
    }
}

const ERROR: Rgba = Rgba::hex(0xFF_4D5E);
const BRIGHT: Rgba = Rgba::hex(0x3F_F5D6);
const MUTED: Rgba = Rgba::hex(0x7E_8886);
const SHARE: &str = "https://example.org/endlogic/#p=";
const MOUSE: u32 = 1;
const FINGER: u32 = 7;

// ---- helpers ----

fn config(width: f32, height: f32) -> Config {
    Config {
        width,
        height,
        entropy: 0x5eed,
        settings: Vec::new(),
        system_language: "en-US".to_owned(),
        code: None,
        touch: false,
        share_prefix: SHARE.to_owned(),
    }
}

fn sample(name: &str) -> Puzzle {
    let path = format!("{}/../../puzzles/{name}", env!("CARGO_MANIFEST_DIR"));
    parse(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// Runs frames until the app needs no more; returns the last drawing.
fn settle(app: &mut App) -> Recorder {
    for _ in 0..2000 {
        let mut recorder = Recorder::default();
        let busy = app.frame(0.05, &mut recorder);
        assert_eq!(recorder.depth, 0, "unbalanced clips");
        if !busy {
            return recorder;
        }
    }
    panic!("the app never became idle");
}

fn open(puzzle: &Puzzle) -> App {
    let mut config = config(1366.0, 768.0);
    config.code = Some(encode(puzzle).unwrap());
    let mut app = App::new(config);
    settle(&mut app);
    assert!(!app.is_generating());
    app
}

fn generated(config: Config) -> App {
    let mut app = App::new(config);
    assert!(app.is_generating());
    settle(&mut app);
    assert!(app.board().is_some());
    app
}

fn pointer(id: u32, p: Point, kind: PointerKind) -> Pointer {
    Pointer {
        id,
        x: p.x,
        y: p.y,
        kind,
    }
}

fn card(app: &App, i: usize) -> Point {
    app.layout().cards[i].center()
}

/// The center of board cell `(row, col)`.
fn cell(app: &App, row: usize, col: usize) -> Point {
    app.layout().cell_rect(row, col).center()
}

/// Where to release a piece pressed at `press` so that it lands with its
/// top-left corner on `(row, col)`.
fn release_point(
    app: &App,
    i: usize,
    press: Point,
    row: usize,
    col: usize,
    kind: PointerKind,
) -> Point {
    let rect = app.piece_rect(i);
    let layout = app.layout();
    let shape = app.board().unwrap().shape(i);
    let size = |n: usize| f32::from(u8::try_from(n).unwrap()) * layout.cell;
    let grab = Point::new((press.x - rect.x) / rect.w, (press.y - rect.y) / rect.h);
    let lift = if kind == PointerKind::Touch {
        1.5 * layout.cell
    } else {
        0.0
    };
    Point::new(
        layout.cells.x + size(col) + grab.x * size(shape.width()),
        layout.cells.y + size(row) + grab.y * size(shape.height()) + lift,
    )
}

/// Input helpers as methods, so arguments may read the app.
trait Drive {
    fn hover(&mut self, p: Point);
    fn click(&mut self, p: Point);
    fn tap(&mut self, p: Point);
    /// Presses at `from`, drags past the threshold and moves to `to`,
    /// without releasing.
    fn start_drag(&mut self, from: Point, to: Point, kind: PointerKind) -> u32;
    fn drag(&mut self, from: Point, to: Point, kind: PointerKind);
    /// Turns tray piece `i` with `R` until it has `turns`, then drags it
    /// from its card onto `(row, col)`.
    fn place(&mut self, i: usize, turns: u8, row: usize, col: usize, kind: PointerKind);
    fn play(&mut self, solution: &Solution);
}

impl Drive for App {
    fn hover(&mut self, p: Point) {
        self.handle(Event::PointerMove(pointer(MOUSE, p, PointerKind::Mouse)));
    }

    fn click(&mut self, p: Point) {
        self.handle(Event::PointerDown(pointer(MOUSE, p, PointerKind::Mouse)));
        self.handle(Event::PointerUp(pointer(MOUSE, p, PointerKind::Mouse)));
    }

    fn tap(&mut self, p: Point) {
        self.handle(Event::PointerDown(pointer(FINGER, p, PointerKind::Touch)));
        self.handle(Event::PointerUp(pointer(FINGER, p, PointerKind::Touch)));
    }

    fn start_drag(&mut self, from: Point, to: Point, kind: PointerKind) -> u32 {
        let id = if kind == PointerKind::Mouse {
            MOUSE
        } else {
            FINGER
        };
        self.handle(Event::PointerDown(pointer(id, from, kind)));
        let nudge = Point::new(from.x + 12.0, from.y + 3.0);
        self.handle(Event::PointerMove(pointer(id, nudge, kind)));
        self.handle(Event::PointerMove(pointer(id, to, kind)));
        id
    }

    fn drag(&mut self, from: Point, to: Point, kind: PointerKind) {
        let id = self.start_drag(from, to, kind);
        self.handle(Event::PointerUp(pointer(id, to, kind)));
        settle(self);
    }

    fn place(&mut self, i: usize, turns: u8, row: usize, col: usize, kind: PointerKind) {
        let from = card(self, i);
        self.hover(from);
        while self.board().unwrap().piece(i).turns != turns {
            self.handle(Event::Key(Key::R));
        }
        settle(self);
        let to = release_point(self, i, from, row, col, kind);
        self.drag(from, to, kind);
    }

    fn play(&mut self, solution: &Solution) {
        for (i, placement) in solution.placements.iter().enumerate() {
            let turns = u8::try_from(placement.rotation.quarter_turns()).unwrap();
            self.place(i, turns, placement.row, placement.col, PointerKind::Mouse);
            assert_eq!(
                self.board().unwrap().piece(i).at,
                Some((placement.row, placement.col)),
                "piece {i}"
            );
        }
    }
}

fn at(app: &App, i: usize) -> Option<(usize, usize)> {
    app.board().unwrap().piece(i).at
}

fn turns(app: &App, i: usize) -> u8 {
    app.board().unwrap().piece(i).turns
}

fn texts(recorder: &Recorder) -> Vec<String> {
    recorder
        .calls
        .iter()
        .filter_map(|c| match c {
            Call::Text(t, _, _) => Some(t.clone()),
            _ => None,
        })
        .collect()
}

fn count_color(recorder: &Recorder, color: Rgba) -> usize {
    recorder
        .calls
        .iter()
        .filter(|c| match c {
            Call::FillRect(_, k)
            | Call::StrokeRect(_, k)
            | Call::Line(_, _, k)
            | Call::FillCircle(_, k)
            | Call::StrokeCircle(_, k)
            | Call::Polygon(_, k)
            | Call::Text(_, _, k) => *k == color,
            Call::PushClip(_) | Call::PopClip => false,
        })
        .count()
}

/// A 3×2 board with a straight tromino and a monomino: lying down the
/// tromino fits, standing up it cannot.
fn strip() -> Puzzle {
    parse("endlogic 1\ngrid\n...\n...\nrows 3 1\ncols 2 1 1\npiece\n###\npiece\n#\n").unwrap()
}

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
fn a_piece_that_cannot_turn_in_place_goes_to_the_tray() {
    let mut app = open(&strip());
    app.place(0, 0, 0, 0, PointerKind::Mouse);
    assert_eq!(at(&app, 0), Some((0, 0)));
    app.hover(cell(&app, 0, 1));
    app.handle(Event::Key(Key::R));
    assert_eq!(at(&app, 0), None);
    assert_eq!(turns(&app, 0), 1);
    settle(&mut app);
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
    assert!(app.take_effects().contains(&Effect::SetCode(new)));
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
    config.code = Some(format!(" {code} "));
    let mut app = App::new(config);
    assert!(!app.is_generating());
    assert_eq!(app.board().unwrap().puzzle(), &puzzle);
    assert_eq!(app.code(), Some(code.as_str()));
    assert_eq!(app.puzzle_id().unwrap(), format!("Δ-H-{}", &code[..6]));
    assert_eq!(app.difficulty(), Difficulty::Hard);
    assert!(app.take_effects().contains(&Effect::SetCode(code)));
    assert!(app.toast().is_none());
    let drawing = settle(&mut app);
    assert!(texts(&drawing).iter().any(|t| t.starts_with("Δ-H-AWYAG")));
}

#[test]
fn a_bad_code_shows_a_message_and_generates() {
    let mut config = config(1366.0, 768.0);
    config.code = Some("not-a-code!".to_owned());
    let mut app = App::new(config);
    let toast = app.toast().unwrap();
    assert_eq!((toast.text, toast.error), (Text::InvalidLink, true));
    assert!(app.is_generating());
    let drawing = settle(&mut app);
    assert!(app.board().is_some());
    assert!(app.toast().is_none(), "toast faded");
    drop(drawing);
    let mut app = App::new(Config {
        code: Some("AQ".to_owned()),
        ..config_with_lang("uk-UA")
    });
    let mut recorder = Recorder::default();
    app.frame(0.0, &mut recorder);
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
    app.handle(Event::CodeChanged(code.clone()));
    assert_eq!(app.code(), Some(code.as_str()));
    app.take_effects();
    app.handle(Event::CodeChanged("garbage".to_owned()));
    assert_eq!(app.code(), Some(code.as_str()));
    assert_eq!(app.toast().unwrap().text, Text::InvalidLink);
    assert_eq!(app.take_effects(), vec![Effect::SetCode(code)]);
    assert_ne!(first, app.code().unwrap());
}

#[test]
fn generated_puzzles_publish_their_code() {
    let mut app = App::new(config(1366.0, 768.0));
    assert_eq!(app.take_effects(), Vec::new());
    let drawing = settle(&mut app);
    let code = app.code().unwrap().to_owned();
    assert!(app.take_effects().contains(&Effect::SetCode(code.clone())));
    assert_eq!(app.difficulty(), Difficulty::Normal);
    assert_eq!(app.puzzle_id().unwrap(), format!("Δ-N-{}", &code[..6]));
    assert!(!texts(&drawing).iter().any(|t| t == "Generating…"));

    app.click(app.layout().new.center());
    let mut recorder = Recorder::default();
    assert!(app.is_generating());
    // The first frame may already finish; the indicator shows while it runs.
    let busy = app.frame(0.0, &mut recorder);
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
        .filter(|e| matches!(e, Effect::CopyText(_)))
        .collect();
    assert_eq!(copied, vec![Effect::CopyText(format!("{SHARE}{code}"))]);
    assert_eq!(app.toast().unwrap().text, Text::LinkCopied);
    let drawing = settle(&mut app);
    drop(drawing);
    assert!(app.toast().is_none());
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
    assert_eq!(app.puzzle_id().unwrap().chars().nth(2), Some('E'));
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
    assert!(
        !app.frame(0.016, &mut Recorder::default()),
        "a held piece does not animate"
    );
    app.handle(Event::Resize {
        width: 500.0,
        height: 900.0,
    });
    assert!(!app.layout().wide);
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

// ---- layout ----

const SIZES: [(f32, f32); 4] = [
    (1366.0, 768.0),
    (1920.0, 1080.0),
    (390.0, 844.0),
    (844.0, 390.0),
];

fn check_layout(layout: &Layout, w: f32, h: f32, what: &str) {
    let screen = Rect::new(0.0, 0.0, w, h);
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
    for (name, rect) in layout.targets() {
        assert!(
            rect.w >= MIN_TARGET - 0.01 && rect.h >= MIN_TARGET - 0.01,
            "{what}: {name} {rect:?} too small to tap"
        );
    }
    if let Some(panel) = layout.panel {
        for rect in [
            Some(layout.title),
            Some(layout.puzzle_id),
            layout.subtitle,
            layout.help,
        ]
        .into_iter()
        .flatten()
        {
            assert!(
                panel.contains_rect(rect),
                "{what}: {rect:?} outside the panel"
            );
        }
    }
    assert!(layout.frame.contains_rect(layout.cells), "{what}: cells");
    for card in &layout.cards {
        assert!(
            layout.tray.contains_rect(*card),
            "{what}: card outside the tray"
        );
    }
    for (k, a) in layout.cards.iter().enumerate() {
        for b in &layout.cards[k + 1..] {
            assert!(!a.intersects(*b), "{what}: cards overlap");
        }
    }
    assert!(screen.contains_rect(layout.banner), "{what}: banner");
    assert!(layout.banner.contains_rect(layout.next), "{what}: next");
    assert!(layout.cell >= 20.0, "{what}: cell {}", layout.cell);
    assert_eq!(layout.wide, w > h, "{what}: arrangement");
}

#[test]
fn layout_fits_every_size_and_difficulty() {
    for (w, h) in SIZES {
        for difficulty in Difficulty::ALL {
            let params = difficulty.params();
            for lang in [Lang::En, Lang::Ua] {
                for touch in [false, true] {
                    // The largest tray this difficulty can produce.
                    for side in [3, params.piece_size.max] {
                        let input = LayoutInput {
                            width: w,
                            height: h,
                            touch,
                            lang,
                            grid_width: params.width,
                            grid_height: params.height,
                            pieces: params.pieces.max,
                            max_piece_side: side,
                        };
                        let what =
                            format!("{w}x{h} {difficulty:?} {lang:?} touch={touch} side={side}");
                        let layout = Layout::compute(&input);
                        check_layout(&layout, w, h, &what);
                        if touch || !layout.wide {
                            assert!(layout.help.is_none(), "{what}: help");
                        }
                        if !touch && layout.wide && h >= 700.0 {
                            assert!(layout.help.is_some(), "{what}: no help");
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn layout_of_real_puzzles_fits() {
    for (w, h) in SIZES {
        for (k, difficulty) in Difficulty::ALL.into_iter().enumerate() {
            let mut config = config(w, h);
            config.touch = w < h;
            config.entropy = u64::try_from(k).unwrap();
            config.settings = vec![("difficulty".to_owned(), difficulty.name().to_owned())];
            let mut app = generated(config);
            check_layout(app.layout(), w, h, &format!("{w}x{h} {difficulty:?}"));
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
        });
        assert!(
            app.layout().cell >= 24.0,
            "{w}x{h}: cell {}",
            app.layout().cell
        );
    }
}
