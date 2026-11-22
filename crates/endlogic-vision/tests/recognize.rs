//! The recognizer on screens drawn in memory in the style of the game.
// Pixel bounds into indices; the pictures here are small.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use endlogic_core::format::parse;
use endlogic_core::generator::{Difficulty, generate};
use endlogic_core::{Color, Puzzle};
use endlogic_vision::synth::{Scene, draw};
use endlogic_vision::{Doubt, Failure, Image, Place, Reason, Recognized, recognize};

fn sample(name: &str) -> Puzzle {
    let path = format!("{}/../../puzzles/{name}", env!("CARGO_MANIFEST_DIR"));
    parse(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn read(scene: &Scene) -> Result<Recognized, Failure> {
    let rgba = draw(scene);
    recognize(&Image::new(scene.width, scene.height, &rgba).unwrap())
}

/// Reads the scene and expects its puzzle back with no doubts.
fn exact(scene: &Scene, what: &str) {
    match read(scene) {
        Ok(r) => {
            assert_eq!(r.puzzle, scene.puzzle, "{what}");
            assert_eq!(r.doubts, [], "{what}");
        }
        Err(e) => panic!("{what}: {e:?}"),
    }
}

const SIZES: [(usize, usize); 3] = [(1280, 720), (1920, 1080), (2560, 1440)];

#[test]
fn the_samples_are_read_at_every_size() {
    for name in [
        "wln006-2.txt",
        "wlh9000-1.txt",
        "wla980-1.txt",
        "wla980-2.txt",
    ] {
        for (i, &(w, h)) in SIZES.iter().enumerate() {
            let mut scene = Scene::new(sample(name), w, h);
            scene.seed = i as u64 + 1;
            exact(&scene, &format!("{name} {w}x{h}"));
        }
    }
}

#[test]
fn a_full_4k_screen_is_read() {
    exact(&Scene::new(sample("wla980-1.txt"), 3840, 2160), "4k");
}

#[test]
fn generated_puzzles_of_every_size_and_both_color_counts_are_read() {
    for difficulty in Difficulty::ALL {
        for colors in [1, 2] {
            for seed in 0..3 {
                let params = difficulty.params().with_colors(colors);
                let puzzle = generate(params, seed).unwrap().puzzle;
                let mut scene = Scene::new(puzzle, 1920, 1080);
                scene.seed = seed + 10;
                exact(
                    &scene,
                    &format!("{} {colors} colors seed {seed}", difficulty.name()),
                );
            }
        }
    }
}

#[test]
fn the_grid_and_the_tray_may_stand_anywhere_in_a_larger_picture() {
    let puzzle = sample("wla980-2.txt");
    let mut scene = Scene::new(puzzle, 2400, 1500);
    scene.pitch = 80.0;
    scene.left = 900.0;
    scene.top = 700.0;
    exact(&scene, "moved");
}

#[test]
fn zero_lines_dead_cells_and_locks_at_the_edges_are_read() {
    let text = "endlogic 1\ngrid\n......\nx.....\n......\n.....#\n......\n......\nrows 0 3 2 5 2 0\ncols 0 2 3 3 3 1\npiece\n##\n#.\npiece\n###\n.#.\npiece\n##\n##\n";
    let puzzle = parse(text).unwrap();
    for &(w, h) in &SIZES {
        exact(&Scene::new(puzzle.clone(), w, h), &format!("{w}x{h}"));
    }
}

#[test]
fn each_card_may_scale_its_piece_its_own_way() {
    let puzzle = sample("wln006-2.txt");
    let mut scene = Scene::new(puzzle, 1920, 1080);
    // Cell sizes of the five pieces, in grid cells.
    scene.piece_cells = Some(vec![0.3, 0.2, 0.25, 0.4, 0.13]);
    exact(&scene, "own scales");
}

#[test]
fn the_sums_pick_the_scale_of_a_piece_without_visible_cells() {
    // A 2×2 square drawn in one flat shade looks like one cell; only the
    // clue sums tell it has four.
    let puzzle = sample("wlh9000-1.txt");
    let mut scene = Scene::new(puzzle.clone(), 1920, 1080);
    scene.shaded = false;
    let r = read(&scene).unwrap();
    assert_eq!(r.puzzle, puzzle);
    let text = "endlogic 1\ngrid\n....\n....\n....\n....\nrows 2 2 2 1\ncols 2 2 2 1\npiece\n##\n##\npiece\n##\n#.\n";
    let puzzle = parse(text).unwrap();
    let mut scene = Scene::new(puzzle.clone(), 1920, 1080);
    scene.shaded = false;
    let r = read(&scene).unwrap();
    assert_eq!(r.puzzle, puzzle);
    assert_eq!(r.doubts, []);
}

#[test]
fn pieces_that_could_swap_their_readings_are_doubtful() {
    // A 2×2 square and a single cell, both flat: each could be either.
    let text = "endlogic 1\ngrid\n....\n....\n....\n....\nrows 2 2 1 0\ncols 2 2 1 0\npiece\n##\n##\npiece\n#\n";
    let mut scene = Scene::new(parse(text).unwrap(), 1920, 1080);
    scene.shaded = false;
    let r = read(&scene).unwrap();
    let unclear = |i| Doubt {
        place: Place::Piece(i),
        reason: Reason::Unclear,
    };
    assert_eq!(r.doubts, [unclear(0), unclear(1)]);
}

#[test]
fn sums_that_cannot_add_up_mark_the_places() {
    // The clues say 6 cells, the pieces have 5.
    let text = "endlogic 1\ngrid\n....\n....\n....\n....\nrows 2 2 2 0\ncols 2 2 2 0\npiece\n##\n##\npiece\n#\n";
    let puzzle = parse(text).unwrap();
    let r = read(&Scene::new(puzzle, 1920, 1080)).unwrap();
    assert!(
        r.doubts.iter().all(|d| d.reason == Reason::Sums),
        "{:?}",
        r.doubts
    );
    assert!(r.doubts.contains(&Doubt {
        place: Place::Piece(0),
        reason: Reason::Sums
    }));
    assert!(r.doubts.contains(&Doubt {
        place: Place::Clue {
            color: Color(0),
            row: true,
            index: 0
        },
        reason: Reason::Sums
    }));
}

#[test]
fn a_color_outside_the_palette_gets_the_next_letter_and_a_doubt() {
    // Teal and violet: violet is not one of the game colors seen so far.
    let text = "endlogic 2\ncolors 3\ngrid\n....\n....\n....\n....\nrows a 2 1 0 0\nrows b 0 0 0 0\nrows c 0 1 2 0\ncols a 2 1 0 0\ncols b 0 0 0 0\ncols c 0 1 2 0\npiece a\n##\n#.\npiece c\n#.\n##\n";
    let puzzle = parse(text).unwrap();
    let r = read(&Scene::new(puzzle.clone(), 1920, 1080)).unwrap();
    assert_eq!(r.puzzle, puzzle);
    assert_eq!(
        r.doubts,
        [Doubt {
            place: Place::Piece(1),
            reason: Reason::NewColor
        }]
    );
}

#[test]
fn the_failures() {
    let puzzle = sample("wla980-1.txt");
    let scene = Scene::new(puzzle.clone(), 1920, 1080);
    // The numbers display.
    let mut numbers = scene.clone();
    numbers.numbers = true;
    assert_eq!(read(&numbers).unwrap_err(), Failure::NumbersMode);
    // A piece on the grid, its card empty.
    let mut placed = scene.clone();
    placed.on_grid = Some(3);
    assert_eq!(read(&placed).unwrap_err(), Failure::PiecesOnBoard);
    // Too small.
    assert_eq!(
        recognize(&Image::new(200, 100, &vec![0; 200 * 100 * 4]).unwrap()).unwrap_err(),
        Failure::Unreadable
    );
    let tiny = Scene::new(puzzle.clone(), 640, 360);
    assert_eq!(read(&tiny).unwrap_err(), Failure::Unreadable);
    // Not the game at all: the background alone.
    let mut empty = Scene::new(
        parse("endlogic 1\ngrid\n.\nrows 0\ncols 0\n").unwrap(),
        1280,
        720,
    );
    empty.pitch = 0.0;
    assert_eq!(read(&empty).unwrap_err(), Failure::NotPuzzle);
    // The grid or the tray painted over.
    let r = read(&scene).unwrap();
    let rgba = draw(&scene);
    let paint = |b: endlogic_vision::Bounds, rgb: [u8; 3]| {
        let mut out = rgba.clone();
        for y in b.y0 as usize..b.y1 as usize {
            for x in b.x0 as usize..b.x1 as usize {
                let i = (y * scene.width + x) * 4;
                out[i..i + 3].copy_from_slice(&rgb);
            }
        }
        recognize(&Image::new(scene.width, scene.height, &out).unwrap())
    };
    assert_eq!(paint(r.grid, [24, 26, 26]).unwrap_err(), Failure::NoBoard);
    assert_eq!(paint(r.tray, [50, 52, 52]).unwrap_err(), Failure::NoTray);
}

#[test]
fn the_same_picture_reads_the_same_way() {
    let scene = Scene::new(sample("wla980-2.txt"), 1600, 900);
    assert_eq!(read(&scene), read(&scene));
}

#[test]
fn a_picture_of_the_wrong_size_is_refused() {
    assert!(Image::new(10, 10, &[0; 399]).is_none());
    assert!(Image::new(10, 10, &[0; 400]).is_some());
}
