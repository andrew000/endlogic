//! Behaviour of the trainer, driven by synthetic events and a recording
//! painter: no browser and no window.

use std::fmt::Write as _;

use endlogic_core::code::encode;
use endlogic_core::format::parse;
use endlogic_core::generator::Difficulty;
use endlogic_core::solver::{DEFAULT_NODE_BUDGET, search};
use endlogic_core::{Puzzle, Solution};
use endlogic_ui::{
    Align, App, Arrangement, BAND, COUNTER_GAP, Config, CounterMode, CounterView, Cursor, Effect,
    Event, Font, Insets, Key, Lang, Layout, LayoutInput, Line, LineState, MIN_SQUARE, MIN_TARGET,
    Painter, Point, Pointer, PointerKind, Rect, Rgba, TOUCH_DRAG_THRESHOLD, TRAY_MIN_SCALE,
    TRAY_SCALE, Text, counter_view,
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
        insets: Insets::default(),
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
    assert!(
        texts(&recorder)
            .iter()
            .any(|t| t == "Couldn't copy the link")
    );
    let mut ua = config_with_lang("uk-UA");
    ua.code = Some(code);
    let mut app = App::new(ua);
    app.handle(Event::CopyFinished(false));
    let mut recorder = Recorder::default();
    app.frame(0.2, &mut recorder);
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
    assert_eq!(layout.cards.len(), input.pieces, "{what}: cards");
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
    for button in [layout.reset, layout.counters, layout.new] {
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
    // No top or bottom row: every control is beside the board.
    let block = layout.board_block();
    let mut controls = vec![
        layout.lang,
        layout.share,
        layout.reset,
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
        match counter_view(line(0, 0), area, row, cell) {
            CounterView::Empty { center, radius } => {
                assert!(area.contains_rect(Rect::centered(center, 2.0 * radius, 2.0 * radius)));
            }
            other => panic!("zero clue: {other:?}"),
        }
        for n in 1..=16 {
            let (marks, ticks) = marks(counter_view(line(0, n), area, row, cell));
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
        counter_view(line(0, 16), row_area(cell), true, cell),
        CounterView::Number
    );
    // Small clues still get marks, and every mark is at least 3 px.
    for n in 1..=16 {
        if let CounterView::Marks {
            marks,
            ticks: false,
        } = counter_view(line(0, n), row_area(cell), true, cell)
        {
            assert!(marks.iter().all(|m| m.w >= MIN_SQUARE), "clue {n}");
        }
    }
    let (marks, _) = marks(counter_view(line(0, 6), col_area(60.0), false, 60.0));
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
