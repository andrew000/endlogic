//! Makes the app icon: draws it with the same [`Painter`] the app uses (on
//! Direct2D, into WIC bitmaps) at every size of a Windows icon and writes
//!
//! - `res/endlogic.ico`, the icon file,
//! - `res/endlogic.res`, a compiled resource file with the icon as group 1,
//!   which the build passes to the linker as it is,
//!
//! and prints the same drawing as SVG, for the web page's tab icon.
//!
//! Run on Windows after changing the drawing:
//! `cargo run -p endlogic-win --example icon`. Ordinary builds never run it.
#![cfg_attr(not(windows), allow(unused))]
// Direct2D, WIC and COM need `unsafe`; every block says why it is sound.
#![allow(unsafe_code)]

#[cfg(windows)]
#[path = "../src/win/painter.rs"]
#[allow(dead_code)]
mod painter;

use std::fmt::Write as _;

use endlogic_ui::{Painter, Point, Rect, Rgba, draw_icon};

/// The sizes of the icon, in pixels.
const SIZES: [u32; 8] = [16, 20, 24, 32, 40, 48, 64, 256];
/// The resource id of the icon group, which Windows shows for the program.
const GROUP_ID: u16 = 1;

/// A painter that writes SVG elements.
#[derive(Default)]
struct Svg {
    body: String,
}

fn svg_color(c: Rgba) -> String {
    if c.a == 255 {
        format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
    } else {
        format!("#{:02x}{:02x}{:02x}{:02x}", c.r, c.g, c.b, c.a)
    }
}

/// A number with at most two decimals and no trailing zeros.
fn n(x: f32) -> String {
    let s = format!("{x:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".to_owned()
    } else {
        s.to_owned()
    }
}

fn points(list: &[Point]) -> String {
    let parts: Vec<String> = list
        .iter()
        .map(|p| format!("{},{}", n(p.x), n(p.y)))
        .collect();
    parts.join(" ")
}

impl Painter for Svg {
    fn fill_rect(&mut self, r: Rect, radius: f32, c: Rgba) {
        let _ = write!(
            self.body,
            "<rect x='{}' y='{}' width='{}' height='{}' rx='{}' fill='{}'/>",
            n(r.x),
            n(r.y),
            n(r.w),
            n(r.h),
            n(radius),
            svg_color(c)
        );
    }
    fn stroke_rect(&mut self, r: Rect, radius: f32, width: f32, c: Rgba) {
        let _ = write!(
            self.body,
            "<rect x='{}' y='{}' width='{}' height='{}' rx='{}' fill='none' stroke='{}' stroke-width='{}'/>",
            n(r.x),
            n(r.y),
            n(r.w),
            n(r.h),
            n(radius),
            svg_color(c),
            n(width)
        );
    }
    fn line(&mut self, from: Point, to: Point, width: f32, c: Rgba) {
        let _ = write!(
            self.body,
            "<path d='M{},{}L{},{}' stroke='{}' stroke-width='{}'/>",
            n(from.x),
            n(from.y),
            n(to.x),
            n(to.y),
            svg_color(c),
            n(width)
        );
    }
    fn fill_circle(&mut self, center: Point, radius: f32, c: Rgba) {
        let _ = write!(
            self.body,
            "<circle cx='{}' cy='{}' r='{}' fill='{}'/>",
            n(center.x),
            n(center.y),
            n(radius),
            svg_color(c)
        );
    }
    fn stroke_circle(&mut self, center: Point, radius: f32, width: f32, c: Rgba) {
        let _ = write!(
            self.body,
            "<circle cx='{}' cy='{}' r='{}' fill='none' stroke='{}' stroke-width='{}'/>",
            n(center.x),
            n(center.y),
            n(radius),
            svg_color(c),
            n(width)
        );
    }
    fn fill_polygon(&mut self, list: &[Point], c: Rgba) {
        let _ = write!(
            self.body,
            "<polygon points='{}' fill='{}'/>",
            points(list),
            svg_color(c)
        );
    }
    fn polyline(&mut self, list: &[Point], width: f32, c: Rgba) {
        let _ = write!(
            self.body,
            "<polyline points='{}' fill='none' stroke='{}' stroke-width='{}' stroke-linecap='square' stroke-miterlimit='10'/>",
            points(list),
            svg_color(c),
            n(width)
        );
    }
    fn push_clip(&mut self, _: Rect) {}
    fn pop_clip(&mut self) {}
}

/// The icon as an SVG document of `side` units.
fn svg(side: f32) -> String {
    let mut svg = Svg::default();
    draw_icon(&mut svg, side);
    format!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 {s} {s}'>{}</svg>",
        svg.body,
        s = n(side)
    )
}

/// An `.ico` file holding `images`, PNG files of the given sizes.
fn ico(images: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let count = u16::try_from(images.len()).unwrap();
    out.extend(0u16.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(count.to_le_bytes());
    let mut offset = 6 + 16 * u32::from(count);
    for (size, png) in images {
        let len = u32::try_from(png.len()).unwrap();
        // 256 is written as 0.
        let side = u8::try_from(*size % 256).unwrap();
        out.extend([side, side, 0, 0]);
        out.extend(1u16.to_le_bytes());
        out.extend(32u16.to_le_bytes());
        out.extend(len.to_le_bytes());
        out.extend(offset.to_le_bytes());
        offset += len;
    }
    for (_, png) in images {
        out.extend(png);
    }
    out
}

/// One entry of a compiled resource file: a numbered resource of a
/// numbered type.
fn resource(out: &mut Vec<u8>, kind: u16, id: u16, flags: u16, data: &[u8]) {
    out.extend(u32::try_from(data.len()).unwrap().to_le_bytes());
    // Header: the two sizes, type, name, then version, flags, language,
    // version and characteristics.
    out.extend(32u32.to_le_bytes());
    out.extend([0xFF, 0xFF]);
    out.extend(kind.to_le_bytes());
    out.extend([0xFF, 0xFF]);
    out.extend(id.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    out.extend(flags.to_le_bytes());
    // Neutral language.
    out.extend(0u16.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    out.extend(data);
    while out.len() % 4 != 0 {
        out.push(0);
    }
}

/// A `.res` file with every image as an icon (ids from 1) and one icon
/// group, [`GROUP_ID`], listing them.
fn res(images: &[(u32, Vec<u8>)]) -> Vec<u8> {
    const RT_ICON: u16 = 3;
    const RT_GROUP_ICON: u16 = 14;
    // Movable, discardable; the group is also pure.
    const ICON_FLAGS: u16 = 0x1010;
    const GROUP_FLAGS: u16 = 0x1030;
    let mut out = Vec::new();
    // The file starts with an empty entry that marks it as 32-bit.
    resource(&mut out, 0, 0, 0, &[]);
    let mut group = Vec::new();
    group.extend(0u16.to_le_bytes());
    group.extend(1u16.to_le_bytes());
    group.extend(u16::try_from(images.len()).unwrap().to_le_bytes());
    for (k, (size, png)) in images.iter().enumerate() {
        let id = u16::try_from(k + 1).unwrap();
        resource(&mut out, RT_ICON, id, ICON_FLAGS, png);
        let side = u8::try_from(*size % 256).unwrap();
        group.extend([side, side, 0, 0]);
        group.extend(1u16.to_le_bytes());
        group.extend(32u16.to_le_bytes());
        group.extend(u32::try_from(png.len()).unwrap().to_le_bytes());
        group.extend(id.to_le_bytes());
    }
    resource(&mut out, RT_GROUP_ICON, GROUP_ID, GROUP_FLAGS, &group);
    out
}

#[cfg(windows)]
mod render {
    use windows::Win32::Foundation::{E_FAIL, GENERIC_WRITE};
    use windows::Win32::Graphics::Direct2D::Common::{
        D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
    };
    use windows::Win32::Graphics::Direct2D::{
        D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT,
        D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_SOFTWARE,
        D2D1_RENDER_TARGET_USAGE_NONE, D2D1CreateFactory, ID2D1Factory, ID2D1RenderTarget,
    };
    use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
    use windows::Win32::Graphics::Imaging::{
        CLSID_WICImagingFactory, GUID_ContainerFormatPng, GUID_WICPixelFormat32bppBGRA,
        GUID_WICPixelFormat32bppPBGRA, IWICImagingFactory, WICBitmapCacheOnLoad,
        WICBitmapEncoderNoCache,
    };
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
    };
    use windows::core::{HSTRING, Result};

    use super::painter::{D2dPainter, polyline_style};

    static CLEAR: D2D1_COLOR_F = D2D1_COLOR_F {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    /// The icon at `size` pixels as a PNG file's bytes.
    pub fn png(size: u32) -> Result<Vec<u8>> {
        // SAFETY: initializes COM for this thread; repeated calls are fine.
        let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        // SAFETY: creates the in-process WIC factory.
        let wic: IWICImagingFactory =
            unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)? };
        // SAFETY: a plain bitmap of the requested size.
        let bitmap = unsafe {
            wic.CreateBitmap(
                size,
                size,
                &GUID_WICPixelFormat32bppPBGRA,
                WICBitmapCacheOnLoad,
            )?
        };
        // SAFETY: plain factory creation.
        let factory: ID2D1Factory =
            unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? };
        let props = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
        };
        // SAFETY: the bitmap and the properties live through the call.
        let target: ID2D1RenderTarget =
            unsafe { factory.CreateWicBitmapRenderTarget(&bitmap, &raw const props)? };
        // SAFETY: a plain creation call on the target.
        let brush = unsafe { target.CreateSolidColorBrush(&raw const CLEAR, None)? };
        let stroke = polyline_style(&factory)?;
        // SAFETY: starts drawing on our target; ended below.
        unsafe { target.BeginDraw() };
        // SAFETY: drawing has begun.
        unsafe { target.Clear(Some(&raw const CLEAR)) };
        {
            let mut painter = D2dPainter::new(&target, &factory, &brush, &stroke);
            #[allow(clippy::cast_precision_loss)]
            endlogic_ui::draw_icon(&mut painter, size as f32);
        }
        // SAFETY: ends the drawing begun above.
        unsafe { target.EndDraw(None, None)? };

        let file = std::env::temp_dir().join(format!("endlogic-icon-{size}.png"));
        let path = HSTRING::from(file.as_os_str());
        // SAFETY: a plain creation call.
        let stream = unsafe { wic.CreateStream()? };
        // SAFETY: `path` lives through the call; the file is created or
        // replaced.
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
        unsafe { frame.SetSize(size, size)? };
        let mut format = GUID_WICPixelFormat32bppBGRA;
        // SAFETY: `format` is a valid in/out pointer.
        unsafe { frame.SetPixelFormat(&raw mut format)? };
        // SAFETY: the bitmap is complete; the whole of it is written.
        unsafe { frame.WriteSource(&bitmap, std::ptr::null())? };
        // SAFETY: commits the frame, then the file.
        unsafe { frame.Commit()? };
        // SAFETY: as above.
        unsafe { encoder.Commit()? };
        drop(stream);
        let bytes = std::fs::read(&file).map_err(|_| windows::core::Error::from(E_FAIL))?;
        let _ = std::fs::remove_file(&file);
        Ok(bytes)
    }
}

#[cfg(windows)]
fn main() -> std::process::ExitCode {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("res");
    let mut images = Vec::new();
    for size in SIZES {
        match render::png(size) {
            Ok(png) => images.push((size, png)),
            Err(err) => {
                eprintln!("icon: drawing {size} px failed: {err}");
                return std::process::ExitCode::FAILURE;
            }
        }
    }
    let ico = ico(&images);
    let res = res(&images);
    if std::fs::create_dir_all(&dir).is_err()
        || std::fs::write(dir.join("endlogic.ico"), &ico).is_err()
        || std::fs::write(dir.join("endlogic.res"), &res).is_err()
    {
        eprintln!("icon: cannot write {}", dir.display());
        return std::process::ExitCode::FAILURE;
    }
    // A 256-pixel copy to look at, outside the repository.
    if let Some((_, big)) = images.last() {
        let _ = std::fs::write(std::env::temp_dir().join("endlogic-icon-256.png"), big);
    }
    for (size, png) in &images {
        eprintln!("{size:>4} px: {} bytes", png.len());
    }
    eprintln!(
        "endlogic.ico: {} bytes, endlogic.res: {} bytes",
        ico.len(),
        res.len()
    );
    println!("{}", svg(16.0));
    std::process::ExitCode::SUCCESS
}

#[cfg(not(windows))]
fn main() {
    println!("{}", svg(16.0));
}
