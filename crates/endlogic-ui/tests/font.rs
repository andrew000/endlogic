//! The trainer's own font: every character the interface can show has a
//! glyph, and widths behave.

mod common;

use common::*;
use endlogic_core::code::encode;
use endlogic_ui::{
    App, CodeKind, Config, Font, MIN_CAP_HEIGHT, MIN_FONT_SIZE, Point, Rgba, SETTING_COUNTERS,
    SETTING_DIFFICULTY, SETTING_LANG, draw_text, has_glyph, repertoire, text_width,
};

/// Every character in the string literals of `source`, format placeholders
/// (`{...}`) left out.
fn literal_chars(source: &str) -> Vec<char> {
    let mut chars = Vec::new();
    let mut rest = source;
    while let Some(start) = rest.find('"') {
        let body = &rest[start + 1..];
        let mut end = 0;
        let mut escaped = false;
        let mut depth = 0;
        for (i, c) in body.char_indices() {
            end = i;
            if escaped {
                escaped = false;
                if depth == 0 && c != 'n' && c != 'u' {
                    chars.push(c);
                }
                continue;
            }
            match c {
                '\\' => escaped = true,
                '"' => break,
                '{' => depth += 1,
                '}' if depth > 0 => depth -= 1,
                _ if depth == 0 => chars.push(c),
                _ => {}
            }
        }
        rest = &body[end + 1..];
    }
    chars
}

#[test]
fn every_text_of_the_interface_has_glyphs() {
    let dir = format!("{}/src", env!("CARGO_MANIFEST_DIR"));
    let mut missing = Vec::new();
    for file in [
        "text.rs",
        "draw.rs",
        "draw/edit.rs",
        "layout.rs",
        "layout/edit.rs",
    ] {
        let source = std::fs::read_to_string(format!("{dir}/{file}")).unwrap();
        for c in literal_chars(&source) {
            if !c.is_control() && !has_glyph(c) {
                missing.push((file, c));
            }
        }
    }
    // What the app builds at run time: puzzle ids (base64url codes),
    // numbers, pages and counts.
    let dynamic = "Δ-E-N-H-X-C-ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_ 0/16 1 / 5 · 50+ …";
    missing.extend(
        dynamic
            .chars()
            .filter(|&c| !has_glyph(c))
            .map(|c| ("dynamic", c)),
    );
    assert!(missing.is_empty(), "no glyph for {missing:?}");
}

#[test]
fn every_text_drawn_in_many_states_has_glyphs() {
    let mut texts = Vec::new();
    for lang in ["en", "ua"] {
        for level in ["easy", "expert"] {
            for counters in ["grid", "numbers"] {
                let mut config: Config = config(1366.0, 768.0);
                config.settings = vec![
                    (SETTING_LANG.to_owned(), lang.to_owned()),
                    (SETTING_DIFFICULTY.to_owned(), level.to_owned()),
                    (SETTING_COUNTERS.to_owned(), counters.to_owned()),
                ];
                texts.extend(settle(&mut App::new(config.clone())).texts);
                config.code = Some((CodeKind::Edit, encode(&sample("wlh9000-1.txt")).unwrap()));
                texts.extend(settle(&mut App::new(config)).texts);
            }
        }
    }
    assert!(texts.len() > 100);
    for text in texts {
        assert!(text.chars().all(has_glyph), "no glyph in {text:?}");
    }
}

#[test]
fn the_repertoire_is_complete_and_lower_case_letters_are_capitals() {
    for c in repertoire().chars() {
        assert!(has_glyph(c), "{c:?}");
    }
    let font = Font::new(20.0, false);
    for (lower, upper) in [
        ("abcdefghijklmnopqrstuvwxyz", "ABCDEFGHIJKLMNOPQRSTUVWXYZ"),
        (
            "абвгґдеєжзиіїйклмнопрстуфхцчшщьюя",
            "АБВГҐДЕЄЖЗИІЇЙКЛМНОПРСТУФХЦЧШЩЬЮЯ",
        ),
    ] {
        assert!(lower.chars().all(has_glyph));
        assert!((text_width(lower, font) - text_width(upper, font)).abs() < 1e-3);
    }
}

#[test]
fn digits_share_one_width_and_narrow_letters_are_narrow() {
    let font = Font::new(20.0, true);
    let one = text_width("0", font);
    for d in '0'..='9' {
        assert!((text_width(&d.to_string(), font) - one).abs() < 1e-4, "{d}");
    }
    assert!(text_width("I", font) < one);
    assert!(text_width("Ш", font) > one);
    assert!((text_width("", font)).abs() < f32::EPSILON);
    // Capitals are 0.7 of the size, never smaller than the minimum.
    assert!((Font::new(20.0, false).cap_height() - 14.0).abs() < 1e-4);
    assert!((Font::new(2.0, false).cap_height() - MIN_CAP_HEIGHT).abs() < 1e-4);
    assert!((Font::new(2.0, false).size - MIN_FONT_SIZE).abs() < 1e-4);
}

#[test]
fn an_unknown_character_is_a_box_and_does_not_panic() {
    assert!(!has_glyph('€'));
    let mut drawing = Recorder::default();
    draw_text(
        &mut drawing,
        "€",
        Point::new(10.0, 10.0),
        Font::new(20.0, false),
        endlogic_ui::Align::Left,
        Rgba::hex(0xFF_FFFF),
    );
    assert_eq!(drawing.calls.len(), 1);
    let Call::Polyline(points, _) = &drawing.calls[0] else {
        panic!("{:?}", drawing.calls);
    };
    // A closed box as tall as a capital.
    assert_eq!(points.first(), points.last());
    let top = points.iter().map(|p| p.y).fold(f32::MAX, f32::min);
    let bottom = points.iter().map(|p| p.y).fold(f32::MIN, f32::max);
    assert!((bottom - top - 14.0).abs() < 1e-3);
    // Text is centered on the anchor vertically.
    assert!((f32::midpoint(top, bottom) - 10.0).abs() < 1e-3);
}
