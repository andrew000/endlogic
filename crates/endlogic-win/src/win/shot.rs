//! The screenshot mode: one settled frame drawn into a PNG file, without a
//! window.

use std::process::ExitCode;

use endlogic_ui::{App, Budgets, Config, Insets, Painter, Point, Rect, Rgba};
use windows::Win32::Foundation::{E_FAIL, GENERIC_WRITE};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT, D2D1_RENDER_TARGET_PROPERTIES,
    D2D1_RENDER_TARGET_TYPE_SOFTWARE, D2D1_RENDER_TARGET_USAGE_NONE, D2D1CreateFactory,
    ID2D1Factory, ID2D1RenderTarget,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_ContainerFormatPng, GUID_WICPixelFormat32bppBGRA,
    GUID_WICPixelFormat32bppPBGRA, IWICBitmap, IWICImagingFactory, WICBitmapCacheOnLoad,
    WICBitmapEncoderNoCache,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
};
use windows::core::{HSTRING, Result};

use super::painter::{D2dPainter, polyline_style};
use crate::args::Shot;
use crate::link;

/// The brush's first color; every draw call sets its own.
static BLACK: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};
/// Fixed seed, so the same arguments draw the same picture.
const SEED: u64 = 0x5EED_0001;
/// Frame step while settling, in seconds.
const STEP: f32 = 1.0 / 60.0;
/// Most frames run before drawing, whatever the app says.
const MAX_FRAMES: u32 = 20_000;

/// A painter that draws nothing, for the frames before the last.
struct Blank;

impl Painter for Blank {
    fn fill_rect(&mut self, _: Rect, _: f32, _: Rgba) {}
    fn stroke_rect(&mut self, _: Rect, _: f32, _: f32, _: Rgba) {}
    fn line(&mut self, _: Point, _: Point, _: f32, _: Rgba) {}
    fn fill_circle(&mut self, _: Point, _: f32, _: Rgba) {}
    fn stroke_circle(&mut self, _: Point, _: f32, _: f32, _: Rgba) {}
    fn fill_polygon(&mut self, _: &[Point], _: Rgba) {}
    fn polyline(&mut self, _: &[Point], _: f32, _: Rgba) {}
    fn push_clip(&mut self, _: Rect) {}
    fn pop_clip(&mut self) {}
}

/// Runs the screenshot mode.
pub fn run(shot: &Shot) -> ExitCode {
    let code = shot.code.as_deref().map(link::parse_link);
    if code == Some(None) {
        eprintln!("endlogic-win: --code is not a puzzle link or code");
        return ExitCode::from(2);
    }
    let code = code.flatten();
    match draw(shot, code) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("endlogic-win: {err}");
            ExitCode::FAILURE
        }
    }
}

fn draw(shot: &Shot, code: Option<(endlogic_ui::CodeKind, String)>) -> Result<()> {
    // SAFETY: initializes COM for this thread once, before any COM use.
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok()? };
    // SAFETY: creates the in-process WIC factory.
    let wic: IWICImagingFactory =
        unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)? };
    // SAFETY: a plain bitmap of the requested size.
    let bitmap = unsafe {
        wic.CreateBitmap(
            shot.width * shot.scale,
            shot.height * shot.scale,
            &GUID_WICPixelFormat32bppPBGRA,
            WICBitmapCacheOnLoad,
        )?
    };
    // SAFETY: plain factory creation.
    let factory: ID2D1Factory =
        unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? };
    // Logical pixels stay 1/96 inch; more dots per inch draw them larger.
    #[allow(clippy::cast_precision_loss)]
    let dpi = 96.0 * shot.scale as f32;
    let props = D2D1_RENDER_TARGET_PROPERTIES {
        // Software rendering: the same pixels on every machine and run.
        r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: dpi,
        dpiY: dpi,
        usage: D2D1_RENDER_TARGET_USAGE_NONE,
        minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
    };
    // SAFETY: the bitmap and the properties live through the call.
    let target: ID2D1RenderTarget =
        unsafe { factory.CreateWicBitmapRenderTarget(&bitmap, &raw const props)? };
    // SAFETY: a plain creation call on the target.
    let brush = unsafe { target.CreateSolidColorBrush(&raw const BLACK, None)? };
    let polyline = polyline_style(&factory)?;

    // Pixel sizes are at most 8192.
    #[allow(clippy::cast_precision_loss)]
    let (width, height) = (shot.width as f32, shot.height as f32);
    if shot.font_sample {
        // SAFETY: starts drawing on our target; ended below.
        unsafe { target.BeginDraw() };
        let mut painter = D2dPainter::new(&target, &factory, &brush, &polyline);
        endlogic_ui::draw_font_sample(&mut painter, width, height);
        // SAFETY: ends the drawing begun above.
        unsafe { target.EndDraw(None, None)? };
        return save_png(&wic, &bitmap, shot);
    }
    let mut app = App::new(Config {
        width,
        height,
        insets: Insets::default(),
        entropy: SEED,
        settings: Vec::new(),
        system_language: shot.lang.clone(),
        code,
        touch: shot.touch,
        budgets: Budgets::default(),
    });
    for _ in 0..MAX_FRAMES {
        if !app.frame(STEP, &mut Blank) {
            break;
        }
    }
    // SAFETY: starts drawing on our target; ended below.
    unsafe { target.BeginDraw() };
    {
        let mut painter = D2dPainter::new(&target, &factory, &brush, &polyline);
        app.frame(0.0, &mut painter);
    }
    // SAFETY: ends the drawing begun above.
    unsafe { target.EndDraw(None, None)? };
    save_png(&wic, &bitmap, shot)
}

fn save_png(wic: &IWICImagingFactory, bitmap: &IWICBitmap, shot: &Shot) -> Result<()> {
    let path = HSTRING::from(shot.file.as_os_str());
    // SAFETY: a plain creation call.
    let stream = unsafe { wic.CreateStream()? };
    // SAFETY: `path` lives through the call; the file is created or replaced.
    unsafe { stream.InitializeFromFilename(&path, GENERIC_WRITE.0)? };
    // SAFETY: a plain creation call with a known container format.
    let encoder = unsafe { wic.CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null())? };
    // SAFETY: the stream is open for writing.
    unsafe { encoder.Initialize(&stream, WICBitmapEncoderNoCache)? };
    let mut frame = None;
    // SAFETY: `frame` is a valid out pointer; no encoder options.
    unsafe { encoder.CreateNewFrame(&raw mut frame, std::ptr::null_mut())? };
    let frame = frame.ok_or_else(|| windows::core::Error::from(E_FAIL))?;
    // SAFETY: default options.
    unsafe { frame.Initialize(None)? };
    // SAFETY: a plain setter.
    unsafe { frame.SetSize(shot.width * shot.scale, shot.height * shot.scale)? };
    let mut format = GUID_WICPixelFormat32bppBGRA;
    // SAFETY: `format` is a valid in/out pointer.
    unsafe { frame.SetPixelFormat(&raw mut format)? };
    // SAFETY: the bitmap is complete; the whole of it is written.
    unsafe { frame.WriteSource(bitmap, std::ptr::null())? };
    // SAFETY: commits the frame, then the file.
    unsafe { frame.Commit()? };
    // SAFETY: as above.
    unsafe { encoder.Commit()? };
    Ok(())
}
