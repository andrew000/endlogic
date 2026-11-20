//! The screenshot mode, run as the real program.
#![cfg(windows)]
// Reading the PNG back uses Windows Imaging, which needs `unsafe`.
#![allow(unsafe_code)]

mod support;

use support::{decode, scratch, shot};

/// The reference puzzle `wln006-2` as an editor link.
const EDIT_LINK: &str =
    "https://andrew000.github.io/endlogic/#e=AWYAG92sqX2JVQAAAAAAAURshXEtRHzP5EwZdQ";

#[test]
fn a_screenshot_is_a_png_of_the_size_asked_for_and_not_blank() {
    for (name, size, extra) in [
        ("wide.png", "1366x768", &[][..]),
        ("phone.png", "390x844", &["--touch", "--lang", "uk"][..]),
        ("edit.png", "800x600", &["--code", EDIT_LINK][..]),
    ] {
        let file = scratch(name);
        let mut args = vec!["--size", size];
        args.extend_from_slice(extra);
        assert!(shot(&file, &args).success(), "{name}");
        let bytes = std::fs::read(&file).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "{name}: not a PNG");
        let (w, h, pixels) = decode(&file);
        let (want_w, want_h) = size.split_once('x').unwrap();
        assert_eq!(
            (w, h),
            (want_w.parse().unwrap(), want_h.parse().unwrap()),
            "{name}"
        );
        let (colors, _) = pixels.as_chunks::<4>();
        let first = colors[0];
        let differ = colors.iter().filter(|&&p| p != first).count();
        assert!(differ > colors.len() / 50, "{name}: almost one color");
        std::fs::remove_file(&file).unwrap();
    }
}

#[test]
fn the_same_arguments_give_the_same_file() {
    let (a, b) = (scratch("same-a.png"), scratch("same-b.png"));
    let args = ["--size", "1366x768", "--lang", "en"];
    assert!(shot(&a, &args).success());
    assert!(shot(&b, &args).success());
    assert_eq!(std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
    std::fs::remove_file(&a).unwrap();
    std::fs::remove_file(&b).unwrap();
}

#[test]
fn bad_arguments_fail() {
    let file = scratch("bad.png");
    assert!(!shot(&file, &["--size", "0x10"]).success());
    assert!(!shot(&file, &["--size", "10x10", "--code", "not a code"]).success());
    let missing = scratch("no-such-dir").join("deeper").join("x.png");
    assert!(!shot(&missing, &["--size", "10x10"]).success());
    assert!(!file.exists());
}
