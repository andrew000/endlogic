//! Shared by the screenshot tests: running the screenshot mode and reading
//! PNG files back with Windows Imaging.
#![allow(dead_code)]

pub mod derive;

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

/// A file in this test run's own temporary folder.
pub fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("endlogic-win-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

/// Runs the screenshot mode into `file` with `args`.
pub fn shot(file: &Path, args: &[&str]) -> std::process::ExitStatus {
    Command::new(env!("CARGO_BIN_EXE_endlogic-win"))
        .arg("--screenshot")
        .arg(file)
        .args(args)
        .status()
        .unwrap()
}

/// Width, height and pixels (BGRA) of a PNG file.
pub fn decode(file: &Path) -> (u32, u32, Vec<u8>) {
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
