//! Benchmarks of the recognizer on screens drawn in memory in the style of
//! the game, at the usual screen sizes.
//!
//! Run with `just bench`; prints a Markdown table. The screens come from
//! fixed seeds, so runs are comparable.

mod measure;

use endlogic_core::format::parse;
use endlogic_core::generator::{Difficulty, generate};
use endlogic_vision::synth::{Scene, draw};
use endlogic_vision::{Image, recognize};
use measure::{Table, show, time};

fn main() {
    let path = format!("{}/../../puzzles/wla980-1.txt", env!("CARGO_MANIFEST_DIR"));
    let wla = parse(&std::fs::read_to_string(path).unwrap()).unwrap();
    let expert = generate(Difficulty::Expert.params().with_colors(2), 0)
        .unwrap()
        .puzzle;
    let mut table = Table::new("Recognizer", &["screen", "size", "mean", "best"]);
    for (name, puzzle) in [("wla980-1", &wla), ("expert, 2 colors", &expert)] {
        for (w, h) in [(1280, 720), (1920, 1080), (2560, 1440), (3840, 2160)] {
            let scene = Scene::new(puzzle.clone(), w, h);
            let rgba = draw(&scene);
            let image = Image::new(w, h, &rgba).unwrap();
            assert_eq!(recognize(&image).map(|r| r.puzzle).as_ref(), Ok(puzzle));
            let t = time(|| recognize(&image));
            table.row(vec![
                name.to_owned(),
                format!("{w}×{h}"),
                show(t.mean),
                show(t.best),
            ]);
        }
    }
    table.print();
}
