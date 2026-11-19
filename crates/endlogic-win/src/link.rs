//! Puzzle links: reading one from pasted text and making a shareable one.

use endlogic_core::code::decode;
use endlogic_ui::CodeKind;

/// The web page that opens shared links.
pub const WEB_ADDRESS: &str = "https://andrew000.github.io/endlogic/";

/// The address fragment key of a kind of code: `p`, `e` or `c`.
fn key(kind: CodeKind) -> char {
    match kind {
        CodeKind::Play => 'p',
        CodeKind::Edit => 'e',
        CodeKind::Custom => 'c',
    }
}

/// The web link that opens `code` as `kind`.
#[must_use]
pub fn share_link(kind: CodeKind, code: &str) -> String {
    format!("{WEB_ADDRESS}#{}={code}", key(kind))
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The reference puzzle `wln006-2`.
    const CODE: &str = "AWYAG92sqX2JVQAAAAAAAURshXEtRHzP5EwZdQ";

    #[test]
    fn full_links_of_every_kind() {
        for (k, kind) in [
            ('p', CodeKind::Play),
            ('e', CodeKind::Edit),
            ('c', CodeKind::Custom),
        ] {
            let link = format!("https://andrew000.github.io/endlogic/#{k}={CODE}");
            assert_eq!(parse_link(&link), Some((kind, CODE.to_owned())));
            assert_eq!(share_link(kind, CODE), link);
            // Another address, a query, spaces around.
            let other = format!("  http://localhost:8080/?r=3#{k}={CODE}\r\n");
            assert_eq!(parse_link(&other), Some((kind, CODE.to_owned())));
        }
    }

    #[test]
    fn fragments_alone() {
        assert_eq!(
            parse_link(&format!("#e={CODE}")),
            Some((CodeKind::Edit, CODE.to_owned()))
        );
        assert_eq!(
            parse_link(&format!("c={CODE}")),
            Some((CodeKind::Custom, CODE.to_owned()))
        );
    }

    #[test]
    fn a_bare_code_is_a_puzzle_to_play() {
        assert_eq!(parse_link(CODE), Some((CodeKind::Play, CODE.to_owned())));
        assert_eq!(
            parse_link(&format!("\t{CODE} ")),
            Some((CodeKind::Play, CODE.to_owned()))
        );
    }

    #[test]
    fn garbage_is_ignored() {
        for text in [
            "",
            "   ",
            "hello world",
            "https://example.org/",
            "https://example.org/#top",
            &format!("#x={CODE}"),
            &format!("#pp={CODE}"),
            "#p=",
            "#p=AAAA",
            &format!("{CODE}x"),
            &format!("see {CODE}"),
            &CODE[..CODE.len() - 1],
        ] {
            assert_eq!(parse_link(text), None, "{text:?}");
        }
    }
}
