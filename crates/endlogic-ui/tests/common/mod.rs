//! Helpers shared by the trainer tests: a recording painter, puzzles and
//! input drivers.
#![allow(dead_code)]

use endlogic_core::code::encode;
use endlogic_core::format::parse;
use endlogic_core::{Puzzle, Solution};
use endlogic_ui::{
    Align, App, Budgets, CodeKind, Config, Event, Font, Insets, Key, Painter, Point, Pointer,
    PointerKind, Rect, Rgba,
};

pub mod golden;

// ---- recording painter ----

#[derive(Debug, Clone, PartialEq)]
pub enum Call {
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
pub struct Recorder {
    pub calls: Vec<Call>,
    pub depth: i32,
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

pub const ERROR: Rgba = Rgba::hex(0xFF_4D5E);
pub const BRIGHT: Rgba = Rgba::hex(0x3F_F5D6);
pub const MUTED: Rgba = Rgba::hex(0x7E_8886);
pub const MOUSE: u32 = 1;
pub const FINGER: u32 = 7;

// ---- helpers ----

pub fn config(width: f32, height: f32) -> Config {
    Config {
        width,
        height,
        insets: Insets::default(),
        entropy: 0x5eed,
        settings: Vec::new(),
        system_language: "en-US".to_owned(),
        code: None,
        touch: false,
        budgets: Budgets::default(),
    }
}

pub fn sample(name: &str) -> Puzzle {
    let path = format!("{}/../../puzzles/{name}", env!("CARGO_MANIFEST_DIR"));
    parse(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// Runs frames until the app needs no more; returns the last drawing.
pub fn settle(app: &mut App) -> Recorder {
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

pub fn open(puzzle: &Puzzle) -> App {
    let mut config = config(1366.0, 768.0);
    config.code = Some((CodeKind::Play, encode(puzzle).unwrap()));
    let mut app = App::new(config);
    settle(&mut app);
    assert!(!app.is_generating());
    app
}

pub fn generated(config: Config) -> App {
    let mut app = App::new(config);
    assert!(app.is_generating());
    settle(&mut app);
    assert!(app.board().is_some());
    app
}

pub fn pointer(id: u32, p: Point, kind: PointerKind) -> Pointer {
    Pointer {
        id,
        x: p.x,
        y: p.y,
        kind,
    }
}

pub fn card(app: &App, i: usize) -> Point {
    app.card_of(i).unwrap().center()
}

/// The center of board cell `(row, col)`.
pub fn cell(app: &App, row: usize, col: usize) -> Point {
    app.layout().cell_rect(row, col).center()
}

/// Where to release a piece pressed at `press` so that it lands with its
/// top-left corner on `(row, col)`.
pub fn release_point(
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
pub trait Drive {
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

pub fn at(app: &App, i: usize) -> Option<(usize, usize)> {
    app.board().unwrap().piece(i).at
}

pub fn turns(app: &App, i: usize) -> u8 {
    app.board().unwrap().piece(i).turns
}

pub fn texts(recorder: &Recorder) -> Vec<String> {
    recorder
        .calls
        .iter()
        .filter_map(|c| match c {
            Call::Text(t, _, _) => Some(t.clone()),
            _ => None,
        })
        .collect()
}

pub fn count_color(recorder: &Recorder, color: Rgba) -> usize {
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
pub fn strip() -> Puzzle {
    parse("endlogic 1\ngrid\n...\n...\nrows 3 1\ncols 2 1 1\npiece\n###\npiece\n#\n").unwrap()
}
