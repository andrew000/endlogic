//! Puzzle links in pasted text, for every shell.

use alloc::borrow::ToOwned;
use alloc::string::String;

use endlogic_core::code::decode;

use crate::shell::CodeKind;

/// The address fragment key of a kind of code: `p`, `e` or `c`.
fn key(kind: CodeKind) -> char {
    match kind {
        CodeKind::Play => 'p',
        CodeKind::Edit => 'e',
        CodeKind::Custom => 'c',
    }
}

/// Reads a puzzle from pasted or passed text: a full link with a `p=`, `e=`
/// or `c=` fragment, the fragment alone (with or without `#`), or a bare
/// code, which means a puzzle to play. Returns `None` for anything else,
/// including a code that does not decode to a puzzle.
#[must_use]
pub fn parse_link(text: &str) -> Option<(CodeKind, String)> {
    let text = text.trim();
    let fragment = text.rsplit_once('#').map_or(text, |(_, f)| f);
    let (kind, code) = match fragment.split_once('=') {
        Some((k, code)) => {
            let kind = [CodeKind::Play, CodeKind::Edit, CodeKind::Custom]
                .into_iter()
                .find(|&kind| k.len() == 1 && k.starts_with(key(kind)))?;
            (kind, code)
        }
        // A bare code only when the text is nothing but the code.
        None if fragment == text => (CodeKind::Play, text),
        None => return None,
    };
    decode(code).ok()?;
    Some((kind, code.to_owned()))
}
