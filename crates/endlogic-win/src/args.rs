//! The command line.

use std::path::PathBuf;

/// What the program was asked to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Open the window, with the puzzle in the first argument if any
    /// (read like pasted text).
    Window {
        /// A link or a code.
        start: Option<String>,
    },
    /// Draw one settled frame into a PNG file, without a window.
    Screenshot(Shot),
}

/// The settings of the screenshot mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shot {
    /// The PNG file to write.
    pub file: PathBuf,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// A link or a code to open.
    pub code: Option<String>,
    /// System language tag: `en` or `uk`.
    pub lang: String,
    /// Report a touch screen.
    pub touch: bool,
    /// Pixels per logical pixel: 1, 2 or 3.
    pub scale: u32,
    /// Draw the font sample instead of the app.
    pub font_sample: bool,
    /// A picture to import, as if pasted.
    pub image: Option<PathBuf>,
}

/// Largest side of a screenshot.
const MAX_SIDE: u32 = 8192;

/// The usage line of the screenshot mode.
pub const USAGE: &str = "endlogic-win [LINK] | endlogic-win --screenshot <FILE.png> --size <W>x<H> [--code <LINK>] [--lang en|uk] [--touch] [--scale 1|2|3] [--font-sample] [--image <FILE>]";

/// Parses the arguments after the program name.
///
/// # Errors
///
/// A message for anything that is not one of the two forms in [`USAGE`].
pub fn parse(args: &[String]) -> Result<Command, String> {
    match args {
        [] => return Ok(Command::Window { start: None }),
        [one] if !one.starts_with("--") => {
            return Ok(Command::Window {
                start: Some(one.clone()),
            });
        }
        _ => {}
    }
    let mut file = None;
    let mut size = None;
    let mut code = None;
    let mut lang = "en".to_owned();
    let mut touch = false;
    let mut scale = 1;
    let mut font_sample = false;
    let mut image = None;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut value = |name: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match arg.as_str() {
            "--screenshot" => file = Some(PathBuf::from(value("--screenshot")?)),
            "--size" => size = Some(parse_size(&value("--size")?)?),
            "--code" => code = Some(value("--code")?),
            "--lang" => {
                lang = value("--lang")?;
                if lang != "en" && lang != "uk" {
                    return Err(format!("--lang must be en or uk, not {lang}"));
                }
            }
            "--touch" => touch = true,
            "--scale" => {
                let text = value("--scale")?;
                scale = match text.as_str() {
                    "1" => 1,
                    "2" => 2,
                    "3" => 3,
                    _ => return Err(format!("--scale must be 1, 2 or 3, not {text}")),
                };
            }
            "--font-sample" => font_sample = true,
            "--image" => image = Some(PathBuf::from(value("--image")?)),
            other => return Err(format!("unexpected argument {other}")),
        }
    }
    let file = file.ok_or("--screenshot <FILE.png> is required")?;
    let (width, height) = size.ok_or("--size <W>x<H> is required")?;
    if width * scale > MAX_SIDE || height * scale > MAX_SIDE {
        return Err(format!("--size times --scale must be at most {MAX_SIDE}"));
    }
    Ok(Command::Screenshot(Shot {
        file,
        width,
        height,
        code,
        lang,
        touch,
        scale,
        font_sample,
        image,
    }))
}

fn parse_size(text: &str) -> Result<(u32, u32), String> {
    let bad = || format!("--size must be <W>x<H>, not {text}");
    let (w, h) = text.split_once(['x', 'X']).ok_or_else(bad)?;
    let w: u32 = w.parse().map_err(|_| bad())?;
    let h: u32 = h.parse().map_err(|_| bad())?;
    if !(1..=MAX_SIDE).contains(&w) || !(1..=MAX_SIDE).contains(&h) {
        return Err(format!("--size sides must be 1 to {MAX_SIDE}"));
    }
    Ok((w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn window_with_or_without_a_puzzle() {
        assert_eq!(parse(&[]), Ok(Command::Window { start: None }));
        assert_eq!(
            parse(&args(&["https://x/#p=AQ"])),
            Ok(Command::Window {
                start: Some("https://x/#p=AQ".to_owned())
            })
        );
    }

    #[test]
    fn screenshot_options() {
        let parsed = parse(&args(&[
            "--screenshot",
            "out.png",
            "--size",
            "390x844",
            "--code",
            "#e=AQ",
            "--lang",
            "uk",
            "--touch",
        ]));
        assert_eq!(
            parsed,
            Ok(Command::Screenshot(Shot {
                file: PathBuf::from("out.png"),
                width: 390,
                height: 844,
                code: Some("#e=AQ".to_owned()),
                lang: "uk".to_owned(),
                touch: true,
                scale: 1,
                font_sample: false,
                image: None,
            }))
        );
        let sample = parse(&args(&[
            "--screenshot",
            "f.png",
            "--size",
            "100x50",
            "--scale",
            "2",
            "--font-sample",
        ]));
        assert!(matches!(
            sample,
            Ok(Command::Screenshot(Shot {
                scale: 2,
                font_sample: true,
                image: None,
                ..
            }))
        ));
        let plain = parse(&args(&["--size", "10X20", "--screenshot", "a.png"]));
        assert!(
            matches!(plain, Ok(Command::Screenshot(Shot { width: 10, height: 20, ref lang, touch: false, code: None, .. })) if lang == "en")
        );
    }

    #[test]
    fn bad_arguments_are_errors() {
        for list in [
            &["--screenshot"][..],
            &["--screenshot", "a.png"],
            &["--size", "10x10"],
            &["--screenshot", "a.png", "--size", "10"],
            &["--screenshot", "a.png", "--size", "0x10"],
            &["--screenshot", "a.png", "--size", "10x99999"],
            &["--screenshot", "a.png", "--size", "axb"],
            &["--screenshot", "a.png", "--size", "10x10", "--lang", "fr"],
            &["--screenshot", "a.png", "--size", "10x10", "--what"],
            &["one", "two"],
            &["--touch"],
        ] {
            assert!(parse(&args(list)).is_err(), "{list:?}");
        }
    }
}
