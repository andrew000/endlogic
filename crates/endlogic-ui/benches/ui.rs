//! Benchmarks of the trainer: one frame drawn into a painter that does
//! nothing, and the layout alone.
//!
//! Run with `just bench`; prints Markdown tables. The app starts from a
//! fixed seed, so runs are comparable.

mod measure;

use std::fmt::Write as _;
use std::time::Instant;

use endlogic_core::code::encode;
use endlogic_core::format::parse;
use endlogic_ui::{
    App, Budgets, CodeKind, Config, Insets, Lang, Layout, LayoutInput, Mode, Painter, Point, Rect,
    Rgba, SETTING_DIFFICULTY,
};
use measure::{Table, show, time};

/// A painter that draws nothing.
struct Blank;

impl Painter for Blank {
    fn fill_rect(&mut self, _: Rect, _: f32, _: Rgba) {}
    fn stroke_rect(&mut self, _: Rect, _: f32, _: f32, _: Rgba) {}
    fn line(&mut self, _: Point, _: Point, _: f32, _: Rgba) {}
    fn fill_circle(&mut self, _: Point, _: f32, _: Rgba) {}
    fn stroke_circle(&mut self, _: Point, _: f32, _: f32, _: Rgba) {}
    fn fill_polygon(&mut self, _: &[Point], _: Rgba) {}
    fn polyline(&mut self, _: &[Point], _: f32, _: Rgba) {}
    fn push_clip(&mut self, _: Rect) {}
    fn pop_clip(&mut self) {}
}

fn config(width: f32, height: f32, touch: bool) -> Config {
    Config {
        width,
        height,
        insets: Insets::default(),
        entropy: 0x5eed,
        settings: vec![(SETTING_DIFFICULTY.to_owned(), "expert".to_owned())],
        system_language: "en-US".to_owned(),
        code: None,
        touch,
        budgets: Budgets::default(),
    }
}

/// Runs frames until the app is idle.
fn settle(app: &mut App) {
    for _ in 0..10_000 {
        if !app.frame(0.05, &mut Blank) {
            return;
        }
    }
    panic!("the app never became idle");
}

/// A 16x16 puzzle for the editor: a frame of locks, clues and 24 pieces.
fn editor_puzzle() -> endlogic_core::Puzzle {
    let mut text = String::from("endlogic 1\ngrid\n");
    for r in 0..16 {
        for c in 0..16 {
            text.push(if (r + c) % 7 == 0 { '#' } else { '.' });
        }
        text.push('\n');
    }
    let clues: Vec<String> = (0..16).map(|i| (i % 9).to_string()).collect();
    writeln!(text, "rows {}\ncols {}", clues.join(" "), clues.join(" ")).unwrap();
    let shapes = [
        "###\n#..",
        "##\n##",
        "#####",
        ".#.\n###",
        "##.\n.##",
        "#..\n###\n..#",
    ];
    for i in 0..24 {
        writeln!(text, "piece\n{}", shapes[i % shapes.len()]).unwrap();
    }
    parse(&text).unwrap()
}

const SIZES: [(&str, f32, f32, bool); 4] = [
    ("wide 1366x768", 1366.0, 768.0, false),
    ("wide 1920x1080", 1920.0, 1080.0, false),
    ("low wide 844x390", 844.0, 390.0, false),
    ("tall 390x844, touch", 390.0, 844.0, true),
];

fn frames() {
    let mut table = Table::new(
        "Trainer: one idle frame into a painter that draws nothing",
        &["scene", "mean", "best"],
    );
    for (name, w, h, touch) in SIZES {
        let mut app = App::new(config(w, h, touch));
        settle(&mut app);
        let t = time(|| app.frame(0.0, &mut Blank));
        table.row(vec![format!("expert, {name}"), show(t.mean), show(t.best)]);
    }
    let code = encode(&editor_puzzle()).unwrap();
    for (name, w, h, touch) in [SIZES[0], SIZES[3]] {
        let mut config = config(w, h, touch);
        config.code = Some((CodeKind::Edit, code.clone()));
        let mut app = App::new(config);
        settle(&mut app);
        assert!(app.is_editing());
        let t = time(|| app.frame(0.0, &mut Blank));
        table.row(vec![
            format!("editor 16x16, 24 pieces, {name}"),
            show(t.mean),
            show(t.best),
        ]);
    }
    table.print();
}

fn layouts() {
    let mut table = Table::new("Trainer: layout alone", &["input", "mean", "best"]);
    for (name, w, h, touch) in SIZES {
        let input = LayoutInput {
            width: w,
            height: h,
            insets: Insets::default(),
            touch,
            lang: Lang::En,
            grid_width: 8,
            grid_height: 8,
            pieces: 8,
            max_piece_side: 5,
            mode: Mode::Play,
        };
        let t = time(|| Layout::compute(&input));
        table.row(vec![format!("expert, {name}"), show(t.mean), show(t.best)]);
    }
    for (name, w, h, touch) in [SIZES[0], SIZES[3]] {
        let input = LayoutInput {
            width: w,
            height: h,
            insets: Insets::default(),
            touch,
            lang: Lang::En,
            grid_width: 16,
            grid_height: 16,
            pieces: 25,
            max_piece_side: 5,
            mode: Mode::Edit,
        };
        let t = time(|| Layout::compute(&input));
        table.row(vec![
            format!("editor 16x16, {name}"),
            show(t.mean),
            show(t.best),
        ]);
    }
    table.print();
}

fn main() {
    let start = Instant::now();
    println!("## endlogic-ui");
    frames();
    layouts();
    println!("\nendlogic-ui benchmarks took {}", show(start.elapsed()));
}
