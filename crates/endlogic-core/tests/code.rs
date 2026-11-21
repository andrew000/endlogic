//! Tests for puzzle codes.

use std::collections::BTreeMap;

use endlogic_core::code::{CODE_VERSION, DecodeError, EncodeError, decode, encode};
use endlogic_core::format::parse;
use endlogic_core::generator::{Difficulty, generate};
use endlogic_core::{Axis, Cell, Color, Grid, LineClues, Piece, Puzzle, Shape};

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Test-side base64url helpers, so tests can build codes byte by byte.
fn to_base64url(bytes: &[u8]) -> String {
    let bits: Vec<u8> = bytes
        .iter()
        .flat_map(|b| (0..8).rev().map(move |i| (b >> i) & 1))
        .collect();
    bits.chunks(6)
        .map(|chunk| {
            // A short final chunk is padded with zero bits on the right.
            let value = (0..6).fold(0usize, |acc, i| {
                (acc << 1) | usize::from(chunk.get(i).copied().unwrap_or(0))
            });
            char::from(ALPHABET[value])
        })
        .collect()
}

fn from_base64url(code: &str) -> Vec<u8> {
    let bits: Vec<u8> = code
        .bytes()
        .flat_map(|ch| {
            let value = u8::try_from(ALPHABET.iter().position(|&a| a == ch).unwrap()).unwrap();
            (0..6).rev().map(move |i| (value >> i) & 1)
        })
        .collect();
    bits.chunks(8)
        .filter(|chunk| chunk.len() == 8)
        .map(|chunk| chunk.iter().fold(0u8, |acc, &bit| (acc << 1) | bit))
        .collect()
}

/// CRC-16/CCITT-FALSE, written independently of the crate's version.
fn crc16(bytes: &[u8]) -> u16 {
    let mut crc = 0xffffu16;
    for &b in bytes {
        for i in (0..8).rev() {
            let bit = (b >> i) & 1 == 1;
            let top = crc & 0x8000 != 0;
            crc <<= 1;
            if bit != top {
                crc ^= 0x1021;
            }
        }
    }
    crc
}

/// Wraps raw payload bytes into a code with a valid checksum.
fn code_from_body(body: &[u8]) -> String {
    let mut bytes = body.to_vec();
    bytes.extend(crc16(body).to_be_bytes());
    to_base64url(&bytes)
}

fn samples() -> Vec<(String, Puzzle)> {
    let dir = format!("{}/../../puzzles", env!("CARGO_MANIFEST_DIR"));
    let mut samples: Vec<(String, Puzzle)> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            let text = std::fs::read_to_string(entry.path()).unwrap();
            (
                entry.file_name().into_string().unwrap(),
                parse(&text).unwrap(),
            )
        })
        .collect();
    samples.sort_by(|a, b| a.0.cmp(&b.0));
    samples
}

#[test]
fn samples_round_trip() {
    for (name, puzzle) in samples() {
        let code = encode(&puzzle).unwrap();
        println!("{name}: {} chars: {code}", code.len());
        assert!(
            code.bytes().all(|b| ALPHABET.contains(&b)),
            "{name}: {code}"
        );
        assert_eq!(decode(&code).unwrap(), puzzle, "{name}");
    }
}

#[test]
fn reference_code_is_short_and_stable() {
    let (_, puzzle) = samples()
        .into_iter()
        .find(|(name, _)| name == "wln006-2.txt")
        .unwrap();
    let code = encode(&puzzle).unwrap();
    assert_eq!(code.len(), 38);
    assert_eq!(encode(&puzzle).unwrap(), code);
    assert_eq!(from_base64url(&code)[0], CODE_VERSION);
    assert_eq!(decode(&format!("  {code}\n")).unwrap(), puzzle);
}

#[test]
fn generated_puzzles_round_trip() {
    for difficulty in Difficulty::ALL {
        for seed in 0..5 {
            let puzzle = generate(difficulty.params(), seed).unwrap().puzzle;
            assert_eq!(decode(&encode(&puzzle).unwrap()).unwrap(), puzzle);
        }
    }
}

#[test]
fn colors_survive_a_round_trip() {
    let mut grid = Grid::new(3, 2).unwrap();
    grid.set(0, 0, Cell::Lock(Color(3)));
    grid.set(1, 2, Cell::Dead);
    let pieces = vec![
        Piece::new(Color(1), Shape::new([(0, 0), (0, 1)]).unwrap()),
        Piece::new(Color(3), Shape::new([(0, 0)]).unwrap()),
    ];
    let mut clues = BTreeMap::new();
    clues.insert(
        Color(1),
        LineClues {
            rows: vec![2, 0],
            cols: vec![0, 1, 1],
        },
    );
    // Color 3 deliberately has no clues; color 2 has clues but nothing else.
    clues.insert(
        Color(2),
        LineClues {
            rows: vec![0, 0],
            cols: vec![0, 0, 0],
        },
    );
    let puzzle = Puzzle::new(grid, pieces, clues).unwrap();
    assert_eq!(decode(&encode(&puzzle).unwrap()).unwrap(), puzzle);
}

/// Generated puzzles of two, three and four colors keep every color.
#[test]
fn generated_colored_puzzles_round_trip() {
    for colors in 2..=4 {
        for difficulty in [Difficulty::Normal, Difficulty::Expert] {
            for seed in 0..3 {
                let params = difficulty.params().with_colors(colors);
                let puzzle = generate(params, seed).unwrap().puzzle;
                assert_eq!(puzzle.colors().len(), colors);
                assert_eq!(decode(&encode(&puzzle).unwrap()).unwrap(), puzzle);
            }
        }
    }
}

/// Codes of single-color puzzles are the same as before colors.
#[test]
fn single_color_codes_are_unchanged() {
    let reference = samples()
        .into_iter()
        .find(|(name, _)| name == "wln006-2.txt")
        .unwrap()
        .1;
    assert_eq!(
        encode(&reference).unwrap(),
        "AWYAG92sqX2JVQAAAAAAAURshXEtRHzP5EwZdQ"
    );
    let hard = generate(Difficulty::Hard.params(), 1).unwrap().puzzle;
    assert_eq!(
        encode(&hard).unwrap(),
        "AWYAGPbauz28AgIAAEAABiH4SuItyQvoTGfEl628"
    );
}

/// The game's two-color samples have fixed codes.
#[test]
fn game_sample_codes() {
    let codes: Vec<(String, String)> = samples()
        .into_iter()
        .filter(|(name, _)| name.starts_with("wla980"))
        .map(|(name, puzzle)| (name, encode(&puzzle).unwrap()))
        .collect();
    assert_eq!(
        codes,
        [
            (
                "wla980-1.txt".to_owned(),
                "AVUgADzcizcihMohMogAABABANEeiPRHhDeM_mMGQ_B3zw".to_owned()
            ),
            (
                "wla980-2.txt".to_owned(),
                "AVUgADUKJUKJoxIoxIgMDAwMDHCPRHhD5EeiPRHhDyDLZQ".to_owned()
            ),
        ]
    );
}

#[test]
fn empty_puzzle_round_trips() {
    let puzzle = Puzzle::new(Grid::new(1, 1).unwrap(), Vec::new(), BTreeMap::new()).unwrap();
    assert_eq!(decode(&encode(&puzzle).unwrap()).unwrap(), puzzle);
}

#[test]
fn encode_errors() {
    let grid = Grid::new(2, 2).unwrap();
    let mono = Shape::new([(0, 0)]).unwrap();
    let too_long = Puzzle::single_color(grid.clone(), Vec::new(), vec![0, 3], vec![0, 0]).unwrap();
    assert_eq!(
        encode(&too_long),
        Err(EncodeError::ClueOutOfRange {
            color: Color::DEFAULT,
            axis: Axis::Row,
            index: 1,
            value: 3
        })
    );
    // A puzzle holds at most four colors, so a code never has too many.
    let fifth = vec![Piece::new(Color(4), mono)];
    assert!(Puzzle::new(grid, fifth, BTreeMap::new()).is_err());
}

/// A well-formed code whose palette names a fifth color is rejected.
#[test]
fn colors_past_the_fourth_are_rejected() {
    // 1x1 grid, palette of one color with id 4, no clues, a free cell, no
    // pieces: 4 + 4 + 3 + 8 + 1 + 1 + 6 bits.
    let mut bits = String::from(concat!(
        "0000", "0000", "000", "00000100", "0", "0", "000000"
    ));
    while bits.len() % 8 != 0 {
        bits.push('0');
    }
    let mut body = vec![1u8];
    for chunk in bits.as_bytes().chunks(8) {
        body.push(u8::from_str_radix(std::str::from_utf8(chunk).unwrap(), 2).unwrap());
    }
    assert_eq!(
        decode(&code_from_body(&body)),
        Err(DecodeError::InvalidData("color id"))
    );
    // The same with color 0 decodes.
    assert_eq!(body[3], 0b1000_0000);
    body[3] = 0;
    let zero = code_from_body(&body);
    assert!(decode(&zero).is_ok(), "{:?}", decode(&zero));
}

#[test]
fn empty_code_is_rejected() {
    assert_eq!(decode(""), Err(DecodeError::Empty));
    assert_eq!(decode("  \n"), Err(DecodeError::Empty));
}

#[test]
fn garbage_is_rejected() {
    assert_eq!(
        decode("AQ!x"),
        Err(DecodeError::InvalidCharacter {
            position: 2,
            ch: '!'
        })
    );
    assert_eq!(
        decode("AQ=="),
        Err(DecodeError::InvalidCharacter {
            position: 2,
            ch: '='
        })
    );
    assert_eq!(decode("ABCDE"), Err(DecodeError::InvalidLength));
    assert_eq!(
        decode("hello-world_this-is-not-a-codeAA"),
        Err(DecodeError::UnsupportedVersion(133))
    );
    assert_eq!(
        decode("AR"),
        Err(DecodeError::InvalidCharacter {
            position: 1,
            ch: 'R'
        })
    );
}

#[test]
fn damaged_code_fails_the_checksum() {
    let code = encode(&samples()[0].1).unwrap();
    let mut chars: Vec<char> = code.chars().collect();
    let middle = chars.len() / 2;
    chars[middle] = if chars[middle] == 'A' { 'B' } else { 'A' };
    let damaged: String = chars.into_iter().collect();
    assert_eq!(decode(&damaged), Err(DecodeError::ChecksumMismatch));
}

#[test]
fn truncated_codes_never_decode() {
    for (name, puzzle) in samples() {
        let code = encode(&puzzle).unwrap();
        for len in 1..code.len() {
            let result = decode(&code[..len]);
            assert!(result.is_err(), "{name}: prefix of {len} decoded");
        }
    }
    assert_eq!(decode("AQ"), Err(DecodeError::Truncated));
    // A valid checksum over a payload that stops too early.
    assert_eq!(
        decode(&code_from_body(&[1, 0x66])),
        Err(DecodeError::Truncated)
    );
}

#[test]
fn unknown_version_is_rejected() {
    let code = encode(&samples()[0].1).unwrap();
    let mut bytes = from_base64url(&code);
    bytes.truncate(bytes.len() - 2);
    bytes[0] = 2;
    assert_eq!(
        decode(&code_from_body(&bytes)),
        Err(DecodeError::UnsupportedVersion(2))
    );
}

#[test]
fn trailing_data_is_rejected() {
    let code = encode(&samples()[0].1).unwrap();
    let mut bytes = from_base64url(&code);
    bytes.truncate(bytes.len() - 2);
    bytes.push(0);
    assert_eq!(
        decode(&code_from_body(&bytes)),
        Err(DecodeError::InvalidData("trailing data"))
    );
}
