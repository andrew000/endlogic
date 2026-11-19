//! Browser shell for the endlogic trainer.
//!
//! Everything visible and every rule lives in `endlogic-ui`. This crate only
//! implements its [`Painter`] on a Canvas 2D context, forwards DOM events,
//! carries out the effects the app asks for and runs the frame loop, which
//! sleeps whenever the app needs no frames.

use std::cell::RefCell;
use std::f64::consts::TAU;
use std::rc::{Rc, Weak};

use endlogic_ui::{
    Align, App, Config, Cursor, Effect, Event, Font, FontFamily, Insets, Key, Painter, Point,
    Pointer, PointerKind, Rect, Rgba, SETTING_KEYS,
};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{
    CanvasRenderingContext2d, HtmlCanvasElement, KeyboardEvent, PointerEvent, Storage, Window,
};

/// Id of the canvas in `index.html`.
const CANVAS_ID: &str = "endlogic";
/// Prefix of the settings keys in `localStorage`.
const STORAGE_PREFIX: &str = "endlogic.";
/// Prefix of the puzzle code in the address fragment.
const CODE_FRAGMENT: &str = "#p=";

const SANS: &str =
    "system-ui, -apple-system, \"Segoe UI\", Roboto, \"Helvetica Neue\", Arial, sans-serif";
const MONO: &str =
    "ui-monospace, \"Cascadia Mono\", \"SF Mono\", Menlo, Consolas, \"Liberation Mono\", monospace";

/// Entry point, run when the module is loaded.
///
/// # Errors
///
/// A JavaScript error if the page lacks the canvas or a browser API.
#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    std::panic::set_hook(Box::new(|info| {
        web_sys::console::error_1(&format!("endlogic crashed: {info}").into());
    }));
    let window = web_sys::window().ok_or("no window")?;
    let canvas: HtmlCanvasElement = window
        .document()
        .ok_or("no document")?
        .get_element_by_id(CANVAS_ID)
        .ok_or("no canvas")?
        .dyn_into()?;
    let ctx: CanvasRenderingContext2d = canvas
        .get_context("2d")?
        .ok_or("no 2d context")?
        .dyn_into()?;

    let (width, height) = css_size(&canvas);
    let probe = inset_probe(&window);
    let insets = probe
        .as_ref()
        .map_or_else(Insets::default, |probe| safe_insets(&window, probe));
    let storage = window.local_storage().ok().flatten();
    let config = Config {
        width,
        height,
        insets,
        entropy: entropy(&window)?,
        settings: load_settings(storage.as_ref()),
        system_language: window.navigator().language().unwrap_or_default(),
        code: code_from_address(&window),
        touch: window
            .match_media("(pointer: coarse)")
            .ok()
            .flatten()
            .is_some_and(|m| m.matches()),
        share_prefix: share_prefix(&window),
    };

    let shell = Rc::new(RefCell::new(Shell {
        app: App::new(config),
        window: window.clone(),
        canvas: canvas.clone(),
        painter: CanvasPainter { ctx },
        storage,
        size: (0, 0),
        insets,
        probe,
        frame_pending: false,
        last_frame: None,
        frame_callback: None,
        this: Weak::new(),
    }));
    shell.borrow_mut().this = Rc::downgrade(&shell);
    install_frame_callback(&shell);
    listen(&shell)?;
    Shell::request_frame(&shell);
    Ok(())
}

/// Adds an invisible element whose padding is the safe-area insets.
fn inset_probe(window: &Window) -> Option<web_sys::Element> {
    let document = window.document()?;
    let probe = document.create_element("div").ok()?;
    probe
        .set_attribute(
            "style",
            "position:fixed;left:0;top:0;width:0;height:0;visibility:hidden;pointer-events:none;\
             padding:env(safe-area-inset-top) env(safe-area-inset-right) \
             env(safe-area-inset-bottom) env(safe-area-inset-left)",
        )
        .ok()?;
    document.body()?.append_child(&probe).ok()?;
    Some(probe)
}

/// The safe-area insets in CSS pixels, read from the probe's padding.
fn safe_insets(window: &Window, probe: &web_sys::Element) -> Insets {
    let Ok(Some(style)) = window.get_computed_style(probe) else {
        return Insets::default();
    };
    let side = |name: &str| {
        style
            .get_property_value(name)
            .ok()
            .and_then(|v| v.trim().trim_end_matches("px").parse::<f32>().ok())
            .unwrap_or(0.0)
            .max(0.0)
    };
    Insets {
        top: side("padding-top"),
        right: side("padding-right"),
        bottom: side("padding-bottom"),
        left: side("padding-left"),
    }
}

/// The canvas size in CSS pixels.
fn css_size(canvas: &HtmlCanvasElement) -> (f32, f32) {
    let rect = canvas.get_bounding_client_rect();
    (to_f32(rect.width()), to_f32(rect.height()))
}

#[allow(clippy::cast_possible_truncation)]
fn to_f32(x: f64) -> f32 {
    x as f32
}

fn entropy(window: &Window) -> Result<u64, JsValue> {
    let mut bytes = [0u8; 8];
    window
        .crypto()?
        .get_random_values_with_u8_array(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

fn load_settings(storage: Option<&Storage>) -> Vec<(String, String)> {
    let Some(storage) = storage else {
        return Vec::new();
    };
    SETTING_KEYS
        .iter()
        .filter_map(|&key| {
            let value = storage
                .get_item(&format!("{STORAGE_PREFIX}{key}"))
                .ok()
                .flatten()?;
            Some((key.to_owned(), value))
        })
        .collect()
}

fn code_from_address(window: &Window) -> Option<String> {
    let hash = window.location().hash().ok()?;
    hash.strip_prefix(CODE_FRAGMENT)
        .filter(|code| !code.is_empty())
        .map(str::to_owned)
}

fn share_prefix(window: &Window) -> String {
    let location = window.location();
    let part = |r: Result<String, JsValue>| r.unwrap_or_default();
    format!(
        "{}{}{}{CODE_FRAGMENT}",
        part(location.origin()),
        part(location.pathname()),
        part(location.search())
    )
}

/// The app plus what the shell needs to serve it.
struct Shell {
    app: App,
    window: Window,
    canvas: HtmlCanvasElement,
    painter: CanvasPainter,
    storage: Option<Storage>,
    /// Canvas backing store size in device pixels.
    size: (u32, u32),
    /// Safe-area insets last passed to the app.
    insets: Insets,
    /// An invisible element padded by `env(safe-area-inset-*)`, read back
    /// to learn the insets.
    probe: Option<web_sys::Element>,
    frame_pending: bool,
    /// Time of the previous frame while frames run back to back.
    last_frame: Option<f64>,
    frame_callback: Option<Closure<dyn FnMut(f64)>>,
    /// The shell itself, for callbacks that arrive later.
    this: Weak<RefCell<Shell>>,
}

type SharedShell = Rc<RefCell<Shell>>;

impl Shell {
    fn request_frame(shell: &SharedShell) {
        let mut s = shell.borrow_mut();
        if s.frame_pending {
            return;
        }
        let Some(callback) = &s.frame_callback else {
            return;
        };
        let requested = s
            .window
            .request_animation_frame(callback.as_ref().unchecked_ref())
            .is_ok();
        s.frame_pending = requested;
    }

    /// Passes an event to the app and makes sure a frame follows.
    fn dispatch(shell: &SharedShell, event: Event) {
        {
            let mut s = shell.borrow_mut();
            s.app.handle(event);
            s.run_effects();
        }
        Shell::request_frame(shell);
    }

    /// Matches the canvas backing store to its CSS size and the screen's
    /// pixel ratio, telling the app about any change.
    fn sync_size(&mut self) {
        let (width, height) = css_size(&self.canvas);
        let ratio = self.window.device_pixel_ratio();
        let device = |css: f32| {
            // Pixel sizes of a window: small and non-negative.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let px = (f64::from(css) * ratio).round().max(1.0) as u32;
            px
        };
        let size = (device(width), device(height));
        let insets = self
            .probe
            .as_ref()
            .map_or_else(Insets::default, |probe| safe_insets(&self.window, probe));
        if size != self.size || insets != self.insets {
            self.size = size;
            self.insets = insets;
            self.canvas.set_width(size.0);
            self.canvas.set_height(size.1);
            self.app.handle(Event::Resize {
                width,
                height,
                insets,
            });
        }
        // Logical pixels from here on; the backing store may have been reset.
        let _ = self
            .painter
            .ctx
            .set_transform(ratio, 0.0, 0.0, ratio, 0.0, 0.0);
    }

    fn frame(shell: &SharedShell, time: f64) {
        let busy = {
            let mut s = shell.borrow_mut();
            s.frame_pending = false;
            s.sync_size();
            let dt = s
                .last_frame
                .map_or(0.0, |last| to_f32((time - last) / 1000.0));
            let Shell { app, painter, .. } = &mut *s;
            let busy = app.frame(dt, painter);
            s.run_effects();
            s.last_frame = busy.then_some(time);
            busy
        };
        if busy {
            Shell::request_frame(shell);
        }
    }

    /// Writes `text` to the clipboard and reports the outcome to the app
    /// with [`Event::CopyFinished`] once the browser has answered.
    fn copy(&self, text: &str) {
        let navigator = self.window.navigator();
        let available = js_sys::Reflect::has(&navigator, &"clipboard".into()).unwrap_or(false);
        let report = |ok: bool| {
            let this = self.this.clone();
            Closure::<dyn FnMut(JsValue)>::new(move |result: JsValue| {
                if !ok {
                    web_sys::console::warn_2(&"clipboard write refused:".into(), &result);
                }
                if let Some(shell) = this.upgrade() {
                    Shell::dispatch(&shell, Event::CopyFinished(ok));
                }
            })
        };
        let (done, refused) = (report(true), report(false));
        if available {
            let _ = navigator
                .clipboard()
                .write_text(text)
                .then2(&done, &refused);
        } else {
            let _ = js_sys::Promise::reject(&"no clipboard API".into()).catch(&refused);
        }
        // Two small closures per share; they must outlive the promise.
        done.forget();
        refused.forget();
    }

    fn run_effects(&mut self) {
        for effect in self.app.take_effects() {
            match effect {
                Effect::SetCode(code) => {
                    let url = format!("{CODE_FRAGMENT}{code}");
                    if let Ok(history) = self.window.history() {
                        let _ = history.replace_state_with_url(&JsValue::NULL, "", Some(&url));
                    }
                }
                Effect::CopyText(text) => self.copy(&text),
                Effect::SaveSetting { key, value } => {
                    if let Some(storage) = &self.storage {
                        let _ = storage.set_item(&format!("{STORAGE_PREFIX}{key}"), &value);
                    }
                }
                Effect::SetCursor(cursor) => {
                    let name = match cursor {
                        Cursor::Default => "default",
                        Cursor::Pointer => "pointer",
                        Cursor::Grab => "grab",
                        Cursor::Grabbing => "grabbing",
                    };
                    let _ = self.canvas.style().set_property("cursor", name);
                }
            }
        }
    }
}

fn install_frame_callback(shell: &SharedShell) {
    let weak = Rc::downgrade(shell);
    let callback = Closure::<dyn FnMut(f64)>::new(move |time: f64| {
        if let Some(shell) = weak.upgrade() {
            Shell::frame(&shell, time);
        }
    });
    shell.borrow_mut().frame_callback = Some(callback);
}

fn pointer(canvas: &HtmlCanvasElement, e: &PointerEvent) -> Pointer {
    let rect = canvas.get_bounding_client_rect();
    let kind = match e.pointer_type().as_str() {
        "touch" | "pen" => PointerKind::Touch,
        _ => PointerKind::Mouse,
    };
    Pointer {
        id: u32::try_from(e.pointer_id()).unwrap_or(0),
        x: to_f32(f64::from(e.client_x()) - rect.left()),
        y: to_f32(f64::from(e.client_y()) - rect.top()),
        kind,
    }
}

/// Adds an event listener that lives as long as the page.
fn on<E: JsCast + 'static>(
    target: &web_sys::EventTarget,
    name: &str,
    handler: impl FnMut(E) + 'static,
) -> Result<(), JsValue> {
    let mut handler = handler;
    let closure = Closure::<dyn FnMut(web_sys::Event)>::new(move |e: web_sys::Event| {
        if let Ok(e) = e.dyn_into::<E>() {
            handler(e);
        }
    });
    target.add_event_listener_with_callback(name, closure.as_ref().unchecked_ref())?;
    closure.forget();
    Ok(())
}

fn listen(shell: &SharedShell) -> Result<(), JsValue> {
    let (window, canvas) = {
        let s = shell.borrow();
        (s.window.clone(), s.canvas.clone())
    };
    let canvas_target: &web_sys::EventTarget = canvas.as_ref();

    let (s, c) = (shell.clone(), canvas.clone());
    on(canvas_target, "pointerdown", move |e: PointerEvent| {
        // Only the primary button: no right button, no middle button.
        if e.pointer_type() == "mouse" && e.button() != 0 {
            return;
        }
        e.prevent_default();
        let _ = c.set_pointer_capture(e.pointer_id());
        Shell::dispatch(&s, Event::PointerDown(pointer(&c, &e)));
    })?;
    let (s, c) = (shell.clone(), canvas.clone());
    on(canvas_target, "pointermove", move |e: PointerEvent| {
        Shell::dispatch(&s, Event::PointerMove(pointer(&c, &e)));
    })?;
    let (s, c) = (shell.clone(), canvas.clone());
    on(canvas_target, "pointerup", move |e: PointerEvent| {
        if e.pointer_type() == "mouse" && e.button() != 0 {
            return;
        }
        Shell::dispatch(&s, Event::PointerUp(pointer(&c, &e)));
    })?;
    let (s, c) = (shell.clone(), canvas.clone());
    on(canvas_target, "pointercancel", move |e: PointerEvent| {
        Shell::dispatch(&s, Event::PointerCancel(pointer(&c, &e)));
    })?;

    let window_target: &web_sys::EventTarget = window.as_ref();
    let s = shell.clone();
    on(window_target, "keydown", move |e: KeyboardEvent| {
        if e.ctrl_key() || e.meta_key() || e.alt_key() {
            return;
        }
        // Physical keys, so `R` works in every keyboard layout.
        let key = match e.code().as_str() {
            "KeyR" if !e.repeat() => Key::R,
            "Tab" => {
                e.prevent_default();
                if e.repeat() {
                    return;
                }
                Key::Tab
            }
            "Escape" => Key::Escape,
            _ => return,
        };
        Shell::dispatch(&s, Event::Key(key));
    })?;
    let s = shell.clone();
    on(window_target, "resize", move |_: web_sys::Event| {
        Shell::request_frame(&s);
    })?;
    let s = shell.clone();
    on(window_target, "hashchange", move |_: web_sys::Event| {
        let code = {
            let shell = s.borrow();
            code_from_address(&shell.window)
        };
        if let Some(code) = code {
            Shell::dispatch(&s, Event::CodeChanged(code));
        }
    })?;
    Ok(())
}

/// [`Painter`] on a Canvas 2D context.
struct CanvasPainter {
    ctx: CanvasRenderingContext2d,
}

fn css_color(c: Rgba) -> String {
    format!(
        "rgba({},{},{},{:.3})",
        c.r,
        c.g,
        c.b,
        f32::from(c.a) / 255.0
    )
}

fn css_font(font: Font) -> String {
    let family = match font.family {
        FontFamily::Sans => SANS,
        FontFamily::Mono => MONO,
    };
    let weight = if font.bold { 700 } else { 400 };
    format!("{weight} {:.2}px {family}", font.size)
}

impl CanvasPainter {
    fn rounded_path(&self, rect: Rect, radius: f32) {
        let ctx = &self.ctx;
        let (x, y, w, h) = (
            f64::from(rect.x),
            f64::from(rect.y),
            f64::from(rect.w),
            f64::from(rect.h),
        );
        let radius = f64::from(radius).clamp(0.0, w.min(h) / 2.0);
        ctx.begin_path();
        if radius <= 0.0 {
            ctx.rect(x, y, w, h);
            return;
        }
        ctx.move_to(x + radius, y);
        let _ = ctx.arc_to(x + w, y, x + w, y + h, radius);
        let _ = ctx.arc_to(x + w, y + h, x, y + h, radius);
        let _ = ctx.arc_to(x, y + h, x, y, radius);
        let _ = ctx.arc_to(x, y, x + w, y, radius);
        ctx.close_path();
    }
}

impl Painter for CanvasPainter {
    fn fill_rect(&mut self, rect: Rect, radius: f32, color: Rgba) {
        self.ctx.set_fill_style_str(&css_color(color));
        if radius <= 0.0 {
            self.ctx.fill_rect(
                f64::from(rect.x),
                f64::from(rect.y),
                f64::from(rect.w),
                f64::from(rect.h),
            );
        } else {
            self.rounded_path(rect, radius);
            self.ctx.fill();
        }
    }

    fn stroke_rect(&mut self, rect: Rect, radius: f32, width: f32, color: Rgba) {
        self.ctx.set_stroke_style_str(&css_color(color));
        self.ctx.set_line_width(f64::from(width));
        self.rounded_path(rect, radius);
        self.ctx.stroke();
    }

    fn line(&mut self, from: Point, to: Point, width: f32, color: Rgba) {
        let ctx = &self.ctx;
        ctx.set_stroke_style_str(&css_color(color));
        ctx.set_line_width(f64::from(width));
        ctx.set_line_cap("butt");
        ctx.begin_path();
        ctx.move_to(f64::from(from.x), f64::from(from.y));
        ctx.line_to(f64::from(to.x), f64::from(to.y));
        ctx.stroke();
    }

    fn fill_circle(&mut self, center: Point, radius: f32, color: Rgba) {
        let ctx = &self.ctx;
        ctx.set_fill_style_str(&css_color(color));
        ctx.begin_path();
        let _ = ctx.arc(
            f64::from(center.x),
            f64::from(center.y),
            f64::from(radius.max(0.0)),
            0.0,
            TAU,
        );
        ctx.fill();
    }

    fn stroke_circle(&mut self, center: Point, radius: f32, width: f32, color: Rgba) {
        let ctx = &self.ctx;
        ctx.set_stroke_style_str(&css_color(color));
        ctx.set_line_width(f64::from(width));
        ctx.begin_path();
        let _ = ctx.arc(
            f64::from(center.x),
            f64::from(center.y),
            f64::from(radius.max(0.0)),
            0.0,
            TAU,
        );
        ctx.stroke();
    }

    fn fill_polygon(&mut self, points: &[Point], color: Rgba) {
        let Some((first, rest)) = points.split_first() else {
            return;
        };
        let ctx = &self.ctx;
        ctx.set_fill_style_str(&css_color(color));
        ctx.begin_path();
        ctx.move_to(f64::from(first.x), f64::from(first.y));
        for p in rest {
            ctx.line_to(f64::from(p.x), f64::from(p.y));
        }
        ctx.close_path();
        ctx.fill();
    }

    fn text(&mut self, text: &str, anchor: Point, font: Font, align: Align, color: Rgba) {
        let ctx = &self.ctx;
        ctx.set_font(&css_font(font));
        ctx.set_fill_style_str(&css_color(color));
        ctx.set_text_align(match align {
            Align::Left => "left",
            Align::Center => "center",
            Align::Right => "right",
        });
        ctx.set_text_baseline("middle");
        let _ = ctx.fill_text(text, f64::from(anchor.x), f64::from(anchor.y));
    }

    fn text_width(&mut self, text: &str, font: Font) -> f32 {
        self.ctx.set_font(&css_font(font));
        self.ctx
            .measure_text(text)
            .map_or(0.0, |m| to_f32(m.width()))
    }

    fn push_clip(&mut self, rect: Rect) {
        let ctx = &self.ctx;
        ctx.save();
        ctx.begin_path();
        ctx.rect(
            f64::from(rect.x),
            f64::from(rect.y),
            f64::from(rect.w),
            f64::from(rect.h),
        );
        ctx.clip();
    }

    fn pop_clip(&mut self) {
        self.ctx.restore();
    }
}
