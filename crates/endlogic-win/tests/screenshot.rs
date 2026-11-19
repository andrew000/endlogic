//! The screenshot mode, run as the real program.
#![cfg(windows)]
// Reading the PNG back uses Windows Imaging, which needs `unsafe`.
#![allow(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppBGRA, IWICImagingFactory,
    WICBitmapDitherTypeNone, WICBitmapPaletteTypeCustom, WICDecodeMetadataCacheOnDemand,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
};
use windows::core::HSTRING;

/// The reference puzzle `wln006-2` as an editor link.
const EDIT_LINK: &str =
    "https://andrew000.github.io/endlogic/#e=AWYAG92sqX2JVQAAAAAAAURshXEtRHzP5EwZdQ";

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("endlogic-win-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn shot(file: &Path, args: &[&str]) -> std::process::ExitStatus {
    Command::new(env!("CARGO_BIN_EXE_endlogic-win"))
        .arg("--screenshot")
        .arg(file)
        .args(args)
        .status()
        .unwrap()
}

/// Width, height and pixels (BGRA) of a PNG file.
fn decode(file: &Path) -> (u32, u32, Vec<u8>) {
    // SAFETY: initializes COM for this test thread; repeated calls are fine.
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    // SAFETY: creates the in-process WIC factory.
    let wic: IWICImagingFactory =
        unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER) }.unwrap();
    let path = HSTRING::from(file.as_os_str());
    // SAFETY: `path` lives through the call; read access only.
    let decoder = unsafe {
        wic.CreateDecoderFromFilename(
            &path,
            None,
            windows::Win32::Foundation::GENERIC_READ,
            WICDecodeMetadataCacheOnDemand,
        )
    }
    .unwrap();
    // SAFETY: a PNG has one frame.
    let frame = unsafe { decoder.GetFrame(0) }.unwrap();
    // SAFETY: a plain creation call.
    let converter = unsafe { wic.CreateFormatConverter() }.unwrap();
    // SAFETY: converts the frame to 32-bit BGRA, no palette.
    unsafe {
        converter.Initialize(
            &frame,
            &GUID_WICPixelFormat32bppBGRA,
            WICBitmapDitherTypeNone,
            None,
            0.0,
            WICBitmapPaletteTypeCustom,
        )
    }
    .unwrap();
    let (mut w, mut h) = (0, 0);
    // SAFETY: both out pointers are valid.
    unsafe { converter.GetSize(&raw mut w, &raw mut h) }.unwrap();
    let mut pixels = vec![0u8; (w * h * 4) as usize];
    // SAFETY: the buffer holds the whole image at this stride.
    unsafe { converter.CopyPixels(std::ptr::null(), w * 4, &mut pixels) }.unwrap();
    (w, h, pixels)
}

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
