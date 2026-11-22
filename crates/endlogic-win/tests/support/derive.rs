//! Pictures made from a screenshot in memory: smaller, with margins, with
//! noise and another brightness, or with a part painted over.
// Pixel arithmetic reads best with x, y, w, h and the like.
#![allow(clippy::many_single_char_names)]

/// An RGBA picture.
#[derive(Clone)]
pub struct Picture {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

/// Averages the picture down to fit `width` × `height`, keeping its shape:
/// every new pixel is the mean of the old pixels it covers.
pub fn shrink(p: &Picture, width: usize, height: usize) -> Picture {
    #[allow(clippy::cast_precision_loss)]
    let scale = (width as f64 / p.width as f64).min(height as f64 / p.height as f64);
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    let (w, h) = (
        (p.width as f64 * scale).round() as usize,
        (p.height as f64 * scale).round() as usize,
    );
    // One axis at a time: a row of weights for each new pixel.
    #[allow(clippy::cast_precision_loss)]
    let weights = |from: usize, to: usize| -> Vec<Vec<(usize, f64)>> {
        let f = from as f64 / to as f64;
        (0..to)
            .map(|i| {
                let (a, b) = (i as f64 * f, (i + 1) as f64 * f);
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let (lo, hi) = (a.floor() as usize, (b.ceil() as usize).min(from));
                (lo..hi)
                    .map(|k| {
                        let cover = (b.min((k + 1) as f64) - a.max(k as f64)) / f;
                        (k, cover)
                    })
                    .collect()
            })
            .collect()
    };
    let wx = weights(p.width, w);
    let wy = weights(p.height, h);
    let mut mid = vec![0f64; w * p.height * 4];
    for y in 0..p.height {
        for (x, ws) in wx.iter().enumerate() {
            for &(k, c) in ws {
                for ch in 0..4 {
                    mid[(y * w + x) * 4 + ch] += c * f64::from(p.rgba[(y * p.width + k) * 4 + ch]);
                }
            }
        }
    }
    let mut out = vec![0u8; w * h * 4];
    for (y, ws) in wy.iter().enumerate() {
        for x in 0..w {
            for ch in 0..4 {
                let v: f64 = ws.iter().map(|&(k, c)| c * mid[(k * w + x) * 4 + ch]).sum();
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                {
                    out[(y * w + x) * 4 + ch] = v.round().clamp(0.0, 255.0) as u8;
                }
            }
        }
    }
    Picture {
        width: w,
        height: h,
        rgba: out,
    }
}

/// The picture inside margins of `side` and `top` pixels: a flat grey on
/// the left and right, a darker grey above and below.
pub fn margins(p: &Picture, side: usize, top: usize) -> Picture {
    let (w, h) = (p.width + 2 * side, p.height + 2 * top);
    let mut out = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 4;
            let inside = x >= side && x < side + p.width && y >= top && y < top + p.height;
            let px = if inside {
                let j = ((y - top) * p.width + x - side) * 4;
                [p.rgba[j], p.rgba[j + 1], p.rgba[j + 2], 255]
            } else if y < top || y >= top + p.height {
                [40, 42, 46, 255]
            } else {
                [120, 120, 120, 255]
            };
            out[i..i + 4].copy_from_slice(&px);
        }
    }
    Picture {
        width: w,
        height: h,
        rgba: out,
    }
}

/// Every channel times `gain`, plus noise of up to `noise` levels either
/// way from a fixed seed.
pub fn noisy(p: &Picture, gain: f32, noise: i32, seed: u64) -> Picture {
    let mut state = seed | 1;
    let mut out = p.clone();
    for (i, v) in out.rgba.iter_mut().enumerate() {
        if i % 4 == 3 {
            continue;
        }
        // xorshift64*.
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        let r = state.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 33;
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let d = (r % u64::from(2 * noise.unsigned_abs() + 1)) as i32 - noise;
        #[allow(clippy::cast_possible_truncation)]
        let scaled = (f32::from(*v) * gain).round() as i32 + d;
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        {
            *v = scaled.clamp(0, 255) as u8;
        }
    }
    out
}

/// The picture with a rectangle painted in one color.
pub fn painted(p: &Picture, x0: usize, y0: usize, x1: usize, y1: usize, rgb: [u8; 3]) -> Picture {
    let mut out = p.clone();
    for y in y0..y1.min(p.height) {
        for x in x0..x1.min(p.width) {
            let i = (y * p.width + x) * 4;
            out.rgba[i..i + 3].copy_from_slice(&rgb);
        }
    }
    out
}
