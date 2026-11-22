//! Shared by the screenshot tests: running the screenshot mode and reading
//! PNG files back with Windows Imaging.
#![allow(dead_code)]

pub mod derive;

use std::path::{Path, PathBuf};
use std::process::Command;

use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_ContainerFormatPng, GUID_WICPixelFormat32bppBGRA,
    IWICImagingFactory, WICBitmapDitherTypeNone, WICBitmapEncoderNoCache,
    WICBitmapPaletteTypeCustom, WICDecodeMetadataCacheOnDemand,
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

/// Writes RGBA pixels into a PNG file.
pub fn encode(file: &Path, width: usize, height: usize, rgba: &[u8]) {
    // SAFETY: initializes COM for this test thread; repeated calls are fine.
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    // SAFETY: creates the in-process WIC factory.
    let wic: IWICImagingFactory =
        unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER) }.unwrap();
    let mut bgra = rgba.to_vec();
    for px in bgra.chunks_mut(4) {
        px.swap(0, 2);
    }
    let (w, h) = (
        u32::try_from(width).unwrap(),
        u32::try_from(height).unwrap(),
    );
    // SAFETY: the buffer holds `h` rows of `w * 4` bytes.
    let bitmap =
        unsafe { wic.CreateBitmapFromMemory(w, h, &GUID_WICPixelFormat32bppBGRA, w * 4, &bgra) }
            .unwrap();
    // SAFETY: a plain creation call.
    let stream = unsafe { wic.CreateStream() }.unwrap();
    let path = HSTRING::from(file.as_os_str());
    // SAFETY: `path` lives through the call; the file is created.
    unsafe { stream.InitializeFromFilename(&path, windows::Win32::Foundation::GENERIC_WRITE.0) }
        .unwrap();
    // SAFETY: a plain creation call with a known container format.
    let encoder = unsafe { wic.CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null()) }.unwrap();
    // SAFETY: the stream is open for writing.
    unsafe { encoder.Initialize(&stream, WICBitmapEncoderNoCache) }.unwrap();
    let mut frame = None;
    // SAFETY: `frame` is a valid out pointer; no encoder options.
    unsafe { encoder.CreateNewFrame(&raw mut frame, std::ptr::null_mut()) }.unwrap();
    let frame = frame.unwrap();
    // SAFETY: default options, then the size, the format and the pixels.
    unsafe { frame.Initialize(None) }.unwrap();
    // SAFETY: as above.
    unsafe { frame.SetSize(w, h) }.unwrap();
    let mut format = GUID_WICPixelFormat32bppBGRA;
    // SAFETY: `format` is a valid in/out pointer.
    unsafe { frame.SetPixelFormat(&raw mut format) }.unwrap();
    // SAFETY: the bitmap is complete.
    unsafe { frame.WriteSource(&bitmap, std::ptr::null()) }.unwrap();
    // SAFETY: commits the frame, then the file.
    unsafe { frame.Commit() }.unwrap();
    // SAFETY: as above.
    unsafe { encoder.Commit() }.unwrap();
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
