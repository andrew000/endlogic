//! The recognizer on screenshots of the game, which are private and stay out
//! of the repository: they are read from `local/screens` when it is there.
//! Also on smaller, framed, noisy and painted-over copies made in memory,
//! and on endlogic's own golden pictures, which are not the game.
#![cfg(windows)]
// Reading PNG files uses Windows Imaging, which needs `unsafe`.
#![allow(unsafe_code)]

mod support;

use std::path::PathBuf;

use endlogic_core::Puzzle;
use endlogic_core::format::parse;
use endlogic_vision::{Failure, Image, Recognized, recognize};
use support::decode;
use support::derive::{Picture, margins, noisy, painted, shrink};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The private screenshots' folder, or `None` (and a note) without it.
fn screens() -> Option<PathBuf> {
    let dir = root().join("local/screens");
    if dir.is_dir() {
        Some(dir)
    } else {
        println!("skipped: no local/screens (the game screenshots are not in the repository)");
        None
    }
}

fn load(file: &std::path::Path) -> Picture {
    let (w, h, mut bgra) = decode(file);
    for px in bgra.chunks_mut(4) {
        px.swap(0, 2);
    }
    Picture {
        width: w as usize,
        height: h as usize,
        rgba: bgra,
    }
}

fn read(p: &Picture) -> Result<Recognized, Failure> {
    recognize(&Image::new(p.width, p.height, &p.rgba).unwrap())
}

/// Reads a screenshot and its copies, each exactly as the truth file says.
fn check(png: &str, truth: &str) {
    let Some(dir) = screens() else { return };
    let want: Puzzle = parse(&std::fs::read_to_string(root().join(truth)).unwrap()).unwrap();
    let shot = load(&dir.join(png));
    let r = read(&shot).unwrap_or_else(|e| panic!("{png}: {e:?}"));
    assert_eq!(r.puzzle, want, "{png}");
    assert_eq!(r.doubts, [], "{png}");
    let (w, h) = (shot.width, shot.height);
    let mut copies: Vec<(String, Picture)> = Vec::new();
    for (tw, th) in [(2560, 1440), (1920, 1080), (1280, 720)] {
        if tw < w {
            copies.push((format!("{tw}x{th}"), shrink(&shot, tw, th)));
        }
    }
    copies.push(("margins".to_owned(), margins(&shot, w / 10, h / 8)));
    copies.push(("darker, noise".to_owned(), noisy(&shot, 0.95, 4, 7)));
    copies.push(("lighter, noise".to_owned(), noisy(&shot, 1.05, 4, 9)));
    let small = shrink(&shot, 1280, 720);
    copies.push(("1280x720, noise".to_owned(), noisy(&small, 0.96, 3, 11)));
    for (name, copy) in &copies {
        let got = read(copy).unwrap_or_else(|e| panic!("{png} {name}: {e:?}"));
        assert_eq!(got.puzzle, want, "{png} {name}");
        assert_eq!(got.doubts, [], "{png} {name}");
    }
    // The grid or the tray painted over.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let at =
        |b: endlogic_vision::Bounds| (b.x0 as usize, b.y0 as usize, b.x1 as usize, b.y1 as usize);
    let (x0, y0, x1, y1) = at(r.grid);
    assert_eq!(
        read(&painted(&shot, x0, y0, x1, y1, [24, 26, 26])).unwrap_err(),
        Failure::NoBoard,
        "{png} without grid"
    );
    let (x0, y0, x1, y1) = at(r.tray);
    assert_eq!(
        read(&painted(&shot, x0, y0, x1, y1, [50, 52, 52])).unwrap_err(),
        Failure::NoTray,
        "{png} without tray"
    );
}

#[test]
fn wln006_windowed() {
    check("wln006-2of2-1356x768.png", "puzzles/wln006-2.txt");
}

#[test]
fn wlh9000_1_windowed() {
    check("wlh9000-1of3-1364x769.png", "puzzles/wlh9000-1.txt");
}

#[test]
fn wlh9000_2_full_screen() {
    check(
        "wlh9000-2of3-3840x2160.png",
        "local/screens/wlh9000-2of3.txt",
    );
}

#[test]
fn wlh9000_3_full_screen() {
    check(
        "wlh9000-3of3-3840x2160.png",
        "local/screens/wlh9000-3of3.txt",
    );
}

#[test]
fn wla980_1_two_colors() {
    check(
        "wla980-1of2-two-colors-3840x2160.png",
        "puzzles/wla980-1.txt",
    );
}

#[test]
fn wla980_2_two_colors() {
    check(
        "wla980-2of2-two-colors-3840x2160.png",
        "puzzles/wla980-2.txt",
    );
}

#[test]
fn endlogic_itself_is_not_the_game() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let mut seen = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "png") {
            let r = read(&load(&path));
            assert!(r.is_err(), "{}: {:?}", path.display(), r.map(|r| r.puzzle));
            seen += 1;
        }
    }
    assert!(seen > 0);
}
