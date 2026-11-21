//! Drawing the whole screen with [`Painter`] primitives.

use alloc::borrow::ToOwned;
use alloc::format;
use alloc::vec::Vec;

use crate::math;

use core::f32::consts::{FRAC_PI_2, TAU};

use endlogic_core::generator::Difficulty;
use endlogic_core::{Cell, Color, MAX_COLORS, Shape};

mod edit;

use crate::app::{App, CounterMode, Game, Gesture};
use crate::board::{Line, LineState};
use crate::counters::{Counter, CounterView, TICK_THICK, line_counters};
use crate::font::{Align, Font, MIN_FONT_SIZE};
use crate::ink::Ink;
use crate::layout::{Arrangement, Layout, TITLE, TextBlock, tab_label, to_f32};
use crate::painter::{Painter, Point, Rect, Rgba};
use crate::text::{Lang, Text, text};
use crate::theme::{self, Palette, ease_out, palette};

/// Text shown in place of the puzzle id while there is no puzzle.
const NO_ID: &str = "Δ-…";

pub(crate) fn draw(app: &App, p: &mut Ink<'_>) {
    let l = &app.layout;
    p.fill_rect(
        Rect::new(0.0, 0.0, app.width, app.height),
        0.0,
        theme::BACKGROUND,
    );
    draw_header(app, p);
    draw_top_controls(app, p);
    if app.is_editing() {
        edit::draw(app, p);
        draw_overlays(app, p);
        return;
    }
    if let Some(game) = &app.game {
        draw_board(app, game, p);
        draw_counters(app, game, p);
        draw_tray(app, game, p);
        draw_pieces(app, game, p);
    } else {
        draw_empty_board(l, p);
    }
    draw_bottom_controls(app, p);
    draw_overlays(app, p);
}

// ---- small helpers ----

/// A font of `size` (at least the smallest readable one).
pub(crate) fn text_font(size: f32, bold: bool) -> Font {
    Font::new(size, bold)
}

/// `font`, shrunk if needed so that `text` is at most `max_width` wide.
pub(crate) fn fit(p: &Ink<'_>, text: &str, font: Font, max_width: f32) -> Font {
    let width = p.text_width(text, font);
    // A hair over is rounding, not text that does not fit.
    if width <= max_width + 0.01 || width <= 0.0 {
        font
    } else {
        Font {
            size: (font.size * max_width / width).max(MIN_FONT_SIZE),
            ..font
        }
    }
}

/// Draws `text` centered in `rect`, shrinking it to fit.
pub(crate) fn centered_text(p: &mut Ink<'_>, text: &str, rect: Rect, font: Font, color: Rgba) {
    let font = fit(p, text, font, rect.w - 8.0);
    p.text(text, rect.center(), font, Align::Center, color);
}

/// Draws the lines the layout broke `block` into, each shrunk if the real
/// font turns out wider than the estimate.
fn paragraph(p: &mut Ink<'_>, block: &TextBlock, color: Rgba) {
    let line_h = block.line_height();
    let font = text_font(block.size, false);
    for (k, line) in block.lines.iter().enumerate() {
        let y = block.rect.y + line_h * (to_f32(k) + 0.5);
        let font = fit(p, line, font, block.rect.w);
        p.text(line, Point::new(block.rect.x, y), font, Align::Left, color);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Style {
    Normal,
    Primary,
    Disabled,
}

pub(crate) fn button(p: &mut Ink<'_>, rect: Rect, label: &str, style: Style, font: f32) {
    let radius = rect.h / 2.0;
    let (fill, stroke, color) = match style {
        Style::Normal => (theme::PANEL, theme::CELL_MARK, theme::TEXT),
        Style::Primary => (theme::ACCENT, theme::ACCENT, theme::BACKGROUND),
        Style::Disabled => (
            theme::PANEL.alpha(0.5),
            theme::CELL_MARK.alpha(0.5),
            theme::MUTED.alpha(0.6),
        ),
    };
    p.fill_rect(rect, radius, fill);
    p.stroke_rect(rect.inset(0.5), radius, 1.0, stroke);
    centered_text(p, label, rect, text_font(font, true), color);
}

/// A pill split into equal segments; `active` is highlighted.
fn segmented(p: &mut Ink<'_>, rect: Rect, labels: &[&str], active: Option<usize>, font: f32) {
    let radius = rect.h / 2.0;
    p.fill_rect(rect, radius, theme::PANEL);
    p.stroke_rect(rect.inset(0.5), radius, 1.0, theme::CELL_MARK);
    let w = rect.w / to_f32(labels.len());
    for (k, label) in labels.iter().enumerate() {
        let seg = Rect::new(rect.x + w * to_f32(k), rect.y, w, rect.h);
        let color = if Some(k) == active {
            p.fill_rect(seg.inset(3.0), (rect.h - 6.0) / 2.0, theme::TEXT);
            theme::BACKGROUND
        } else {
            theme::MUTED
        };
        centered_text(p, label, seg, text_font(font, true), color);
    }
}

/// A keyboard key cap with `label`.
fn keycap(p: &mut Ink<'_>, rect: Rect, label: &str, font: f32) {
    p.stroke_rect(rect.inset(0.5), 4.0, 1.0, theme::MUTED);
    centered_text(p, label, rect, text_font(font, true), theme::TEXT);
}

/// A mouse outline with the left button filled.
fn mouse_icon(p: &mut Ink<'_>, rect: Rect) {
    let h = rect.h.min(rect.w * 1.4);
    let body = Rect::centered(rect.center(), h * 0.62, h);
    let radius = body.w / 2.0;
    p.push_clip(Rect::new(body.x, body.y, body.w / 2.0, body.h * 0.42));
    p.fill_rect(body, radius, theme::ACCENT);
    p.pop_clip();
    p.stroke_rect(body, radius, 1.5, theme::MUTED);
    let mid = body.center().x;
    let split = body.y + body.h * 0.42;
    p.line(
        Point::new(mid, body.y),
        Point::new(mid, split),
        1.5,
        theme::MUTED,
    );
    p.line(
        Point::new(body.x, split),
        Point::new(body.right(), split),
        1.5,
        theme::MUTED,
    );
}

/// The `∅` sign.
fn empty_sign(p: &mut Ink<'_>, center: Point, radius: f32, width: f32, color: Rgba) {
    p.stroke_circle(center, radius, width, color);
    let d = radius * 1.25;
    p.line(
        Point::new(center.x - d * 0.75, center.y + d * 0.75),
        Point::new(center.x + d * 0.75, center.y - d * 0.75),
        width,
        color,
    );
}

// ---- header and controls ----

fn draw_header(app: &App, p: &mut Ink<'_>) {
    let l = &app.layout;
    if let Some(panel) = l.panel {
        p.fill_rect(panel, 10.0, theme::PANEL);
    }
    // Title: `//` in the accent color, then the name.
    let title_font = fit(p, TITLE, text_font(l.title_font, true), l.title.w);
    let y = l.title.center().y;
    p.text(
        "//",
        Point::new(l.title.x, y),
        title_font,
        Align::Left,
        theme::ACCENT,
    );
    let slashes = p.text_width("// ", title_font);
    p.text(
        "ENDLOGIC",
        Point::new(l.title.x + slashes, y),
        title_font,
        Align::Left,
        theme::TEXT,
    );

    // Puzzle id with the yellow marker.
    let id = app.puzzle_id().unwrap_or_else(|| NO_ID.to_owned());
    let r = l.puzzle_id;
    let marker = Rect::new(r.x, r.center().y - r.h * 0.3, 4.0, r.h * 0.6);
    p.fill_rect(marker, 1.0, theme::MARKER);
    let font = fit(p, &id, text_font(l.id_font, false), r.w - 12.0);
    p.text(
        &id,
        Point::new(r.x + 12.0, r.center().y),
        font,
        Align::Left,
        theme::MUTED,
    );

    if let Some(block) = &l.subtitle {
        paragraph(p, block, theme::MUTED);
    }
    draw_help(l, p);
}

/// The help rows: every icon centered in its box, which the layout puts in
/// one column and centers on the row's text.
fn draw_help(l: &Layout, p: &mut Ink<'_>) {
    let caps = [None, Some("R"), Some("TAB")];
    for (row, cap) in l.help.iter().zip(caps) {
        let icon = row.icon;
        match cap {
            Some(label) => {
                let size = row.text.size;
                let font = text_font(size, true);
                let w = (p.text_width(label, font) + size * 1.2)
                    .max(size * 2.0)
                    .min(icon.w);
                let cap = Rect::centered(icon.center(), w, size * 2.0);
                keycap(p, cap, label, size * 0.9);
            }
            None => mouse_icon(p, icon),
        }
        paragraph(p, &row.text, theme::TEXT);
    }
}

fn draw_top_controls(app: &App, p: &mut Ink<'_>) {
    let l = &app.layout;
    let labels: Vec<&str> = (0..5).map(|k| tab_label(app.lang, k)).collect();
    let active = if app.is_custom() {
        Some(4)
    } else {
        Difficulty::ALL.iter().position(|&d| d == app.difficulty)
    };
    if l.tabs_in_a_row() {
        segmented(p, l.tab_bar(), &labels, active, l.font * 0.9);
    } else {
        // A 2×2 grid and the custom tab: each tab is its own pill.
        for (k, (&rect, label)) in l.tabs.iter().zip(&labels).enumerate() {
            segmented(
                p,
                rect,
                &[label],
                (active == Some(k)).then_some(0),
                l.font * 0.9,
            );
        }
    }
    if let Some(rect) = l.colors.filter(|_| !app.is_custom()) {
        colors_switch(p, rect, app.colors, l.compact, l.font * 0.9);
    }
    if l.compact {
        button(p, l.lang, app.lang.label(), Style::Normal, l.font * 0.9);
        if !app.is_editing() {
            counter_toggle(p, l.counters, app.counters, l.font * 0.9);
        }
    } else {
        let lang = [Lang::En, Lang::Ua];
        segmented(
            p,
            l.lang,
            &[Lang::En.label(), Lang::Ua.label()],
            lang.iter().position(|&x| x == app.lang),
            l.font * 0.9,
        );
    }
    let style = if app.game.is_some() || app.is_editing() {
        Style::Normal
    } else {
        Style::Disabled
    };
    let label = if l.share_short {
        Text::ShareShort
    } else {
        Text::Share
    };
    button(p, l.share, text(app.lang, label), style, l.font * 0.9);
}

fn draw_bottom_controls(app: &App, p: &mut Ink<'_>) {
    let l = &app.layout;
    let movable = app.game.as_ref().is_some_and(|g| !g.won) && !app.is_generating();
    let style = if movable {
        Style::Normal
    } else {
        Style::Disabled
    };
    button(p, l.reset, text(app.lang, Text::Reset), style, l.font);
    let used = app.hints_used();
    let mut label = text(app.lang, Text::Hint).to_owned();
    if used > 0 {
        label = format!("{label} · {used}");
    }
    if app.is_hinting() {
        label.push('…');
    }
    button(p, l.hint, &label, style, l.font);
    if let Some(hint) = l.tab_hint {
        keycap(p, hint, "TAB", l.font * 0.7);
    }
    if !l.compact {
        let active = match app.counters {
            CounterMode::Grid => 0,
            CounterMode::Numbers => 1,
        };
        segmented(
            p,
            l.counters,
            &[text(app.lang, Text::Grid), text(app.lang, Text::Numbers)],
            Some(active),
            l.font * 0.9,
        );
    }
    let new = if app.is_custom() {
        Text::Edit
    } else {
        Text::New
    };
    button(p, l.new, text(app.lang, new), Style::Primary, l.font);
}

/// The colors switch: `1 | 2`, each number followed by a swatch of every
/// color it brings; with `compact` one button showing the current number.
fn colors_switch(p: &mut Ink<'_>, rect: Rect, colors: usize, compact: bool, font: f32) {
    let radius = rect.h / 2.0;
    p.fill_rect(rect, radius, theme::PANEL);
    p.stroke_rect(rect.inset(0.5), radius, 1.0, theme::CELL_MARK);
    let counts: &[usize] = if compact { &[colors] } else { &[1, 2] };
    let w = rect.w / to_f32(counts.len());
    let font = text_font(font, true);
    for (k, &n) in counts.iter().enumerate() {
        let seg = Rect::new(rect.x + w * to_f32(k), rect.y, w, rect.h);
        let active = !compact && n == colors;
        let color = if active {
            p.fill_rect(seg.inset(3.0), (rect.h - 6.0) / 2.0, theme::TEXT);
            theme::BACKGROUND
        } else if compact {
            theme::TEXT
        } else {
            theme::MUTED
        };
        let label = if n == 1 { "1" } else { "2" };
        let swatch = (rect.h * 0.22).max(4.0);
        let text_w = p.text_width(label, font);
        let total = text_w + 3.0 + to_f32(n) * (swatch + 2.0);
        let x = seg.center().x - total / 2.0;
        p.text(
            label,
            Point::new(x, seg.center().y),
            font,
            Align::Left,
            color,
        );
        for j in 0..n {
            let at = x + text_w + 3.0 + to_f32(j) * (swatch + 2.0);
            let square = Rect::new(at, seg.center().y - swatch / 2.0, swatch, swatch);
            p.fill_rect(square, 1.0, theme::PALETTES[j].fill);
        }
    }
}

/// The one-button counter switch: three ticks for the grid display, `123`
/// for numbers.
fn counter_toggle(p: &mut Ink<'_>, rect: Rect, mode: CounterMode, font: f32) {
    let radius = rect.h / 2.0;
    p.fill_rect(rect, radius, theme::PANEL);
    p.stroke_rect(rect.inset(0.5), radius, 1.0, theme::CELL_MARK);
    match mode {
        CounterMode::Grid => {
            let c = rect.center();
            let (h, step) = (rect.h * 0.45, rect.h * 0.18);
            for k in [-1.0, 0.0, 1.0] {
                let x = c.x + k * step;
                p.fill_rect(
                    Rect::centered(Point::new(x, c.y), 2.0, h),
                    0.0,
                    theme::ACCENT,
                );
            }
        }
        CounterMode::Numbers => {
            centered_text(p, "123", rect, text_font(font, true), theme::ACCENT);
        }
    }
}

// ---- board ----

pub(crate) fn brackets(p: &mut Ink<'_>, frame: Rect, len: f32, color: Rgba) {
    let w = 2.0;
    let corners = [
        (frame.x, frame.y, 1.0, 1.0),
        (frame.right(), frame.y, -1.0, 1.0),
        (frame.x, frame.bottom(), 1.0, -1.0),
        (frame.right(), frame.bottom(), -1.0, -1.0),
    ];
    for (x, y, sx, sy) in corners {
        p.line(Point::new(x, y), Point::new(x + sx * len, y), w, color);
        p.line(Point::new(x, y), Point::new(x, y + sy * len), w, color);
    }
}

fn draw_empty_board(l: &Layout, p: &mut Ink<'_>) {
    brackets(p, l.frame, l.cell * 0.25, theme::ACCENT.alpha(0.5));
    p.fill_rect(l.cells, 0.0, theme::GRID_LINE);
    p.fill_rect(l.tray, 12.0, theme::TRAY);
}

pub(crate) fn free_cell(p: &mut Ink<'_>, r: Rect) {
    p.fill_rect(r, 0.0, theme::CELL);
    let tick = r.w * 0.14;
    let off = r.w * 0.1;
    let width = (r.w * 0.03).max(1.0);
    let mark = theme::CELL_MARK;
    // A short diagonal stroke pointing inwards from each corner.
    for (x, y, sx, sy) in [
        (r.x + off, r.y + off, 1.0, 1.0),
        (r.right() - off, r.y + off, -1.0, 1.0),
        (r.x + off, r.bottom() - off, 1.0, -1.0),
        (r.right() - off, r.bottom() - off, -1.0, -1.0),
    ] {
        p.line(
            Point::new(x, y),
            Point::new(x + sx * tick, y + sy * tick),
            width,
            mark,
        );
    }
    p.fill_circle(r.center(), (r.w * 0.04).max(1.0), mark);
}

pub(crate) fn lock_cell(p: &mut Ink<'_>, r: Rect, color: Color) {
    let look = palette(color);
    p.fill_rect(r, 0.0, look.lock);
    let w = (r.w * 0.05).max(1.5);
    p.stroke_rect(r.inset(w), 1.0, w, look.bright);
    let c = r.center();
    let icon = theme::BACKGROUND.alpha(0.85);
    let body = Rect::new(c.x - r.w * 0.17, c.y - r.w * 0.04, r.w * 0.34, r.w * 0.26);
    let shackle = r.w * 0.11;
    p.push_clip(Rect::new(r.x, r.y, r.w, body.y - r.y));
    p.stroke_circle(Point::new(c.x, body.y), shackle, w, icon);
    p.pop_clip();
    p.fill_rect(body, 1.5, icon);
}

pub(crate) fn dead_cell(p: &mut Ink<'_>, r: Rect) {
    p.fill_rect(r, 0.0, theme::DEAD);
    p.push_clip(r);
    let step = r.w / 5.0;
    let w = (r.w * 0.03).max(1.0);
    for k in -5..=5 {
        let x = r.x + step * f32::from(i8::try_from(k).unwrap_or(0));
        p.line(
            Point::new(x, r.bottom()),
            Point::new(x + r.w, r.y),
            w,
            theme::DEAD_HATCH,
        );
    }
    p.pop_clip();
    empty_sign(
        p,
        r.center(),
        r.w * 0.17,
        (r.w * 0.05).max(1.5),
        theme::DEAD_ICON,
    );
}

fn draw_board(app: &App, game: &Game, p: &mut Ink<'_>) {
    let l = &app.layout;
    let board = &game.board;
    let win = if game.won {
        ease_out(game.win_elapsed / theme::WIN_TIME)
    } else {
        0.0
    };
    brackets(
        p,
        l.frame,
        l.cell * 0.25,
        theme::ACCENT.mix(theme::BRIGHT, win),
    );
    p.fill_rect(l.cells, 0.0, theme::GRID_LINE);
    let seam = (l.cell * 0.03).max(1.0);
    for row in 0..board.height() {
        for col in 0..board.width() {
            let r = l.cell_rect(row, col).inset(seam / 2.0);
            match board.cell(row, col) {
                Cell::Free => free_cell(p, r),
                Cell::Lock(color) => lock_cell(p, r, color),
                Cell::Dead => dead_cell(p, r),
            }
        }
    }
    draw_silhouette(app, game, p);
    if win > 0.0 {
        p.fill_rect(l.frame, 4.0, theme::BRIGHT.alpha(0.06 * win));
        p.stroke_rect(l.frame, 4.0, 2.0, theme::BRIGHT.alpha(win));
    }
    // Landing shadow of the dragged piece.
    if let Gesture::Dragging(drag) = app.gesture
        && let Some((row, col)) = app.drop_target(&drag)
    {
        let ok = board.fits(drag.piece, board.piece(drag.piece).turns, row, col);
        let color = if ok {
            palette(board.piece_color(drag.piece)).accent
        } else {
            theme::ERROR
        };
        for &(dr, dc) in board.shape(drag.piece).cells() {
            let (r, c) = (
                row + isize::try_from(dr).unwrap_or(0),
                col + isize::try_from(dc).unwrap_or(0),
            );
            if let (Ok(r), Ok(c)) = (usize::try_from(r), usize::try_from(c))
                && r < board.height()
                && c < board.width()
            {
                let rect = l.cell_rect(r, c).inset(seam);
                p.fill_rect(rect, 2.0, color.alpha(0.3));
                p.stroke_rect(rect.inset(1.0), 2.0, 1.5, color.alpha(0.8));
            }
        }
    }
}

/// The hint silhouette: a faint hatched fill on every cell the solution
/// covers, in the color of the piece there, with no borders between
/// pieces. It goes with the win.
fn draw_silhouette(app: &App, game: &Game, p: &mut Ink<'_>) {
    if game.won {
        return;
    }
    let l = &app.layout;
    let width = game.board.width();
    let stroke = (l.cell * 0.03).max(1.0);
    for (k, color) in app.silhouette().iter().enumerate() {
        let Some(color) = color else {
            continue;
        };
        let accent = palette(*color).accent;
        let (fill, hatch) = (accent.alpha(0.16), accent.alpha(0.3));
        let r = l.cell_rect(k / width, k % width);
        p.fill_rect(r, 0.0, fill);
        p.push_clip(r);
        let step = r.w / 4.0;
        for j in -4..=4 {
            let x = r.x + step * f32::from(i8::try_from(j).unwrap_or(0));
            p.line(
                Point::new(x, r.bottom()),
                Point::new(x + r.w, r.y),
                stroke,
                hatch,
            );
        }
        p.pop_clip();
    }
}

// ---- counters ----

/// The color of a counter in the state of `line`.
fn line_color(line: Line, look: Palette) -> Rgba {
    match line.state() {
        LineState::Under => look.accent,
        LineState::Exact => look.bright,
        LineState::Over => theme::ERROR,
    }
}

/// Draws the counters of one line in `area`: for a row (`row`), the band
/// part left of the row; for a column, the band part above it.
fn line_counter(
    p: &mut Ink<'_>,
    area: Rect,
    lines: &[(Color, Line)],
    row: bool,
    numbers: bool,
    cell: f32,
    font: f32,
) {
    let counters = line_counters(lines, area, row, cell);
    let lone = counters.len() == 1;
    for &Counter {
        color,
        line,
        slot,
        view,
    } in counters.iter()
    {
        let look = palette(color);
        // A zero clue: grey while the line is empty, red once cells of the
        // color stand in it.
        let zero = if line.filled == 0 {
            theme::MUTED
        } else {
            theme::ERROR
        };
        if numbers {
            let ink = if line.clue == 0 {
                zero
            } else {
                line_color(line, look)
            };
            let font = if lone { font } else { font * 0.85 };
            number(p, slot, line, ink, row, font);
            continue;
        }
        match view {
            CounterView::Empty { center, radius } => {
                let width = (cell * 0.05).max(1.5).min(radius * 0.4);
                empty_sign(p, center, radius, width, zero);
            }
            CounterView::Ticks { .. } => {
                let outline = (cell * TICK_THICK * 0.22).clamp(1.0, 1.5);
                for (k, rect) in view.marks().enumerate() {
                    match line.state() {
                        LineState::Exact => p.fill_rect(rect, 0.0, look.bright),
                        LineState::Over => p.fill_rect(rect, 0.0, theme::ERROR),
                        LineState::Under if k < line.filled => {
                            p.fill_rect(rect, 0.0, look.accent);
                        }
                        LineState::Under => {
                            p.stroke_rect(rect.inset(outline / 2.0), 0.0, outline, look.accent);
                        }
                    }
                }
            }
        }
    }
}

fn number(p: &mut Ink<'_>, area: Rect, line: Line, color: Rgba, row: bool, font: f32) {
    let size = (if row { area.h } else { area.w } * 0.36).min(font);
    let bold = line.state() == LineState::Exact;
    let font = text_font(size, bold);
    let room = area.w * 0.92;
    // "filled/clue" when it fits, in a column with room to spare so that
    // neighbours do not run together; else the clue alone, the color
    // telling the state.
    let mut label = format!("{}/{}", line.filled, line.clue);
    let both = if row { room } else { area.w * 0.8 };
    if p.text_width(&label, font) > both + 0.01 {
        label = format!("{}", line.clue);
    }
    let font = fit(p, &label, font, room);
    if row {
        let anchor = Point::new(area.right() - area.w * 0.04, area.center().y);
        p.text(&label, anchor, font, Align::Right, color);
    } else {
        let anchor = Point::new(area.center().x, area.bottom() - area.h * 0.35);
        p.text(&label, anchor, font, Align::Center, color);
    }
}

fn draw_counters(app: &App, game: &Game, p: &mut Ink<'_>) {
    let l = &app.layout;
    let board = &game.board;
    let numbers = app.counters == CounterMode::Numbers || l.numbers_only;
    let colors = board.colors();
    let rows: Vec<Vec<Line>> = colors.iter().map(|&c| board.rows_of(c)).collect();
    let cols: Vec<Vec<Line>> = colors.iter().map(|&c| board.cols_of(c)).collect();
    // The colors of one line, without allocating.
    let of = |lines: &[Vec<Line>], k: usize| {
        let mut out = [(Color::DEFAULT, Line::default()); MAX_COLORS];
        for (slot, (&c, l)) in out.iter_mut().zip(colors.iter().zip(lines)) {
            *slot = (c, l[k]);
        }
        (out, colors.len().min(MAX_COLORS))
    };
    for row in 0..board.height() {
        let cell = l.cell_rect(row, 0);
        let area = Rect::new(l.row_counters.x, cell.y, l.row_counters.w, cell.h);
        let (lines, n) = of(&rows, row);
        line_counter(p, area, &lines[..n], true, numbers, l.cell, l.font);
    }
    for col in 0..board.width() {
        let cell = l.cell_rect(0, col);
        let area = Rect::new(cell.x, l.col_counters.y, cell.w, l.col_counters.h);
        let (lines, n) = of(&cols, col);
        line_counter(p, area, &lines[..n], false, numbers, l.cell, l.font);
    }
}

// ---- tray and pieces ----

fn draw_tray(app: &App, game: &Game, p: &mut Ink<'_>) {
    let l = &app.layout;
    if game.won && l.kind != Arrangement::Wide && !app.is_generating() {
        // The banner takes the empty tray's place.
        return;
    }
    p.fill_rect(l.tray, 12.0, theme::TRAY);
    let dragged = app.dragged_piece();
    let hovered = app
        .hover
        .filter(|_| app.gesture == Gesture::Idle && !game.won && !app.is_generating());
    draw_pager(app, game.board.piece_count(), p);
    for i in 0..game.board.piece_count() {
        let Some(card) = app.card_of(i) else {
            continue;
        };
        let away = game.board.piece(i).at.is_some() || dragged == Some(i);
        if away {
            p.fill_rect(card, 8.0, theme::CARD.alpha(0.45));
        } else {
            p.fill_rect(card, 8.0, theme::CARD);
            if hovered.is_some_and(|h| card.contains(h)) {
                p.stroke_rect(card.inset(0.5), 8.0, 1.0, theme::ACCENT.alpha(0.6));
            }
        }
    }
}

/// The page switcher, when the tray has pages.
pub(crate) fn draw_pager(app: &App, cards: usize, p: &mut Ink<'_>) {
    let l = &app.layout;
    let Some(pager) = l.pager else {
        return;
    };
    let font = l.font;
    button(p, pager.prev, "‹", Style::Normal, font * 1.2);
    button(p, pager.next, "›", Style::Normal, font * 1.2);
    let label = format!("{} / {}", app.tray_page + 1, l.pages(cards));
    centered_text(
        p,
        &label,
        pager.label,
        text_font(font * 0.9, false),
        theme::MUTED,
    );
}

fn draw_pieces(app: &App, game: &Game, p: &mut Ink<'_>) {
    let win = if game.won {
        ease_out(game.win_elapsed / theme::WIN_TIME)
    } else {
        0.0
    };
    let dragged = app.dragged_piece();
    let moving = |i: usize| game.anims.get(i).is_some_and(|a| a.fly.is_some());
    let count = game.board.piece_count();
    // Tray pieces on other pages are hidden once they have landed.
    let shown =
        |i: usize| game.board.piece(i).at.is_some() || app.card_of(i).is_some() || moving(i);
    let order = (0..count)
        .filter(|&i| Some(i) != dragged && !moving(i) && shown(i))
        .chain((0..count).filter(|&i| Some(i) != dragged && moving(i)))
        .chain(dragged);
    for i in order {
        let (center, cell) = app.piece_visual(i);
        let anim = game.anims.get(i);
        let spin = anim.and_then(|a| a.spin).map_or(0.0, |t| {
            -FRAC_PI_2 * (1.0 - ease_out(t / theme::ROTATE_TIME))
        });
        // A turn that did not fit: a short shake that dies down.
        let wobble = anim.and_then(|a| a.wobble).map_or(0.0, |t| {
            let left = 1.0 - (t / theme::WOBBLE_TIME).clamp(0.0, 1.0);
            0.14 * left * math::sin(t / theme::WOBBLE_TIME * 3.0 * TAU)
        });
        let spin = spin + wobble;
        let blamed = anim.and_then(|a| a.blame).map_or(0.0, |t| {
            1.0 - (t / theme::BLAME_TIME - 0.8).clamp(0.0, 0.2) * 5.0
        });
        let look = palette(game.board.piece_color(i));
        let fill = look
            .fill
            .mix(look.bright, win * 0.35)
            .mix(theme::ERROR, blamed);
        let alpha = if Some(i) == dragged { 0.92 } else { 1.0 };
        piece(
            p,
            game.board.shape(i),
            center,
            cell,
            spin,
            Tint {
                fill: fill.alpha(alpha),
                edge: look.bright.alpha(alpha),
            },
        );
    }
}

/// The colors a piece is drawn in.
#[derive(Clone, Copy)]
pub(crate) struct Tint {
    pub fill: Rgba,
    pub edge: Rgba,
}

impl Tint {
    /// A piece of `color` as it rests.
    pub(crate) fn of(color: Color) -> Tint {
        let look = palette(color);
        Tint {
            fill: look.fill,
            edge: look.bright,
        }
    }
}

/// Draws a piece centered on `center` with cells of side `cell`, turned by
/// `angle` radians (clockwise) about its center.
pub(crate) fn piece(
    p: &mut Ink<'_>,
    shape: &Shape,
    center: Point,
    cell: f32,
    angle: f32,
    tint: Tint,
) {
    piece_with(p, shape, center, cell, angle, tint, true);
}

/// The app icon in the square from the origin to (`side`, `side`): a dark
/// rounded square with a three-cell corner piece in the game's style. Below
/// 20 pixels the seams between cells are left out; only the outline stays.
pub fn draw_icon(painter: &mut dyn Painter, side: f32) {
    let mut p = Ink::new(painter);
    p.fill_rect(Rect::new(0.0, 0.0, side, side), side * 0.18, theme::PANEL);
    let Ok(shape) = Shape::new([(0, 0), (1, 0), (1, 1)]) else {
        return;
    };
    let center = Point::new(side / 2.0, side / 2.0);
    piece_with(
        &mut p,
        &shape,
        center,
        side * 0.32,
        0.0,
        Tint::of(Color::DEFAULT),
        side >= 20.0,
    );
}

/// [`piece`], with or without the seams between cells.
fn piece_with(
    p: &mut Ink<'_>,
    shape: &Shape,
    center: Point,
    cell: f32,
    angle: f32,
    tint: Tint,
    seams: bool,
) {
    let Tint { fill, edge } = tint;
    let (half_w, half_h) = (to_f32(shape.width()) / 2.0, to_f32(shape.height()) / 2.0);
    let (sin, cos) = math::sin_cos(angle);
    let map = |x: f32, y: f32| {
        let (lx, ly) = ((x - half_w) * cell, (y - half_h) * cell);
        Point::new(
            center.x + lx * cos - ly * sin,
            center.y + lx * sin + ly * cos,
        )
    };
    let has = |row: usize, col: usize| shape.contains(row, col);
    let seam = theme::BACKGROUND.alpha(0.35);
    let edge_w = (cell * 0.07).max(1.5);
    for &(row, col) in shape.cells() {
        let (cx, cy) = (to_f32(col), to_f32(row));
        p.fill_polygon(
            &[
                map(cx, cy),
                map(cx + 1.0, cy),
                map(cx + 1.0, cy + 1.0),
                map(cx, cy + 1.0),
            ],
            fill,
        );
    }
    for &(row, col) in shape.cells().iter().filter(|_| seams) {
        let (cx, cy) = (to_f32(col), to_f32(row));
        if has(row, col + 1) {
            p.line(map(cx + 1.0, cy), map(cx + 1.0, cy + 1.0), 1.0, seam);
        }
        if has(row + 1, col) {
            p.line(map(cx, cy + 1.0), map(cx + 1.0, cy + 1.0), 1.0, seam);
        }
    }
    // Outer contour, drawn half a line inside the cells.
    let inset = edge_w / (2.0 * cell);
    for &(row, col) in shape.cells() {
        let (cx, cy) = (to_f32(col), to_f32(row));
        if row == 0 || !has(row - 1, col) {
            p.line(map(cx, cy + inset), map(cx + 1.0, cy + inset), edge_w, edge);
        }
        if !has(row + 1, col) {
            p.line(
                map(cx, cy + 1.0 - inset),
                map(cx + 1.0, cy + 1.0 - inset),
                edge_w,
                edge,
            );
        }
        if col == 0 || !has(row, col - 1) {
            p.line(map(cx + inset, cy), map(cx + inset, cy + 1.0), edge_w, edge);
        }
        if !has(row, col + 1) {
            p.line(
                map(cx + 1.0 - inset, cy),
                map(cx + 1.0 - inset, cy + 1.0),
                edge_w,
                edge,
            );
        }
    }
}

// ---- overlays ----

fn pill(p: &mut Ink<'_>, center: Point, label: &str, font: Font, stroke: Rgba, alpha: f32) {
    let w = p.text_width(label, font) + font.size * 2.4;
    let rect = Rect::centered(center, w, font.size * 2.6);
    p.fill_rect(rect, rect.h / 2.0, theme::PANEL.alpha(0.96 * alpha));
    p.stroke_rect(rect.inset(0.5), rect.h / 2.0, 1.5, stroke.alpha(alpha));
    p.text(label, center, font, Align::Center, theme::TEXT.alpha(alpha));
}

fn draw_overlays(app: &App, p: &mut Ink<'_>) {
    let l = &app.layout;
    if app.is_generating() {
        p.fill_rect(l.board_block(), 6.0, theme::BACKGROUND.alpha(0.6));
        let label = text(app.lang, Text::Generating);
        pill(
            p,
            l.frame.center(),
            label,
            text_font(l.font, true),
            theme::ACCENT,
            1.0,
        );
    }
    if let Some(game) = &app.game
        && game.won
        && !app.is_generating()
    {
        let t = ease_out(game.win_elapsed / theme::WIN_TIME);
        let banner = l.banner;
        p.fill_rect(banner, 12.0, theme::PANEL.alpha(0.97 * t));
        p.stroke_rect(banner.inset(0.75), 12.0, 1.5, theme::BRIGHT.alpha(t));
        // The text fills the part left of the button.
        let title_area = if l.next.x > banner.center().x {
            Rect::new(banner.x, banner.y, l.next.x - banner.x, banner.h)
        } else {
            Rect::new(banner.x, banner.y, banner.w, l.next.y - banner.y)
        };
        let label = text(app.lang, Text::Complete);
        let font = fit(p, label, text_font(l.font * 1.2, true), title_area.w - 16.0);
        let used = game.hints.used;
        let center = title_area.center();
        let (title_y, hints_y) = if used > 0 {
            (center.y - l.font * 0.6, center.y + l.font * 0.75)
        } else {
            (center.y, center.y)
        };
        p.text(
            label,
            Point::new(center.x, title_y),
            font,
            Align::Center,
            theme::BRIGHT.alpha(t),
        );
        if used > 0 {
            let line = format!("{} {used}", text(app.lang, Text::HintsUsed));
            let font = fit(
                p,
                &line,
                text_font(l.font * 0.85, false),
                title_area.w - 16.0,
            );
            p.text(
                &line,
                Point::new(center.x, hints_y),
                font,
                Align::Center,
                theme::MUTED.alpha(t),
            );
        }
        button(
            p,
            l.next,
            text(
                app.lang,
                if app.is_custom() {
                    Text::Edit
                } else {
                    Text::Next
                },
            ),
            Style::Primary,
            l.font,
        );
    }
    if let Some(toast) = app.toast {
        let remaining = theme::TOAST_TIME - toast.elapsed;
        let alpha = (remaining / 0.3).clamp(0.0, 1.0) * (toast.elapsed / 0.12).clamp(0.0, 1.0);
        let stroke = if toast.error {
            theme::ERROR
        } else {
            theme::ACCENT
        };
        let label = text(app.lang, toast.text);
        let font = fit(p, label, text_font(l.font * 0.95, true), app.width * 0.8);
        pill(p, l.notice, label, font, stroke, alpha);
    }
}
