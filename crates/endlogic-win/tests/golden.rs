//! Golden screenshots: the screenshot mode compared with PNG files in
//! `tests/golden`, pixel by pixel with a small allowance for antialiasing.
//!
//! When a picture differs, the actual one is written under the target
//! directory and the test names both files. `just bless` (`ENDLOGIC_BLESS=1`)
//! replaces the golden pictures instead.
#![cfg(windows)]
// Reading the PNG back uses Windows Imaging, which needs `unsafe`.
#![allow(unsafe_code)]

mod support;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use support::{decode, shot};

/// A channel may differ by this much and the pixel still counts as equal.
const CHANNEL_SLACK: u8 = 16;
/// At most this many pixels in a million may differ by more.
const PIXELS_PER_MILLION: usize = 200;

const EASY: &str = "AUQAFQxJlwCAIABkLwjpDyDalg";
const NORMAL: &str = "AVUAFyuJXSAgQBlAGAQS3IXhL0R4QjU";
const HARD: &str = "AWYAGyOW-e1AAGEQjEDiABRHRJ4j5JMj8yDgzbA";
const EXPERT: &str = "AXcAElRkdmRXYjdgAAaAQeg5AUAZDGeIzmMeRyeiLiLkaXLn7w";
const WLN: &str = "AWYAG92sqX2JVQAAAAAAAURshXEtRHzP5EwZdQ";
/// Normal and Expert with two colors, seed 0.
const NORMAL_2: &str = "AVUgADKaRCqQpIoplIEBQDJQDAJIbwlbEXEPwF-S";
const EXPERT_2: &str = "AXcgADAAI2RiIUIUWSogiQEbEBGIAANQED0DiAWgGQaZHxBHRmJ_ER4Y-MjvJPz7";
/// The game's two-color samples.
const WLA_1: &str = "AVUgADzcizcihMohMogAABABANEeiPRHhDeM_mMGQ_B3zw";
const WLA_2: &str = "AVUgADUKJUKJoxIoxIgMDAwMDHCPRHhD5EeiPRHhDyDLZQ";

/// Name, size and the other arguments of every golden picture.
fn scenes() -> Vec<(&'static str, &'static str, Vec<String>)> {
    let web = "https://andrew000.github.io/endlogic/";
    let code = |c: &str| vec!["--code".to_owned(), c.to_owned()];
    let with = |mut a: Vec<String>, more: &[&str]| {
        a.extend(more.iter().map(|s| (*s).to_owned()));
        a
    };
    vec![
        ("start-1366x768-en", "1366x768", vec![]),
        ("easy-1366x768-en", "1366x768", code(EASY)),
        (
            "normal-1366x768-uk",
            "1366x768",
            with(code(NORMAL), &["--lang", "uk"]),
        ),
        ("hard-1366x768-en", "1366x768", code(HARD)),
        (
            "expert-1366x768-uk",
            "1366x768",
            with(code(EXPERT), &["--lang", "uk"]),
        ),
        (
            "easy-390x844-touch-uk",
            "390x844",
            with(code(EASY), &["--touch", "--lang", "uk"]),
        ),
        (
            "hard-390x844-touch-en",
            "390x844",
            with(code(HARD), &["--touch"]),
        ),
        (
            "expert-390x844-touch-en",
            "390x844",
            with(code(EXPERT), &["--touch"]),
        ),
        (
            "editor-wln006-2-1366x768-en",
            "1366x768",
            code(&format!("{web}#e={WLN}")),
        ),
        (
            "editor-wln006-2-390x844-touch-uk",
            "390x844",
            with(
                code(&format!("{web}#e={WLN}")),
                &["--touch", "--lang", "uk"],
            ),
        ),
        (
            "custom-wln006-2-844x390-uk",
            "844x390",
            with(code(&format!("{web}#c={WLN}")), &["--lang", "uk"]),
        ),
        ("normal-2c-1366x768-en", "1366x768", code(NORMAL_2)),
        (
            "normal-2c-390x844-touch-uk",
            "390x844",
            with(code(NORMAL_2), &["--touch", "--lang", "uk"]),
        ),
        (
            "expert-2c-1366x768-uk",
            "1366x768",
            with(code(EXPERT_2), &["--lang", "uk"]),
        ),
        (
            "expert-2c-390x844-touch-en",
            "390x844",
            with(code(EXPERT_2), &["--touch"]),
        ),
        ("wla980-1-1366x768-en", "1366x768", code(WLA_1)),
        (
            "wla980-2-390x844-touch-uk",
            "390x844",
            with(code(WLA_2), &["--touch", "--lang", "uk"]),
        ),
    ]
}

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join(format!("{name}.png"))
}

fn actual_path(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("golden")
        .join(env!("CARGO_PKG_NAME"));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(format!("{name}.png"))
}

/// How two pictures differ, or `None` if they are equal within the slack.
fn compare(expected: &Path, actual: &Path) -> Option<String> {
    let (ew, eh, old) = decode(expected);
    let (aw, ah, new) = decode(actual);
    if (ew, eh) != (aw, ah) {
        return Some(format!("size {ew}x{eh} expected, {aw}x{ah} drawn"));
    }
    let (old, _) = old.as_chunks::<4>();
    let (new, _) = new.as_chunks::<4>();
    let mut count = 0usize;
    let mut worst = 0u8;
    let (mut left, mut top, mut right, mut bottom) = (u32::MAX, u32::MAX, 0, 0);
    for (i, (a, b)) in old.iter().zip(new).enumerate() {
        let diff = a.iter().zip(b).map(|(x, y)| x.abs_diff(*y)).max().unwrap();
        if diff > CHANNEL_SLACK {
            count += 1;
            worst = worst.max(diff);
            let i = u32::try_from(i).unwrap();
            let (x, y) = (i % ew, i / ew);
            (left, top, right, bottom) = (left.min(x), top.min(y), right.max(x), bottom.max(y));
        }
    }
    let allowed = old.len() * PIXELS_PER_MILLION / 1_000_000;
    if count <= allowed {
        return None;
    }
    let mut text = format!("{count} pixels differ (at most {allowed} may), ");
    let _ = write!(
        text,
        "by up to {worst} per channel, within x {left}..={right}, y {top}..={bottom}"
    );
    Some(text)
}

#[test]
fn screenshots_match_the_golden_pictures() {
    let bless = std::env::var_os("ENDLOGIC_BLESS").is_some_and(|v| v == "1");
    let mut failures = Vec::new();
    for (name, size, extra) in scenes() {
        let actual = actual_path(name);
        let mut args = vec!["--size", size];
        args.extend(extra.iter().map(String::as_str));
        assert!(
            shot(&actual, &args).success(),
            "{name}: the screenshot mode failed"
        );
        let expected = golden_path(name);
        if bless {
            std::fs::copy(&actual, &expected).unwrap();
            continue;
        }
        if !expected.exists() {
            failures.push(format!(
                "  {name}: no golden picture {}\n    actual: {}",
                expected.display(),
                actual.display()
            ));
            continue;
        }
        if let Some(why) = compare(&expected, &actual) {
            failures.push(format!(
                "  {name}: {why}\n    expected: {}\n    actual:   {}",
                expected.display(),
                actual.display()
            ));
        } else {
            std::fs::remove_file(&actual).unwrap();
        }
    }
    assert!(
        failures.is_empty(),
        "{} screenshots differ from the golden pictures:\n{}\n\
         if the change is intended, run `just bless`",
        failures.len(),
        failures.join("\n")
    );
}
