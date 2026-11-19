//! Small things from the system: randomness, language, time, clipboard.

use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND};
use windows::Win32::Globalization::GetUserDefaultUILanguage;
use windows::Win32::Security::Cryptography::{BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::CF_UNICODETEXT;
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

/// Primary language id of Ukrainian.
const LANG_UKRAINIAN: u16 = 0x22;

/// Random bits from the system generator.
pub fn entropy() -> u64 {
    let mut bytes = [0u8; 8];
    // SAFETY: `bytes` is a valid buffer; no algorithm handle is needed with
    // the system-preferred flag.
    let status = unsafe { BCryptGenRandom(None, &mut bytes, BCRYPT_USE_SYSTEM_PREFERRED_RNG) };
    if status.is_err() {
        // Still varied between runs: the time.
        return ticks().unsigned_abs();
    }
    u64::from_le_bytes(bytes)
}

/// The language of the Windows interface as a tag: `uk` for Ukrainian,
/// `en` for everything else.
pub fn language() -> &'static str {
    // SAFETY: a plain query without arguments.
    let id = unsafe { GetUserDefaultUILanguage() };
    if id & 0x3FF == LANG_UKRAINIAN {
        "uk"
    } else {
        "en"
    }
}

/// The high-resolution counter.
pub fn ticks() -> i64 {
    let mut now = 0;
    // SAFETY: `now` is a valid out pointer; the call cannot fail on
    // supported systems.
    let _ = unsafe { QueryPerformanceCounter(&raw mut now) };
    now
}

/// Counter ticks per second.
pub fn tick_rate() -> i64 {
    let mut rate = 0;
    // SAFETY: `rate` is a valid out pointer.
    let _ = unsafe { QueryPerformanceFrequency(&raw mut rate) };
    rate.max(1)
}

/// The clipboard, open for the life of this value.
struct Clipboard;

impl Clipboard {
    fn open(owner: HWND) -> Option<Clipboard> {
        // SAFETY: `owner` is our window or null; closed again on drop.
        unsafe { OpenClipboard(Some(owner)) }
            .ok()
            .map(|()| Clipboard)
    }
}

impl Drop for Clipboard {
    fn drop(&mut self) {
        // SAFETY: the clipboard was opened by `open`.
        let _ = unsafe { CloseClipboard() };
    }
}

/// The text on the clipboard, if any.
pub fn read_clipboard(owner: HWND) -> Option<String> {
    let _open = Clipboard::open(owner)?;
    // SAFETY: the clipboard is open; the handle belongs to the clipboard.
    let handle = unsafe { GetClipboardData(u32::from(CF_UNICODETEXT.0)) }.ok()?;
    let global = HGLOBAL(handle.0);
    // SAFETY: a valid global handle from the clipboard.
    let data = unsafe { GlobalLock(global) }.cast::<u16>();
    if data.is_null() {
        return None;
    }
    let mut len = 0;
    loop {
        let unit = data.wrapping_add(len);
        // SAFETY: clipboard text in this format ends with a NUL, and we
        // stop there, so `unit` is inside the locked block.
        if unsafe { *unit } == 0 {
            break;
        }
        len += 1;
    }
    // SAFETY: `len` units were just read from the locked block.
    let text = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(data, len) });
    // SAFETY: matches the lock above.
    let _ = unsafe { GlobalUnlock(global) };
    Some(text)
}

/// Puts `text` on the clipboard. Returns whether it got there.
pub fn write_clipboard(owner: HWND, text: &str) -> bool {
    let Some(_open) = Clipboard::open(owner) else {
        return false;
    };
    let wide: Vec<u16> = text.encode_utf16().chain([0]).collect();
    // SAFETY: the clipboard is open and ours.
    if unsafe { EmptyClipboard() }.is_err() {
        return false;
    }
    // SAFETY: a plain allocation of the needed size.
    let Ok(global) = (unsafe { GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2) }) else {
        return false;
    };
    // SAFETY: a valid, newly allocated block.
    let data = unsafe { GlobalLock(global) }.cast::<u16>();
    if data.is_null() {
        // SAFETY: the block is ours and unused.
        let _ = unsafe { GlobalFree(Some(global)) };
        return false;
    }
    // SAFETY: the block holds `wide.len()` units and does not overlap `wide`.
    unsafe { std::ptr::copy_nonoverlapping(wide.as_ptr(), data, wide.len()) };
    // SAFETY: matches the lock above.
    let _ = unsafe { GlobalUnlock(global) };
    // SAFETY: on success the clipboard owns the block.
    let set = unsafe { SetClipboardData(u32::from(CF_UNICODETEXT.0), Some(HANDLE(global.0))) };
    if set.is_err() {
        // SAFETY: the clipboard did not take the block.
        let _ = unsafe { GlobalFree(Some(global)) };
        return false;
    }
    true
}
