//! Pictures for the screenshot import: from a file, from the clipboard or
//! from a file picked in the system dialog. Windows Imaging decodes PNG,
//! JPEG and BMP; the pixels go to the app and nowhere else.

use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;

use windows::Win32::Foundation::{GENERIC_READ, HGLOBAL, HWND};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppRGBA, IWICBitmapDecoder, IWICImagingFactory,
    WICBitmapDitherTypeNone, WICBitmapPaletteTypeCustom, WICDecodeMetadataCacheOnDemand,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
};
use windows::Win32::System::DataExchange::{
    CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    RegisterClipboardFormatW,
};
use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::System::Ole::{CF_DIB, CF_DIBV5};
use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR, OFN_PATHMUSTEXIST, OPENFILENAMEW,
};
use windows::Win32::UI::Shell::{DragFinish, DragQueryFileW, HDROP};
use windows::core::{HSTRING, PWSTR, w};

/// Width, height and RGBA pixels.
pub type Pixels = (usize, usize, Vec<u8>);

/// Largest picture accepted, in pixels per side.
const MAX_SIDE: u32 = 16_384;

/// Makes COM usable on this thread (the window's); call once.
pub fn init_com() {
    // SAFETY: initializes COM for the window thread; a second call is
    // harmless.
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
}

fn factory() -> Option<IWICImagingFactory> {
    // SAFETY: creates the in-process WIC factory.
    unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER) }.ok()
}

/// The first frame of a decoder as RGBA pixels.
fn pixels(wic: &IWICImagingFactory, decoder: &IWICBitmapDecoder) -> Option<Pixels> {
    // SAFETY: frame 0 exists in every still picture.
    let frame = unsafe { decoder.GetFrame(0) }.ok()?;
    // SAFETY: a plain creation call.
    let converter = unsafe { wic.CreateFormatConverter() }.ok()?;
    // SAFETY: converts the frame to 32-bit RGBA, no palette.
    unsafe {
        converter.Initialize(
            &frame,
            &GUID_WICPixelFormat32bppRGBA,
            WICBitmapDitherTypeNone,
            None,
            0.0,
            WICBitmapPaletteTypeCustom,
        )
    }
    .ok()?;
    let (mut w, mut h) = (0, 0);
    // SAFETY: both out pointers are valid locals.
    unsafe { converter.GetSize(&raw mut w, &raw mut h) }.ok()?;
    if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE {
        return None;
    }
    let (width, height) = (usize::try_from(w).ok()?, usize::try_from(h).ok()?);
    let mut rgba = vec![0u8; width * height * 4];
    // SAFETY: the buffer holds the whole picture at this stride.
    unsafe { converter.CopyPixels(std::ptr::null(), w * 4, &mut rgba) }.ok()?;
    Some((width, height, rgba))
}

/// Decodes a picture file.
pub fn from_file(path: &std::path::Path) -> Option<Pixels> {
    let wic = factory()?;
    let name = HSTRING::from(path.as_os_str());
    // SAFETY: `name` lives through the call; read access only.
    let decoder = unsafe {
        wic.CreateDecoderFromFilename(&name, None, GENERIC_READ, WICDecodeMetadataCacheOnDemand)
    }
    .ok()?;
    pixels(&wic, &decoder)
}

/// Decodes a picture file held in memory.
fn from_bytes(bytes: &[u8]) -> Option<Pixels> {
    let wic = factory()?;
    // SAFETY: a plain creation call.
    let stream = unsafe { wic.CreateStream() }.ok()?;
    // SAFETY: `bytes` outlives the stream and the decoder, both dropped
    // before this function returns.
    unsafe { stream.InitializeFromMemory(bytes) }.ok()?;
    // SAFETY: the stream is ready to read.
    let decoder = unsafe {
        wic.CreateDecoderFromStream(&stream, std::ptr::null(), WICDecodeMetadataCacheOnDemand)
    }
    .ok()?;
    pixels(&wic, &decoder)
}

/// A device-independent bitmap from the clipboard as a BMP file: the
/// 14-byte file header in front, pointing past the header, the color
/// masks and the palette.
fn bmp_file(dib: &[u8]) -> Option<Vec<u8>> {
    let u32_at = |at: usize| Some(u32::from_le_bytes(dib.get(at..at + 4)?.try_into().ok()?));
    let header = u32_at(0)?;
    let bits = u16::from_le_bytes(dib.get(14..16)?.try_into().ok()?);
    let compression = u32_at(16)?;
    let used = u32_at(32)?;
    // BI_BITFIELDS after a plain 40-byte header: three masks follow it.
    let masks = if header == 40 && compression == 3 {
        12
    } else {
        0
    };
    let palette = if used > 0 {
        used
    } else if bits <= 8 {
        1 << bits
    } else {
        0
    };
    let offset = 14 + header + masks + palette * 4;
    let size = u32::try_from(dib.len() + 14).ok()?;
    let mut file = Vec::with_capacity(dib.len() + 14);
    file.extend_from_slice(b"BM");
    file.extend_from_slice(&size.to_le_bytes());
    file.extend_from_slice(&[0; 4]);
    file.extend_from_slice(&offset.to_le_bytes());
    file.extend_from_slice(dib);
    Some(file)
}

/// The clipboard, open for the life of this value.
struct Open;

impl Drop for Open {
    fn drop(&mut self) {
        // SAFETY: the clipboard was opened by `clipboard`.
        let _ = unsafe { CloseClipboard() };
    }
}

/// The bytes of a clipboard format, if it is there.
fn clipboard_bytes(format: u32) -> Option<Vec<u8>> {
    // SAFETY: the clipboard is open (see `clipboard`); the handle belongs
    // to the clipboard.
    let handle = unsafe { GetClipboardData(format) }.ok()?;
    let global = HGLOBAL(handle.0);
    // SAFETY: a valid global handle from the clipboard.
    let size = unsafe { GlobalSize(global) };
    // SAFETY: as above.
    let data = unsafe { GlobalLock(global) }.cast::<u8>();
    if data.is_null() || size == 0 {
        return None;
    }
    // SAFETY: the locked block is `size` bytes long.
    let bytes = unsafe { std::slice::from_raw_parts(data, size) }.to_vec();
    // SAFETY: matches the lock above.
    let _ = unsafe { GlobalUnlock(global) };
    Some(bytes)
}

/// What the clipboard holds for the import.
pub enum Clip {
    /// A picture, decoded.
    Picture(Pixels),
    /// A picture that could not be decoded.
    Broken,
    /// No picture.
    None,
}

/// The picture on the clipboard: PNG when a program put one there, else a
/// device-independent bitmap.
pub fn clipboard(owner: HWND) -> Clip {
    // SAFETY: `owner` is our window; closed again by `Open`'s drop.
    if unsafe { OpenClipboard(Some(owner)) }.is_err() {
        return Clip::None;
    }
    let _open = Open;
    // SAFETY: registers (or finds) the format by its static name.
    let png = unsafe { RegisterClipboardFormatW(w!("PNG")) };
    // SAFETY: plain queries of the open clipboard.
    let has = |format: u32| unsafe { IsClipboardFormatAvailable(format) }.is_ok();
    let decoded = if png != 0 && has(png) {
        clipboard_bytes(png).and_then(|b| from_bytes(&b))
    } else if has(u32::from(CF_DIBV5.0)) || has(u32::from(CF_DIB.0)) {
        let format = if has(u32::from(CF_DIBV5.0)) {
            CF_DIBV5
        } else {
            CF_DIB
        };
        clipboard_bytes(u32::from(format.0))
            .and_then(|dib| bmp_file(&dib))
            .and_then(|file| from_bytes(&file))
    } else {
        return Clip::None;
    };
    decoded.map_or(Clip::Broken, Clip::Picture)
}

/// The first file dropped on the window; finishes the drop.
pub fn dropped(drop: HDROP) -> Option<PathBuf> {
    let mut name = [0u16; 1024];
    // SAFETY: `drop` comes with WM_DROPFILES; the buffer is ours.
    let len = unsafe { DragQueryFileW(drop, 0, Some(&mut name)) };
    // SAFETY: releases the drop's memory, once.
    unsafe { DragFinish(drop) };
    let len = usize::try_from(len)
        .ok()
        .filter(|&n| n > 0 && n < name.len())?;
    Some(PathBuf::from(std::ffi::OsString::from_wide(&name[..len])))
}

/// Asks for a picture file in the system's open dialog.
pub fn pick(owner: HWND) -> Option<PathBuf> {
    let mut name = [0u16; 1024];
    let filter = w!("Pictures (PNG, JPEG, BMP)\0*.png;*.jpg;*.jpeg;*.bmp\0");
    let mut dialog = OPENFILENAMEW {
        lStructSize: u32::try_from(size_of::<OPENFILENAMEW>()).unwrap_or(0),
        hwndOwner: owner,
        lpstrFilter: filter,
        lpstrFile: PWSTR(name.as_mut_ptr()),
        nMaxFile: u32::try_from(name.len()).unwrap_or(0),
        Flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR,
        ..Default::default()
    };
    // SAFETY: the struct, the buffer and the filter outlive the modal call.
    if !unsafe { GetOpenFileNameW(&raw mut dialog) }.as_bool() {
        return None;
    }
    let len = name.iter().position(|&c| c == 0)?;
    Some(PathBuf::from(std::ffi::OsString::from_wide(&name[..len])))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clipboard_bitmap_becomes_a_bmp_file() {
        // A 1×1 32-bit bitmap with a 40-byte header and its three masks.
        let mut dib = vec![0u8; 40 + 12 + 4];
        dib[0] = 40;
        dib[4] = 1;
        dib[8] = 1;
        dib[12] = 1;
        dib[14] = 32;
        dib[16] = 3;
        let file = bmp_file(&dib).unwrap();
        assert_eq!(&file[..2], b"BM");
        assert_eq!(u32::from_le_bytes(file[2..6].try_into().unwrap()), 70);
        assert_eq!(u32::from_le_bytes(file[10..14].try_into().unwrap()), 66);
        // 8-bit with a full palette.
        let mut dib = vec![0u8; 40 + 1024 + 4];
        dib[0] = 40;
        dib[14] = 8;
        let file = bmp_file(&dib).unwrap();
        assert_eq!(
            u32::from_le_bytes(file[10..14].try_into().unwrap()),
            14 + 40 + 1024
        );
    }
}
