//! Screens in the style of the game's repair screen, drawn in memory: for
//! tests of the recognizer and of what uses it. Nothing here is taken from
//! the game's pictures; sizes and colors are rough measurements, the
//! background is made-up noise.

use alloc::vec::Vec;

use endlogic_core::{Cell, Color, Puzzle, Shape};

/// What to draw.
#[derive(Debug, Clone)]
pub struct Scene {
    /// The puzzle.
    pub puzzle: Puzzle,
    /// Picture size.
    pub width: usize,
    /// Picture size.
    pub height: usize,
    /// Distance between neighbouring cell centers, in pixels.
    pub pitch: f32,
    /// Top-left corner of the grid.
    pub left: f32,
    /// Top-left corner of the grid.
    pub top: f32,
    /// Seed of the background, the cell shades and the noise.
    pub seed: u64,
    /// Noise added to every channel, in levels either way.
    pub noise: u8,
    /// Counters as digits, as in the game's "Numbers" display.
    pub numbers: bool,
    /// Index of a piece drawn on the grid at its top-left free cells
    /// instead of in the tray; its card stays, empty.
    pub on_grid: Option<usize>,
    /// Cell size of each piece in its card, in grid cells; `None` draws
    /// the piece's longer side 0.7 cell long, as the game does.
    pub piece_cells: Option<Vec<f32>>,
    /// Each piece cell in its own shade, as the game draws them; else every
    /// cell of a piece alike, so its cell borders cannot be seen.
    pub shaded: bool,
}

impl Scene {
    /// A scene with the grid in the middle left and the tray on the right,
    /// sized like the game at this picture size.
    #[must_use]
    pub fn new(puzzle: Puzzle, width: usize, height: usize) -> Scene {
        let grid = puzzle.grid();
        let (w, h) = (to_f32(grid.width()), to_f32(grid.height()));
        // The game draws a 6×6 grid 0.27 of the screen height per... about
        // 184 px a cell at 2160 px high.
        let pitch = to_f32(height) * 0.0852 * 6.0 / w.max(h).max(6.0);
        let left = to_f32(width) * 0.5 - w * pitch * 0.5;
        let top = to_f32(height) * 0.52 - h * pitch * 0.5;
        Scene {
            puzzle,
            width,
            height,
            pitch,
            left,
            top,
            seed: 1,
            noise: 3,
            numbers: false,
            on_grid: None,
            piece_cells: None,
            shaded: true,
        }
    }
}

/// Fill, outline and lighter shade of the palette colors (teal, orange,
/// violet, lime).
const COLORS: [([u8; 3], [u8; 3], [u8; 3]); 4] = [
    ([17, 146, 118], [0, 253, 207], [32, 224, 176]),
    ([196, 134, 58], [255, 168, 72], [232, 160, 70]),
    ([120, 84, 196], [176, 130, 255], [160, 120, 240]),
    ([120, 168, 40], [182, 240, 70], [160, 220, 60]),
];

fn to_f32(v: usize) -> f32 {
    // Picture sizes are exact in f32.
    #[allow(clippy::cast_precision_loss)]
    let v = v as f32;
    v
}

/// Nearest whole number, as a signed pixel index.
fn px(v: f32) -> i64 {
    // Picture coordinates are small.
    #[allow(clippy::cast_possible_truncation)]
    let t = v as i64;
    if v - to_f32_i(t) >= 0.5 {
        t + 1
    } else if v - to_f32_i(t) < -0.5 {
        t - 1
    } else {
        t
    }
}

fn to_f32_i(v: i64) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    let v = v as f32;
    v
}

struct Canvas {
    width: usize,
    height: usize,
    rgba: Vec<u8>,
    state: u64,
}

impl Canvas {
    fn random(&mut self) -> u64 {
        // xorshift64*.
        self.state ^= self.state >> 12;
        self.state ^= self.state << 25;
        self.state ^= self.state >> 27;
        self.state.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32
    }

    /// Mixes `rgb` over the pixels of a rectangle with weight `alpha`.
    #[allow(clippy::too_many_arguments)]
    fn rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, rgb: [u8; 3], alpha: f32) {
        let clamp = |v: i64, max: usize| usize::try_from(v.max(0)).unwrap_or(0).min(max);
        let (xa, xb) = (clamp(px(x0), self.width), clamp(px(x1), self.width));
        let (ya, yb) = (clamp(px(y0), self.height), clamp(px(y1), self.height));
        for y in ya..yb {
            for x in xa..xb {
                let i = (y * self.width + x) * 4;
                for c in 0..3 {
                    let old = f32::from(self.rgba[i + c]);
                    let new = old + (f32::from(rgb[c]) - old) * alpha;
                    // Between two bytes.
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    {
                        self.rgba[i + c] = (new + 0.5) as u8;
                    }
                }
            }
        }
    }

    fn fill(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, rgb: [u8; 3]) {
        self.rect(x0, y0, x1, y1, rgb, 1.0);
    }

    /// A rectangle outline of thickness `t`, inside its bounds.
    fn frame(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, t: f32, rgb: [u8; 3]) {
        self.fill(x0, y0, x1, y0 + t, rgb);
        self.fill(x0, y1 - t, x1, y1, rgb);
        self.fill(x0, y0, x0 + t, y1, rgb);
        self.fill(x1 - t, y0, x1, y1, rgb);
    }

    /// Diagonal stripes, `step` apart and `t` thick, over a rectangle.
    #[allow(clippy::too_many_arguments)]
    fn hatch(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, step: f32, t: f32, rgb: [u8; 3]) {
        for y in px(y0)..px(y1) {
            for x in px(x0)..px(x1) {
                let k = to_f32_i(x + y) / step;
                #[allow(clippy::cast_possible_truncation)]
                let frac = k - to_f32_i(k as i64);
                if frac * step < t {
                    let (fx, fy) = (to_f32_i(x), to_f32_i(y));
                    self.fill(fx, fy, fx + 1.0, fy + 1.0, rgb);
                }
            }
        }
    }

    /// A line of square dots from one point to another.
    fn line(&mut self, a: (f32, f32), b: (f32, f32), t: f32, rgb: [u8; 3]) {
        let len = (b.0 - a.0).abs().max((b.1 - a.1).abs()).max(1.0);
        let mut k = 0.0;
        while k <= len {
            let f = k / len;
            let (x, y) = (a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f);
            self.fill(x - t / 2.0, y - t / 2.0, x + t / 2.0, y + t / 2.0, rgb);
            k += 0.5;
        }
    }

    /// A ring of radius `r` and thickness `t`, with a slash: the `∅` sign.
    fn empty_sign(&mut self, x: f32, y: f32, r: f32, t: f32, rgb: [u8; 3]) {
        let (x0, x1) = (px(x - r - t), px(x + r + t));
        let (y0, y1) = (px(y - r - t), px(y + r + t));
        for py in y0..=y1 {
            for qx in x0..=x1 {
                let (dx, dy) = (to_f32_i(qx) + 0.5 - x, to_f32_i(py) + 0.5 - y);
                let d2 = dx * dx + dy * dy;
                let on_ring =
                    d2 <= (r + t / 2.0) * (r + t / 2.0) && d2 >= (r - t / 2.0) * (r - t / 2.0);
                // The slash from bottom left to top right, a bit longer.
                let on_slash = (dx + dy).abs() <= t * 0.75 && dx.abs() <= r * 1.2;
                if on_ring || on_slash {
                    let fx = to_f32_i(qx);
                    let fy = to_f32_i(py);
                    self.fill(fx, fy, fx + 1.0, fy + 1.0, rgb);
                }
            }
        }
    }
}

/// Value noise: smooth blotches of about `size` pixels.
fn blotches(width: usize, height: usize, size: usize, seed: u64) -> impl Fn(usize, usize) -> f32 {
    let gw = width / size + 2;
    let gh = height / size + 2;
    let mut state = seed | 1;
    let grid: Vec<f32> = (0..gw * gh)
        .map(|_| {
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            let v = state.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40;
            // 24 bits into 0..1.
            #[allow(clippy::cast_precision_loss)]
            let v = v as f32 / 16_777_216.0;
            v
        })
        .collect();
    move |x, y| {
        let (gx, gy) = (x / size, y / size);
        let (fx, fy) = (
            to_f32(x % size) / to_f32(size),
            to_f32(y % size) / to_f32(size),
        );
        let at = |i: usize, j: usize| grid[j * gw + i];
        let top = at(gx, gy) * (1.0 - fx) + at(gx + 1, gy) * fx;
        let bottom = at(gx, gy + 1) * (1.0 - fx) + at(gx + 1, gy + 1) * fx;
        top * (1.0 - fy) + bottom * fy
    }
}

/// Draws the scene: RGBA pixels, row by row.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn draw(scene: &Scene) -> Vec<u8> {
    let (w, h) = (scene.width, scene.height);
    let mut canvas = Canvas {
        width: w,
        height: h,
        rgba: alloc::vec![255; w * h * 4],
        state: scene.seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1,
    };
    // A dim, blotchy background with a reddish and a bluish tint.
    let big = blotches(w, h, (h / 6).max(8), scene.seed);
    let tint = blotches(w, h, (h / 4).max(8), scene.seed ^ 0x55);
    for y in 0..h {
        for x in 0..w {
            let v = big(x, y);
            let t = tint(x, y);
            let i = (y * w + x) * 4;
            // Within a byte.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            {
                canvas.rgba[i] = (12.0 + 30.0 * v + 14.0 * t) as u8;
                canvas.rgba[i + 1] = (14.0 + 30.0 * v) as u8;
                canvas.rgba[i + 2] = (16.0 + 30.0 * v + 10.0 * (1.0 - t)) as u8;
            }
        }
    }
    let p = scene.pitch;
    let puzzle = &scene.puzzle;
    let grid = puzzle.grid();
    let (gw, gh) = (grid.width(), grid.height());
    let cx = |c: usize| scene.left + (to_f32(c) + 0.5) * p;
    let cy = |r: usize| scene.top + (to_f32(r) + 0.5) * p;
    // A yellow bar near the top left, like the game's title mark.
    canvas.fill(
        0.03 * to_f32(w),
        0.14 * to_f32(h),
        0.03 * to_f32(w) + 0.075 * p,
        0.14 * to_f32(h) + 0.35 * p,
        [255, 210, 43],
    );
    // The grid: gaps, then cells.
    let half = 0.473 * p;
    canvas.fill(
        scene.left,
        scene.top,
        scene.left + to_f32(gw) * p,
        scene.top + to_f32(gh) * p,
        [44, 46, 44],
    );
    // Pieces drawn on the grid.
    let mut on_grid: Vec<(usize, usize, Color)> = Vec::new();
    if let Some(i) = scene.on_grid {
        let piece = &puzzle.pieces()[i];
        'search: for r0 in 0..gh {
            for c0 in 0..gw {
                let fits = piece
                    .shape
                    .cells()
                    .iter()
                    .all(|&(r, c)| grid.get(r0 + r, c0 + c) == Some(Cell::Free));
                if fits {
                    for &(r, c) in piece.shape.cells() {
                        on_grid.push((r0 + r, c0 + c, piece.color));
                    }
                    break 'search;
                }
            }
        }
    }
    for r in 0..gh {
        for c in 0..gw {
            let (x, y) = (cx(c), cy(r));
            let (x0, y0, x1, y1) = (x - half, y - half, x + half, y + half);
            let placed = on_grid.iter().find(|&&(pr, pc, _)| pr == r && pc == c);
            match (grid.get(r, c), placed) {
                (_, Some(&(_, _, color))) => {
                    let (fill, edge, _) = COLORS[color.index()];
                    canvas.fill(x0, y0, x1, y1, fill);
                    canvas.frame(x0, y0, x1, y1, 0.03 * p, edge);
                }
                (Some(Cell::Lock(color)), None) => {
                    let (fill, edge, light) = COLORS[color.index()];
                    canvas.fill(x0, y0, x1, y1, edge);
                    canvas.fill(
                        x0 + 0.08 * p,
                        y0 + 0.08 * p,
                        x1 - 0.08 * p,
                        y1 - 0.08 * p,
                        [20, 20, 20],
                    );
                    canvas.fill(x0 + 0.1 * p, y0 + 0.1 * p, x1 - 0.1 * p, y1 - 0.1 * p, fill);
                    // The padlock: body and shackle.
                    canvas.fill(
                        x - 0.12 * p,
                        y - 0.02 * p,
                        x + 0.12 * p,
                        y + 0.14 * p,
                        light,
                    );
                    canvas.frame(
                        x - 0.08 * p,
                        y - 0.14 * p,
                        x + 0.08 * p,
                        y,
                        0.035 * p,
                        light,
                    );
                }
                (Some(Cell::Dead), None) => {
                    canvas.fill(x0, y0, x1, y1, [100, 100, 100]);
                    canvas.hatch(x0, y0, x1, y1, 0.08 * p, 0.025 * p, [84, 84, 84]);
                    canvas.empty_sign(x, y, 0.2 * p, 0.05 * p, [72, 72, 72]);
                }
                _ => {
                    canvas.fill(x0, y0, x1, y1, [20, 22, 22]);
                    for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                        let (ax, ay) = (x + sx * half, y + sy * half);
                        canvas.line(
                            (ax - sx * 0.11 * p, ay - sy * 0.11 * p),
                            (ax - sx * 0.31 * p, ay - sy * 0.31 * p),
                            0.025 * p,
                            [44, 47, 47],
                        );
                    }
                    canvas.fill(
                        x - 0.025 * p,
                        y - 0.025 * p,
                        x + 0.025 * p,
                        y + 0.025 * p,
                        [40, 42, 42],
                    );
                }
            }
        }
    }
    // Grey corner marks.
    for (x, y, sx, sy) in [
        (scene.left, scene.top, -1.0, -1.0),
        (scene.left + to_f32(gw) * p, scene.top, 1.0, -1.0),
        (scene.left, scene.top + to_f32(gh) * p, -1.0, 1.0),
        (
            scene.left + to_f32(gw) * p,
            scene.top + to_f32(gh) * p,
            1.0,
            1.0,
        ),
    ] {
        let (mx, my) = (x + sx * 0.06 * p, y + sy * 0.06 * p);
        canvas.line(
            (mx, my),
            (mx - sx * 0.08 * p, my),
            0.02 * p,
            [150, 150, 150],
        );
        canvas.line(
            (mx, my),
            (mx, my - sy * 0.08 * p),
            0.02 * p,
            [150, 150, 150],
        );
    }
    // Counters.
    let colors = puzzle.colors();
    let long = 0.31 * p;
    let short = 0.098 * p;
    let step = 0.148 * p;
    let ring = (0.016 * p).max(1.0);
    let row_base = scene.left - 0.125 * p;
    let col_base = scene.top - 0.12 * p;
    for row in [true, false] {
        let n = if row { gh } else { gw };
        for k in 0..n {
            let shown: Vec<(Color, usize)> = colors
                .iter()
                .map(|&c| {
                    (
                        c,
                        if row {
                            puzzle.row_clue(c, k)
                        } else {
                            puzzle.col_clue(c, k)
                        },
                    )
                })
                .filter(|&(_, v)| v > 0)
                .collect();
            let mid = if row { cy(k) } else { cx(k) };
            if shown.is_empty() {
                let (x, y) = if row {
                    (row_base - 0.147 * p, mid)
                } else {
                    (mid, col_base - 0.147 * p)
                };
                canvas.empty_sign(x, y, 0.136 * p, 0.03 * p, [110, 110, 110]);
                continue;
            }
            let m = to_f32(shown.len());
            for (j, &(color, v)) in shown.iter().enumerate() {
                let (_, edge, _) = COLORS[color.index()];
                let locks = (0..if row { gw } else { gh })
                    .filter(|&i| {
                        let (r, c) = if row { (k, i) } else { (i, k) };
                        grid.get(r, c) == Some(Cell::Lock(color))
                    })
                    .count();
                // Rows: the first color at the bottom; columns: on the left.
                let off = (to_f32(j) - (m - 1.0) / 2.0) * 0.356 * p;
                let across = if row { mid - off } else { mid + off };
                if scene.numbers {
                    let (x, y) = if row {
                        (row_base, across)
                    } else {
                        (across, col_base)
                    };
                    digits(&mut canvas, v, x, y, row, p, edge);
                    continue;
                }
                for t in 0..v {
                    let along = to_f32(t) * step;
                    let (x0, y0, x1, y1) = if row {
                        (
                            row_base - along - short,
                            across - long / 2.0,
                            row_base - along,
                            across + long / 2.0,
                        )
                    } else {
                        (
                            across - long / 2.0,
                            col_base - along - short,
                            across + long / 2.0,
                            col_base - along,
                        )
                    };
                    if t < locks {
                        canvas.fill(x0, y0, x1, y1, edge);
                    } else {
                        canvas.rect(x0, y0, x1, y1, edge, 0.12);
                        canvas.frame(x0, y0, x1, y1, ring, edge);
                    }
                }
            }
        }
    }
    // The tray: a panel with cards in two columns.
    let card = 1.52 * p;
    let card_step = 1.72 * p;
    let tray_left = scene.left + to_f32(gw) * p + 1.6 * p;
    let tray_top = scene.top - 0.6 * p;
    canvas.fill(
        tray_left - 0.2 * p,
        tray_top - 0.2 * p,
        tray_left + 2.0 * card_step,
        to_f32(h) * 0.93,
        [50, 52, 52],
    );
    let cells = scene.piece_cells.clone().unwrap_or_default();
    for (i, piece) in puzzle.pieces().iter().enumerate() {
        let (x0, y0) = (
            tray_left + to_f32(i % 2) * card_step,
            tray_top + to_f32(i / 2) * card_step,
        );
        canvas.fill(x0, y0, x0 + card, y0 + card, [28, 30, 30]);
        for (sx, sy, ax, ay) in [
            (1.0, 1.0, x0, y0),
            (-1.0, 1.0, x0 + card, y0),
            (1.0, -1.0, x0, y0 + card),
            (-1.0, -1.0, x0 + card, y0 + card),
        ] {
            canvas.line(
                (ax + sx * 0.12 * p, ay + sy * 0.12 * p),
                (ax + sx * 0.22 * p, ay + sy * 0.22 * p),
                0.02 * p,
                [70, 72, 72],
            );
        }
        if scene.on_grid == Some(i) {
            continue;
        }
        let shape = &piece.shape;
        let side = to_f32(shape.width().max(shape.height()));
        let c = cells.get(i).map_or(0.7 * p / side, |&k| k * p);
        let (pw, ph) = (to_f32(shape.width()) * c, to_f32(shape.height()) * c);
        let (px0, py0) = (x0 + (card - pw) / 2.0, y0 + (card - ph) / 2.0);
        draw_piece(
            &mut canvas,
            shape,
            piece.color,
            (px0, py0, c),
            p,
            scene.shaded,
        );
    }
    // Noise.
    if scene.noise > 0 {
        let span = u64::from(scene.noise) * 2 + 1;
        for i in 0..canvas.rgba.len() {
            if i % 4 == 3 {
                continue;
            }
            let d = i64::try_from(canvas.random() % span).unwrap_or(0) - i64::from(scene.noise);
            let v = (i64::from(canvas.rgba[i]) + d).clamp(0, 255);
            canvas.rgba[i] = u8::try_from(v).unwrap_or(0);
        }
    }
    canvas.rgba
}

/// A piece in its card: cells in shades of its color, a dark line and a
/// bright outline around the whole piece.
fn draw_piece(
    canvas: &mut Canvas,
    shape: &Shape,
    color: Color,
    at: (f32, f32, f32),
    p: f32,
    shaded: bool,
) {
    let (x0, y0, c) = at;
    let (fill, edge, _) = COLORS[color.index()];
    let t = (0.018 * p).max(1.0);
    for &(r, k) in shape.cells() {
        let (x, y) = (x0 + to_f32(k) * c, y0 + to_f32(r) * c);
        // Each cell a little lighter or darker.
        let shade = if shaded {
            0.82 + to_f32(usize::try_from(canvas.random() % 37).unwrap_or(0)) / 100.0
        } else {
            1.0
        };
        let mut rgb = fill;
        for v in &mut rgb {
            // Within a byte.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            {
                *v = (f32::from(*v) * shade).min(255.0) as u8;
            }
        }
        canvas.fill(x, y, x + c, y + c, rgb);
    }
    // The dark line first, then the outline over it, so corners close.
    let dark = (0.012 * p).max(1.0);
    for pass in 0..2 {
        for &(r, k) in shape.cells() {
            let (x, y) = (x0 + to_f32(k) * c, y0 + to_f32(r) * c);
            let open = |dr: isize, dk: isize| {
                let (nr, nk) = (r.checked_add_signed(dr), k.checked_add_signed(dk));
                !matches!((nr, nk), (Some(nr), Some(nk)) if shape.contains(nr, nk))
            };
            let (d, rgb) = if pass == 0 {
                (t + dark, [16, 16, 16])
            } else {
                (t, edge)
            };
            if open(-1, 0) {
                canvas.fill(x, y, x + c, y + d, rgb);
            }
            if open(1, 0) {
                canvas.fill(x, y + c - d, x + c, y + c, rgb);
            }
            if open(0, -1) {
                canvas.fill(x, y, x + d, y + c, rgb);
            }
            if open(0, 1) {
                canvas.fill(x + c - d, y, x + c, y + c, rgb);
            }
        }
    }
}

/// A number in blocky digits, ending at `x` (a row counter) or standing on
/// `y` (a column counter).
fn digits(canvas: &mut Canvas, v: usize, x: f32, y: f32, row: bool, p: f32, rgb: [u8; 3]) {
    // Segments a to g of each digit.
    const SEGMENTS: [u8; 10] = [
        0b011_1111, 0b000_0110, 0b101_1011, 0b100_1111, 0b110_0110, 0b110_1101, 0b111_1101,
        0b000_0111, 0b111_1111, 0b110_1111,
    ];
    let (dh, dw, t) = (0.3 * p, 0.16 * p, 0.035 * p);
    let text: Vec<usize> = if v >= 10 {
        alloc::vec![v / 10, v % 10]
    } else {
        alloc::vec![v]
    };
    let total = to_f32(text.len()) * (dw + 0.05 * p);
    let (mut gx, gy) = if row {
        (x - total, y - dh / 2.0)
    } else {
        (x - total / 2.0, y - dh)
    };
    for d in text {
        let s = SEGMENTS[d];
        let seg = |i: u8| s & (1 << i) != 0;
        let (x0, x1, y0, ym, y1) = (gx, gx + dw, gy, gy + dh / 2.0, gy + dh);
        if seg(0) {
            canvas.fill(x0, y0, x1, y0 + t, rgb);
        }
        if seg(1) {
            canvas.fill(x1 - t, y0, x1, ym, rgb);
        }
        if seg(2) {
            canvas.fill(x1 - t, ym, x1, y1, rgb);
        }
        if seg(3) {
            canvas.fill(x0, y1 - t, x1, y1, rgb);
        }
        if seg(4) {
            canvas.fill(x0, ym, x0 + t, y1, rgb);
        }
        if seg(5) {
            canvas.fill(x0, y0, x0 + t, ym, rgb);
        }
        if seg(6) {
            canvas.fill(x0, ym - t / 2.0, x1, ym + t / 2.0, rgb);
        }
        gx += dw + 0.05 * p;
    }
}
