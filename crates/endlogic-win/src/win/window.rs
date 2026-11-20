//! The window: messages in, frames out.

use std::cell::RefCell;
use std::process::ExitCode;

use endlogic_ui::{
    App, BACKGROUND, Budgets, Config, Cursor, Effect, Event, Insets, Pointer, PointerKind,
};
use windows::Win32::Foundation::{
    COLORREF, D2DERR_RECREATE_TARGET, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM,
};
use windows::Win32::Graphics::Direct2D::Common::{D2D_SIZE_U, D2D1_COLOR_F};
use windows::Win32::Graphics::Direct2D::{
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_HWND_RENDER_TARGET_PROPERTIES,
    D2D1_PRESENT_OPTIONS_NONE, D2D1_RENDER_TARGET_PROPERTIES, D2D1CreateFactory, ID2D1Factory,
    ID2D1HwndRenderTarget, ID2D1SolidColorBrush, ID2D1StrokeStyle,
};
use windows::Win32::Graphics::Dwm::{
    DWMWA_CAPTION_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, InvalidateRect, MONITOR_DEFAULTTOPRIMARY, MonitorFromPoint, PAINTSTRUCT,
    ScreenToClient,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{
    AdjustWindowRectExForDpi, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForMonitor,
    GetDpiForWindow, MDT_EFFECTIVE_DPI, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::Input::Pointer::GetPointerType;
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DispatchMessageW,
    GetClientRect, GetMessageW, GetSystemMetrics, HICON, HTCLIENT, IDC_ARROW, IDC_HAND,
    IDC_SIZEALL, IMAGE_ICON, LR_DEFAULTCOLOR, LoadCursorW, LoadImageW, MINMAXINFO, MSG, PM_REMOVE,
    POINTER_INPUT_TYPE, PT_PEN, PT_TOUCH, PeekMessageW, PostQuitMessage, RegisterClassExW,
    SM_CXICON, SM_CXSMICON, SM_CYICON, SM_CYSMICON, SPI_GETWORKAREA, SW_SHOW, SWP_NOACTIVATE,
    SWP_NOZORDER, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SetCursor, SetWindowPos, ShowWindow,
    SystemParametersInfoW, TranslateMessage, WINDOW_EX_STYLE, WM_CAPTURECHANGED, WM_DESTROY,
    WM_DPICHANGED, WM_ERASEBKGND, WM_GETMINMAXINFO, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MOUSEMOVE, WM_PAINT, WM_POINTERCAPTURECHANGED, WM_POINTERDOWN, WM_POINTERUP,
    WM_POINTERUPDATE, WM_QUIT, WM_SETCURSOR, WM_SIZE, WNDCLASSEXW, WS_OVERLAPPEDWINDOW,
};
use windows::core::{BOOL, PCWSTR, Result, w};

use super::painter::{D2dPainter, polyline_style};
use super::system;
use crate::keys::{self, Modifiers, Press};
use crate::link;

/// The brush's first color; every draw call sets its own.
static BLACK: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};
/// Client area at start, in logical pixels.
const START_SIZE: (f32, f32) = (1280.0, 800.0);
/// Smallest client area, in logical pixels.
const MIN_SIZE: (f32, f32) = (360.0, 480.0);
/// The mouse's pointer id; touch ids are offset past it.
const MOUSE_ID: u32 = 1;
const TOUCH_ID_BASE: u32 = 0x1_0000;
/// Logical pixels per inch.
const BASE_DPI: f32 = 96.0;
/// Longest frame step passed to the app, in seconds.
const MAX_STEP: f32 = 0.1;

/// The render target and the brush made for it.
struct Device {
    target: ID2D1HwndRenderTarget,
    brush: ID2D1SolidColorBrush,
}

/// Resource id of the program's icon group (see `examples/icon.rs`).
const ICON_ID: usize = 1;

/// Everything the window needs between messages.
struct Shell {
    app: App,
    hwnd: HWND,
    factory: ID2D1Factory,
    polyline: ID2D1StrokeStyle,
    device: Option<Device>,
    dpi: u32,
    /// The left button is down and the mouse captured.
    mouse_down: bool,
    cursor: Cursor,
    /// The code the app last asked to show, kept as the "address".
    code: Option<(endlogic_ui::CodeKind, String)>,
    /// A frame is needed: the app said so, or an event arrived.
    pending: bool,
    /// Counter value of the previous frame while frames run back to back.
    last_frame: Option<i64>,
    tick_rate: i64,
}

thread_local! {
    static SHELL: RefCell<Option<Shell>> = const { RefCell::new(None) };
}

/// Runs `f` on the shell if it exists and is not already in use (a message
/// sent while another one is being handled).
fn with_shell<R>(f: impl FnOnce(&mut Shell) -> R) -> Option<R> {
    SHELL.with(|cell| {
        let mut guard = cell.try_borrow_mut().ok()?;
        guard.as_mut().map(f)
    })
}

fn scale(dpi: u32) -> f32 {
    // A DPI is a small count.
    #[allow(clippy::cast_precision_loss)]
    let dpi = dpi as f32;
    dpi / BASE_DPI
}

/// Physical pixels for `logical` at `dpi`.
fn physical(logical: f32, dpi: u32) -> i32 {
    // Window sizes: small and finite.
    #[allow(clippy::cast_possible_truncation)]
    let px = (logical * scale(dpi)).round() as i32;
    px
}

/// The 16 bits of `v` from bit `shift`.
fn word(v: usize, shift: u32) -> u16 {
    // Masked to 16 bits first.
    #[allow(clippy::cast_possible_truncation)]
    let w = ((v >> shift) & 0xFFFF) as u16;
    w
}

/// The low 16 bits, signed, as Windows packs coordinates.
fn low_word(v: isize) -> i32 {
    i32::from(word(v.cast_unsigned(), 0).cast_signed())
}

/// Bits 16 to 31, signed.
fn high_word(v: isize) -> i32 {
    i32::from(word(v.cast_unsigned(), 16).cast_signed())
}

impl Shell {
    fn logical(&self, x: i32, y: i32) -> (f32, f32) {
        let s = scale(self.dpi);
        // Pixel coordinates are small.
        #[allow(clippy::cast_precision_loss)]
        let (x, y) = (x as f32, y as f32);
        (x / s, y / s)
    }

    fn client_size(&self) -> (u32, u32) {
        let mut rect = RECT::default();
        // SAFETY: `hwnd` is our live window; `rect` is a valid out pointer.
        let _ = unsafe { GetClientRect(self.hwnd, &raw mut rect) };
        (
            u32::try_from(rect.right - rect.left).unwrap_or(0),
            u32::try_from(rect.bottom - rect.top).unwrap_or(0),
        )
    }

    fn send(&mut self, event: Event) {
        self.app.handle(event);
        self.run_effects();
        self.pending = true;
    }

    fn resize(&mut self) {
        let (w, h) = self.client_size();
        if w == 0 || h == 0 {
            return;
        }
        if let Some(device) = &self.device {
            // SAFETY: a plain resize of our live target.
            let _ = unsafe {
                device.target.Resize(&D2D_SIZE_U {
                    width: w,
                    height: h,
                })
            };
        }
        let (width, height) =
            self.logical(i32::try_from(w).unwrap_or(0), i32::try_from(h).unwrap_or(0));
        self.send(Event::Resize {
            width,
            height,
            insets: Insets::default(),
        });
    }

    fn device(&mut self) -> Result<&Device> {
        if self.device.is_none() {
            let (width, height) = self.client_size();
            let props = D2D1_RENDER_TARGET_PROPERTIES::default();
            let hwnd_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
                hwnd: self.hwnd,
                pixelSize: D2D_SIZE_U {
                    width: width.max(1),
                    height: height.max(1),
                },
                // Waits for the display's refresh, which paces the frames.
                presentOptions: D2D1_PRESENT_OPTIONS_NONE,
            };
            // SAFETY: both property structs live through the call.
            let target = unsafe {
                self.factory
                    .CreateHwndRenderTarget(&raw const props, &raw const hwnd_props)?
            };
            let dpi = scale(self.dpi) * BASE_DPI;
            // SAFETY: a plain setter; logical pixels become DIPs.
            unsafe { target.SetDpi(dpi, dpi) };
            // SAFETY: a plain creation call on the new target.
            let brush = unsafe { target.CreateSolidColorBrush(&raw const BLACK, None)? };
            self.device = Some(Device { target, brush });
        }
        self.device
            .as_ref()
            .ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_FAIL))
    }

    /// Draws one frame and runs the effects it produced.
    fn render(&mut self) {
        let now = system::ticks();
        let dt = self.last_frame.map_or(0.0, |last| {
            // A frame step: small.
            #[allow(clippy::cast_precision_loss)]
            let dt = (now - last) as f32 / self.tick_rate as f32;
            dt.clamp(0.0, MAX_STEP)
        });
        let Ok(device) = self.device() else {
            return;
        };
        let (target, brush) = (device.target.clone(), device.brush.clone());
        let base: windows::Win32::Graphics::Direct2D::ID2D1RenderTarget = target.clone().into();
        // SAFETY: starts drawing on our target; ended below.
        unsafe { target.BeginDraw() };
        let busy = {
            let mut painter = D2dPainter::new(&base, &self.factory, &brush, &self.polyline);
            self.app.frame(dt, &mut painter)
        };
        // SAFETY: ends the drawing begun above.
        let ended = unsafe { target.EndDraw(None, None) };
        if let Err(err) = ended
            && err.code() == D2DERR_RECREATE_TARGET
        {
            // The device went away (display change, sleep, remote desktop):
            // make a new target and draw again.
            self.device = None;
            self.pending = true;
            self.last_frame = None;
            return;
        }
        self.pending = false;
        self.last_frame = busy.then_some(now);
        self.pending = busy;
        self.run_effects();
    }

    fn run_effects(&mut self) {
        for effect in self.app.take_effects() {
            match effect {
                Effect::SetCode(kind, code) => self.code = Some((kind, code)),
                Effect::CopyLink(kind, code) => {
                    let ok = system::write_clipboard(self.hwnd, &link::share_link(kind, &code));
                    self.app.handle(Event::CopyFinished(ok));
                }
                Effect::SaveSetting { .. } => {}
                Effect::SetCursor(cursor) => {
                    self.cursor = cursor;
                    apply_cursor(cursor);
                }
            }
        }
    }

    fn paste(&mut self) {
        let parsed = system::read_clipboard(self.hwnd).and_then(|t| link::parse_link(&t));
        if let Some((kind, code)) = parsed {
            self.send(Event::CodeChanged(kind, code));
        }
    }
}

fn apply_cursor(cursor: Cursor) {
    let id = match cursor {
        Cursor::Default => IDC_ARROW,
        Cursor::Pointer => IDC_HAND,
        Cursor::Grab | Cursor::Grabbing => IDC_SIZEALL,
    };
    // SAFETY: loading a shared system cursor.
    if let Ok(handle) = unsafe { LoadCursorW(None, id) } {
        // SAFETY: setting a valid system cursor.
        unsafe { SetCursor(Some(handle)) };
    }
}

fn key_down(v: u16) -> bool {
    // SAFETY: a plain query of the key state for this thread.
    let state = unsafe { GetKeyState(i32::from(v)) };
    state < 0
}

fn modifiers() -> Modifiers {
    Modifiers {
        ctrl: key_down(VK_CONTROL.0),
        alt: key_down(VK_MENU.0),
        shift: key_down(VK_SHIFT.0),
        win: key_down(VK_LWIN.0) || key_down(VK_RWIN.0),
    }
}

/// A mouse pointer event at the coordinates packed in `lparam`.
fn mouse(shell: &Shell, lparam: LPARAM) -> Pointer {
    let (x, y) = shell.logical(low_word(lparam.0), high_word(lparam.0));
    Pointer {
        id: MOUSE_ID,
        x,
        y,
        kind: PointerKind::Mouse,
    }
}

/// A touch or pen pointer from a `WM_POINTER*` message, or `None` for a
/// mouse (handled by the mouse messages).
fn touch(shell: &Shell, wparam: WPARAM, lparam: LPARAM) -> Option<Pointer> {
    let id = u32::from(word(wparam.0, 0));
    let mut kind = POINTER_INPUT_TYPE::default();
    // SAFETY: `id` comes from the message; `kind` is a valid out pointer.
    unsafe { GetPointerType(id, &raw mut kind) }.ok()?;
    if kind != PT_TOUCH && kind != PT_PEN {
        return None;
    }
    let mut p = POINT {
        x: low_word(lparam.0),
        y: high_word(lparam.0),
    };
    // SAFETY: our live window; `p` is a valid in/out pointer.
    let _ = unsafe { ScreenToClient(shell.hwnd, &raw mut p) };
    let (x, y) = shell.logical(p.x, p.y);
    Some(Pointer {
        id: TOUCH_ID_BASE + id,
        x,
        y,
        kind: PointerKind::Touch,
    })
}

fn default_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: passes the message on unchanged.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

#[allow(clippy::too_many_lines)]
extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let handled = |r: Option<()>| {
        if r.is_some() {
            LRESULT(0)
        } else {
            default_proc(hwnd, msg, wparam, lparam)
        }
    };
    match msg {
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            // SAFETY: our window, between BeginPaint and EndPaint.
            unsafe { BeginPaint(hwnd, &raw mut ps) };
            with_shell(Shell::render);
            // SAFETY: matches BeginPaint above.
            let _ = unsafe { EndPaint(hwnd, &raw const ps) };
            LRESULT(0)
        }
        WM_SIZE => {
            with_shell(Shell::resize);
            // SAFETY: marks the whole client area for a repaint.
            let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
            LRESULT(0)
        }
        WM_DPICHANGED => {
            let dpi = u32::from(word(wparam.0, 0));
            with_shell(|s| {
                s.dpi = dpi;
                if let Some(device) = &s.device {
                    let d = scale(dpi) * BASE_DPI;
                    // SAFETY: a plain setter on our target.
                    unsafe { device.target.SetDpi(d, d) };
                }
            });
            // SAFETY: Windows passes the suggested window rectangle here.
            let rect = unsafe { *(lparam.0 as *const RECT) };
            // SAFETY: moves our own window; WM_SIZE follows.
            let _ = unsafe {
                SetWindowPos(
                    hwnd,
                    None,
                    rect.left,
                    rect.top,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                )
            };
            with_shell(Shell::resize);
            LRESULT(0)
        }
        WM_GETMINMAXINFO => {
            // SAFETY: a plain query for our window.
            let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: physical(MIN_SIZE.0, dpi),
                bottom: physical(MIN_SIZE.1, dpi),
            };
            // SAFETY: `rect` is a valid in/out pointer.
            let _ = unsafe {
                AdjustWindowRectExForDpi(
                    &raw mut rect,
                    WS_OVERLAPPEDWINDOW,
                    false,
                    WINDOW_EX_STYLE(0),
                    dpi,
                )
            };
            // SAFETY: Windows passes a MINMAXINFO for us to fill in.
            let info = unsafe { &mut *(lparam.0 as *mut MINMAXINFO) };
            info.ptMinTrackSize = POINT {
                x: rect.right - rect.left,
                y: rect.bottom - rect.top,
            };
            LRESULT(0)
        }
        WM_LBUTTONDOWN => handled(with_shell(|s| {
            s.mouse_down = true;
            // SAFETY: captures the mouse for our window until the button
            // goes up, so a release outside is not lost.
            unsafe { SetCapture(s.hwnd) };
            let p = mouse(s, lparam);
            s.send(Event::PointerDown(p));
        })),
        WM_MOUSEMOVE => handled(with_shell(|s| {
            let p = mouse(s, lparam);
            s.send(Event::PointerMove(p));
        })),
        WM_LBUTTONUP => handled(with_shell(|s| {
            let p = mouse(s, lparam);
            if s.mouse_down {
                s.mouse_down = false;
                s.send(Event::PointerUp(p));
            }
        }))
        .and_then_release(),
        WM_CAPTURECHANGED => handled(with_shell(|s| {
            if s.mouse_down {
                // Something else took the mouse in the middle of a press.
                s.mouse_down = false;
                s.send(Event::PointerCancel(Pointer {
                    id: MOUSE_ID,
                    x: 0.0,
                    y: 0.0,
                    kind: PointerKind::Mouse,
                }));
            }
        })),
        WM_POINTERDOWN | WM_POINTERUPDATE | WM_POINTERUP | WM_POINTERCAPTURECHANGED => {
            let done = with_shell(|s| {
                let p = touch(s, wparam, lparam)?;
                let event = match msg {
                    WM_POINTERDOWN => Event::PointerDown(p),
                    WM_POINTERUPDATE => Event::PointerMove(p),
                    WM_POINTERUP => Event::PointerUp(p),
                    _ => Event::PointerCancel(p),
                };
                s.send(event);
                Some(())
            })
            .flatten();
            handled(done)
        }
        WM_KEYDOWN => {
            let scan = u32::try_from((lparam.0 >> 16) & 0xFF).unwrap_or(0);
            let extended = (lparam.0 >> 24) & 1 == 1;
            let repeat = (lparam.0 >> 30) & 1 == 1;
            let press = if extended {
                Press::Ignore
            } else {
                keys::press(scan, repeat, modifiers())
            };
            match press {
                Press::Key(key) => handled(with_shell(|s| s.send(Event::Key(key)))),
                Press::Paste => handled(with_shell(Shell::paste)),
                Press::Ignore => default_proc(hwnd, msg, wparam, lparam),
            }
        }
        WM_SETCURSOR if u32::from(word(lparam.0.cast_unsigned(), 0)) == HTCLIENT => {
            let cursor = with_shell(|s| s.cursor).unwrap_or_default();
            apply_cursor(cursor);
            LRESULT(1)
        }
        WM_DESTROY => {
            // SAFETY: ends the message loop.
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        _ => default_proc(hwnd, msg, wparam, lparam),
    }
}

/// Releases the mouse after a button-up message has been handled.
trait ReleaseAfter {
    fn and_then_release(self) -> LRESULT;
}

impl ReleaseAfter for LRESULT {
    fn and_then_release(self) -> LRESULT {
        // SAFETY: releases a capture we may hold; harmless otherwise.
        let _ = unsafe { ReleaseCapture() };
        self
    }
}

/// The work area of the primary monitor and its DPI.
fn primary_monitor() -> (RECT, u32) {
    let mut area = RECT::default();
    // SAFETY: `area` is a valid out pointer of the right size.
    let _ = unsafe {
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some((&raw mut area).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS::default(),
        )
    };
    // SAFETY: a plain lookup of the primary monitor.
    let monitor = unsafe { MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY) };
    let (mut dpi_x, mut dpi_y) = (96, 96);
    // SAFETY: both out pointers are valid locals.
    let _ = unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &raw mut dpi_x, &raw mut dpi_y) };
    (area, dpi_x.max(96))
}

fn dark_title(hwnd: HWND) {
    let on = BOOL::from(true);
    // SAFETY: the attribute takes a BOOL of this size; failure (older
    // Windows) only leaves the title light.
    let _ = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            (&raw const on).cast(),
            u32::try_from(size_of::<BOOL>()).unwrap_or(4),
        )
    };
    let c = BACKGROUND;
    let colorref = COLORREF(u32::from(c.r) | (u32::from(c.g) << 8) | (u32::from(c.b) << 16));
    // SAFETY: the attribute takes a COLORREF; Windows 10 ignores it.
    let _ = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_CAPTION_COLOR,
            (&raw const colorref).cast(),
            u32::try_from(size_of::<COLORREF>()).unwrap_or(4),
        )
    };
}

fn create_window() -> Result<(HWND, u32)> {
    // SAFETY: the module of this program.
    let instance = unsafe { GetModuleHandleW(None)? };
    let icon = |metric_x, metric_y| {
        // SAFETY: a plain query of the system's icon width.
        let w = unsafe { GetSystemMetrics(metric_x) };
        // SAFETY: as above, the height.
        let h = unsafe { GetSystemMetrics(metric_y) };
        // SAFETY: loads icon group 1 of this program at that size; a
        // missing icon gives an error and the default icon is used.
        unsafe {
            LoadImageW(
                Some(instance.into()),
                PCWSTR(std::ptr::without_provenance(ICON_ID)),
                IMAGE_ICON,
                w,
                h,
                LR_DEFAULTCOLOR,
            )
        }
        .map_or_else(|_| HICON::default(), |h| HICON(h.0))
    };
    let class = WNDCLASSEXW {
        cbSize: u32::try_from(std::mem::size_of::<WNDCLASSEXW>()).unwrap_or(0),
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(window_proc),
        hInstance: instance.into(),
        hIcon: icon(SM_CXICON, SM_CYICON),
        hIconSm: icon(SM_CXSMICON, SM_CYSMICON),
        lpszClassName: w!("endlogic"),
        ..Default::default()
    };
    // SAFETY: the class struct and its static name outlive the call.
    if unsafe { RegisterClassExW(&raw const class) } == 0 {
        return Err(windows::core::Error::from_thread());
    }
    let (area, dpi) = primary_monitor();
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: physical(START_SIZE.0, dpi),
        bottom: physical(START_SIZE.1, dpi),
    };
    // SAFETY: `rect` is a valid in/out pointer.
    unsafe {
        AdjustWindowRectExForDpi(
            &raw mut rect,
            WS_OVERLAPPEDWINDOW,
            false,
            WINDOW_EX_STYLE(0),
            dpi,
        )?;
    };
    let (w, h) = (
        (rect.right - rect.left).min(area.right - area.left),
        (rect.bottom - rect.top).min(area.bottom - area.top),
    );
    let (x, y) = (
        area.left + (area.right - area.left - w) / 2,
        area.top + (area.bottom - area.top - h) / 2,
    );
    // SAFETY: the class is registered; the strings are static.
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("endlogic"),
            w!("endlogic"),
            WS_OVERLAPPEDWINDOW,
            if w > 0 { x } else { CW_USEDEFAULT },
            y,
            w,
            h,
            None,
            None,
            Some(instance.into()),
            None,
        )?
    };
    Ok((hwnd, dpi))
}

/// Opens the window and runs until it is closed.
pub fn run(start: Option<&str>) -> ExitCode {
    // SAFETY: called once, before any window exists.
    let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    match open(start) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("endlogic-win: {err}");
            ExitCode::FAILURE
        }
    }
}

fn open(start: Option<&str>) -> Result<()> {
    let (hwnd, _) = create_window()?;
    // SAFETY: a plain query for our window.
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    dark_title(hwnd);
    // SAFETY: plain factory creation.
    let factory: ID2D1Factory =
        unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? };
    let polyline = polyline_style(&factory)?;
    let mut client = RECT::default();
    // SAFETY: our live window; `client` is a valid out pointer.
    unsafe { GetClientRect(hwnd, &raw mut client)? };
    let s = scale(dpi);
    // Pixel sizes are small.
    #[allow(clippy::cast_precision_loss)]
    let (width, height) = (
        (client.right - client.left) as f32 / s,
        (client.bottom - client.top) as f32 / s,
    );
    let config = Config {
        width,
        height,
        insets: Insets::default(),
        entropy: system::entropy(),
        settings: Vec::new(),
        system_language: system::language().to_owned(),
        code: start.and_then(link::parse_link),
        touch: false,
        budgets: Budgets::default(),
    };
    let shell = Shell {
        app: App::new(config),
        hwnd,
        factory,
        polyline,
        device: None,
        dpi,
        mouse_down: false,
        cursor: Cursor::Default,
        code: None,
        pending: true,
        last_frame: None,
        tick_rate: system::tick_rate(),
    };
    SHELL.with(|cell| *cell.borrow_mut() = Some(shell));
    with_shell(Shell::run_effects);
    // SAFETY: shows our new window.
    let _ = unsafe { ShowWindow(hwnd, SW_SHOW) };
    message_loop();
    SHELL.with(|cell| cell.borrow_mut().take());
    Ok(())
}

/// Sleeps in the message queue while nothing moves; while the app wants
/// frames, handles waiting messages and draws one frame per refresh.
fn message_loop() {
    let mut msg = MSG::default();
    loop {
        let frames = with_shell(|s| s.pending).unwrap_or(false);
        if frames {
            // SAFETY: `msg` is a valid out pointer; any window of ours.
            while unsafe { PeekMessageW(&raw mut msg, None, 0, 0, PM_REMOVE) }.as_bool() {
                if msg.message == WM_QUIT {
                    return;
                }
                dispatch(&msg);
            }
            with_shell(Shell::render);
        } else {
            // SAFETY: as above; blocks until a message arrives.
            let got = unsafe { GetMessageW(&raw mut msg, None, 0, 0) };
            if got.0 <= 0 {
                return;
            }
            dispatch(&msg);
        }
    }
}

fn dispatch(msg: &MSG) {
    // SAFETY: a message just taken from our queue.
    let _ = unsafe { TranslateMessage(msg) };
    // SAFETY: as above.
    unsafe { DispatchMessageW(msg) };
}
