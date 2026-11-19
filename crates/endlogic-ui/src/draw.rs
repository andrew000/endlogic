//! Drawing the whole screen with [`Painter`] primitives.

use core::f32::consts::FRAC_PI_2;

use endlogic_core::generator::Difficulty;
use endlogic_core::{Cell, Shape};

use crate::app::{App, CounterMode, Game, Gesture};
use crate::board::{Line, LineState};
use crate::layout::{Layout, to_f32};
use crate::painter::{Align, Font, FontFamily, Painter, Point, Rect, Rgba};
use crate::text::{Lang, Text, difficulty_label, text};
use crate::theme::{self, ease_out};

/// Text shown in place of the puzzle id while there is no puzzle.
const NO_ID: &str = "Δ-…";

pub(crate) fn draw(app: &App, p: &mut dyn Painter) {
    let l = &app.layout;
    p.fill_rect(
        Rect::new(0.0, 0.0, app.width, app.height),
        0.0,
        theme::BACKGROUND,
    );
    draw_header(app, p);
    draw_top_controls(app, p);
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

fn sans(size: f32, bold: bool) -> Font {
    Font {
        family: FontFamily::Sans,
        size,
        bold,
    }
}

fn mono(size: f32, bold: bool) -> Font {
    Font {
        family: FontFamily::Mono,
        size,
        bold,
    }
}

/// `font`, shrunk if needed so that `text` is at most `max_width` wide.
fn fit(p: &mut dyn Painter, text: &str, font: Font, max_width: f32) -> Font {
    let width = p.text_width(text, font);
    if width <= max_width || width <= 0.0 {
        font
    } else {
        Font {
            size: (font.size * max_width / width).max(6.0),
            ..font
        }
    }
}

/// Draws `text` centered in `rect`, shrinking it to fit.
fn centered_text(p: &mut dyn Painter, text: &str, rect: Rect, font: Font, color: Rgba) {
    let font = fit(p, text, font, rect.w - 8.0);
    p.text(text, rect.center(), font, Align::Center, color);
}

/// Splits `text` into lines no wider than `max_width`, breaking at spaces.
fn wrap(p: &mut dyn Painter, text: &str, font: Font, max_width: f32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split(' ') {
        let candidate = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if line.is_empty() || p.text_width(&candidate, font) <= max_width {
            line = candidate;
        } else {
            lines.push(std::mem::replace(&mut line, word.to_owned()));
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// Draws wrapped text from the top of `rect`, clipped to it.
fn paragraph(p: &mut dyn Painter, text: &str, rect: Rect, font: Font, color: Rgba) {
    let line_h = font.size * 1.35;
    p.push_clip(rect);
    for (k, line) in wrap(p, text, font, rect.w).iter().enumerate() {
        let y = rect.y + line_h * (to_f32(k) + 0.5);
        p.text(line, Point::new(rect.x, y), font, Align::Left, color);
    }
    p.pop_clip();
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Style {
    Normal,
    Primary,
    Disabled,
}

fn button(p: &mut dyn Painter, rect: Rect, label: &str, style: Style, font: f32) {
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
    centered_text(p, label, rect, sans(font, true), color);
}

/// A pill split into equal segments; `active` is highlighted.
fn segmented(p: &mut dyn Painter, rect: Rect, labels: &[&str], active: Option<usize>, font: f32) {
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
        centered_text(p, label, seg, sans(font, true), color);
    }
}

/// A keyboard key cap with `label`.
fn keycap(p: &mut dyn Painter, rect: Rect, label: &str, font: f32) {
    p.stroke_rect(rect.inset(0.5), 4.0, 1.0, theme::MUTED);
    centered_text(p, label, rect, mono(font, true), theme::TEXT);
}

/// A mouse outline with the left button filled.
fn mouse_icon(p: &mut dyn Painter, rect: Rect) {
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
fn empty_sign(p: &mut dyn Painter, center: Point, radius: f32, width: f32, color: Rgba) {
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

fn draw_header(app: &App, p: &mut dyn Painter) {
    let l = &app.layout;
    if let Some(panel) = l.panel {
        p.fill_rect(panel, 10.0, theme::PANEL);
    }
    // Title: `//` in the accent color, then the name.
    let title_font = fit(p, "// ENDLOGIC", mono(l.font * 1.4, true), l.title.w);
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
    let font = fit(p, &id, mono(l.font * 0.9, false), r.w - 12.0);
    p.text(
        &id,
        Point::new(r.x + 12.0, r.center().y),
        font,
        Align::Left,
        theme::MUTED,
    );

    if let Some(rect) = l.subtitle {
        paragraph(
            p,
            text(app.lang, Text::Subtitle),
            rect,
            sans(l.font * 0.85, false),
            theme::MUTED,
        );
    }
    if let Some(rect) = l.help {
        draw_help(app, rect, p);
    }
}

fn draw_help(app: &App, rect: Rect, p: &mut dyn Painter) {
    let l = &app.layout;
    let row_h = rect.h / 3.0;
    let icon_w = (l.font * 2.8).min(rect.w * 0.3);
    let small = l.font * 0.85;
    let rows = [
        (Text::HelpDrag, None),
        (Text::HelpRotate, Some("R")),
        (Text::HelpDisplay, Some("TAB")),
    ];
    for (k, (key, cap)) in rows.into_iter().enumerate() {
        let row = Rect::new(rect.x, rect.y + row_h * to_f32(k), rect.w, row_h);
        let icon = Rect::new(row.x, row.y + row_h * 0.15, icon_w, row_h * 0.6);
        match cap {
            Some(label) => {
                let w = (p.text_width(label, mono(small, true)) + 12.0).min(icon_w);
                let cap = Rect::new(
                    icon.x,
                    icon.center().y - small,
                    w.max(small * 2.0),
                    small * 2.0,
                );
                keycap(p, cap, label, small * 0.9);
            }
            None => mouse_icon(p, icon),
        }
        let text_rect = Rect::new(
            row.x + icon_w + 8.0,
            row.y + row_h * 0.15 - small * 0.1,
            row.w - icon_w - 8.0,
            row_h * 0.85,
        );
        paragraph(
            p,
            text(app.lang, key),
            text_rect,
            sans(small, false),
            theme::TEXT,
        );
    }
}

fn draw_top_controls(app: &App, p: &mut dyn Painter) {
    let l = &app.layout;
    let labels: Vec<&str> = Difficulty::ALL
        .iter()
        .map(|&d| difficulty_label(app.lang, d))
        .collect();
    let active = Difficulty::ALL.iter().position(|&d| d == app.difficulty);
    let tabs = Rect::new(
        l.tabs[0].x,
        l.tabs[0].y,
        l.tabs[3].right() - l.tabs[0].x,
        l.tabs[0].h,
    );
    segmented(p, tabs, &labels, active, l.font * 0.9);
    let lang = [Lang::En, Lang::Ua];
    segmented(
        p,
        l.lang,
        &[Lang::En.label(), Lang::Ua.label()],
        lang.iter().position(|&x| x == app.lang),
        l.font * 0.9,
    );
    let style = if app.game.is_some() {
        Style::Normal
    } else {
        Style::Disabled
    };
    button(p, l.share, text(app.lang, Text::Share), style, l.font * 0.9);
}

fn draw_bottom_controls(app: &App, p: &mut dyn Painter) {
    let l = &app.layout;
    let movable = app.game.as_ref().is_some_and(|g| !g.won) && !app.is_generating();
    let style = if movable {
        Style::Normal
    } else {
        Style::Disabled
    };
    button(p, l.reset, text(app.lang, Text::Reset), style, l.font);
    if let Some(hint) = l.tab_hint {
        keycap(p, hint, "TAB", l.font * 0.7);
    }
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
    button(p, l.new, text(app.lang, Text::New), Style::Primary, l.font);
}

// ---- board ----

fn brackets(p: &mut dyn Painter, frame: Rect, len: f32, color: Rgba) {
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

fn draw_empty_board(l: &Layout, p: &mut dyn Painter) {
    brackets(p, l.frame, l.cell * 0.5, theme::ACCENT.alpha(0.5));
    p.fill_rect(l.cells, 0.0, theme::GRID_LINE);
    p.fill_rect(l.tray, 12.0, theme::TRAY);
}

fn free_cell(p: &mut dyn Painter, r: Rect) {
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

fn lock_cell(p: &mut dyn Painter, r: Rect) {
    p.fill_rect(r, 0.0, theme::LOCK);
    let w = (r.w * 0.05).max(1.5);
    p.stroke_rect(r.inset(w), 1.0, w, theme::BRIGHT);
    let c = r.center();
    let icon = theme::BACKGROUND.alpha(0.85);
    let body = Rect::new(c.x - r.w * 0.17, c.y - r.w * 0.04, r.w * 0.34, r.w * 0.26);
    let shackle = r.w * 0.11;
    p.push_clip(Rect::new(r.x, r.y, r.w, body.y - r.y));
    p.stroke_circle(Point::new(c.x, body.y), shackle, w, icon);
    p.pop_clip();
    p.fill_rect(body, 1.5, icon);
}

fn dead_cell(p: &mut dyn Painter, r: Rect) {
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

fn draw_board(app: &App, game: &Game, p: &mut dyn Painter) {
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
        l.cell * 0.5,
        theme::ACCENT.mix(theme::BRIGHT, win),
    );
    p.fill_rect(l.cells, 0.0, theme::GRID_LINE);
    let seam = (l.cell * 0.03).max(1.0);
    for row in 0..board.height() {
        for col in 0..board.width() {
            let r = l.cell_rect(row, col).inset(seam / 2.0);
            match board.cell(row, col) {
                Cell::Free => free_cell(p, r),
                Cell::Lock(_) => lock_cell(p, r),
                Cell::Dead => dead_cell(p, r),
            }
        }
    }
    if win > 0.0 {
        p.fill_rect(l.frame, 4.0, theme::BRIGHT.alpha(0.06 * win));
        p.stroke_rect(l.frame, 4.0, 2.0, theme::BRIGHT.alpha(win));
    }
    // Landing shadow of the dragged piece.
    if let Gesture::Dragging(drag) = app.gesture
        && let Some((row, col)) = app.drop_target(&drag)
    {
        let ok = board.fits(drag.piece, board.piece(drag.piece).turns, row, col);
        let color = if ok { theme::ACCENT } else { theme::ERROR };
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

// ---- counters ----

fn line_color(line: Line) -> Rgba {
    match line.state() {
        LineState::Under => theme::ACCENT,
        LineState::Exact => theme::BRIGHT,
        LineState::Over => theme::ERROR,
    }
}

/// Draws one counter in `area`; `vertical_ticks` for rows (ticks side by
/// side), otherwise columns (ticks stacked).
fn counter(p: &mut dyn Painter, area: Rect, line: Line, mode: CounterMode, rows: bool, font: f32) {
    let cell = if rows { area.h } else { area.w };
    if line.clue == 0 {
        let color = if line.filled == 0 {
            theme::MUTED
        } else {
            theme::ERROR
        };
        match mode {
            CounterMode::Grid => {
                let c = if rows {
                    Point::new(area.right() - cell * 0.4, area.center().y)
                } else {
                    Point::new(area.center().x, area.bottom() - cell * 0.4)
                };
                empty_sign(p, c, cell * 0.16, (cell * 0.05).max(1.5), color);
            }
            CounterMode::Numbers => number(p, area, line, color, rows, font),
        }
        return;
    }
    match mode {
        CounterMode::Numbers => number(p, area, line, line_color(line), rows, font),
        CounterMode::Grid => ticks(p, area, line, rows, cell),
    }
}

fn number(p: &mut dyn Painter, area: Rect, line: Line, color: Rgba, rows: bool, font: f32) {
    let label = format!("{}/{}", line.filled, line.clue);
    let size = (if rows { area.h } else { area.w } * 0.36).min(font);
    let bold = line.state() == LineState::Exact;
    let font = fit(
        p,
        &label,
        mono(size, bold),
        if rows { area.w } else { area.w - 2.0 } * 0.9,
    );
    if rows {
        let anchor = Point::new(area.right() - area.w * 0.12, area.center().y);
        p.text(&label, anchor, font, Align::Right, color);
    } else {
        let anchor = Point::new(area.center().x, area.bottom() - area.h * 0.3);
        p.text(&label, anchor, font, Align::Center, color);
    }
}

fn ticks(p: &mut dyn Painter, area: Rect, line: Line, rows: bool, cell: f32) {
    let n = line.clue;
    let depth = if rows { area.w } else { area.h };
    let room = depth * 0.85;
    let spacing = (cell * 0.18).min(room / to_f32(n));
    let thick = (spacing * 0.55).clamp(1.5, cell * 0.08);
    let long = cell * 0.5;
    let state = line.state();
    for k in 0..n {
        // Tick 0 is next to the board.
        let offset = depth * 0.08 + spacing * (to_f32(k) + 0.5);
        let rect = if rows {
            Rect::centered(
                Point::new(area.right() - offset, area.center().y),
                thick,
                long,
            )
        } else {
            Rect::centered(
                Point::new(area.center().x, area.bottom() - offset),
                long,
                thick,
            )
        };
        let filled = k < line.filled;
        match state {
            LineState::Exact => p.fill_rect(rect, 0.0, theme::BRIGHT),
            LineState::Over => p.fill_rect(rect, 0.0, theme::ERROR),
            LineState::Under if filled => p.fill_rect(rect, 0.0, theme::ACCENT),
            LineState::Under => p.stroke_rect(rect.inset(0.5), 0.0, 1.0, theme::ACCENT),
        }
    }
}

fn draw_counters(app: &App, game: &Game, p: &mut dyn Painter) {
    let l = &app.layout;
    let font = l.font;
    for (row, line) in game.board.rows().into_iter().enumerate() {
        let cell = l.cell_rect(row, 0);
        let area = Rect::new(l.row_counters.x, cell.y, l.row_counters.w, cell.h);
        counter(p, area, line, app.counters, true, font);
    }
    for (col, line) in game.board.cols().into_iter().enumerate() {
        let cell = l.cell_rect(0, col);
        let area = Rect::new(cell.x, l.col_counters.y, cell.w, l.col_counters.h);
        counter(p, area, line, app.counters, false, font);
    }
}

// ---- tray and pieces ----

fn draw_tray(app: &App, game: &Game, p: &mut dyn Painter) {
    let l = &app.layout;
    p.fill_rect(l.tray, 12.0, theme::TRAY);
    let dragged = app.dragged_piece();
    let hovered = app
        .hover
        .filter(|_| app.gesture == Gesture::Idle && !game.won && !app.is_generating());
    for (i, &card) in l.cards.iter().enumerate() {
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

fn draw_pieces(app: &App, game: &Game, p: &mut dyn Painter) {
    let win = if game.won {
        ease_out(game.win_elapsed / theme::WIN_TIME)
    } else {
        0.0
    };
    let dragged = app.dragged_piece();
    let moving = |i: usize| game.anims.get(i).is_some_and(|a| a.fly.is_some());
    let count = game.board.piece_count();
    let order = (0..count)
        .filter(|&i| Some(i) != dragged && !moving(i))
        .chain((0..count).filter(|&i| Some(i) != dragged && moving(i)))
        .chain(dragged);
    for i in order {
        let (center, cell) = app.piece_visual(i);
        let spin = game.anims.get(i).and_then(|a| a.spin).map_or(0.0, |t| {
            -FRAC_PI_2 * (1.0 - ease_out(t / theme::ROTATE_TIME))
        });
        let fill = theme::PIECE.mix(theme::BRIGHT, win * 0.35);
        let alpha = if Some(i) == dragged { 0.92 } else { 1.0 };
        piece(
            p,
            game.board.shape(i),
            center,
            cell,
            spin,
            fill.alpha(alpha),
        );
    }
}

/// Draws a piece centered on `center` with cells of side `cell`, turned by
/// `angle` radians (clockwise) about its center.
fn piece(p: &mut dyn Painter, shape: &Shape, center: Point, cell: f32, angle: f32, fill: Rgba) {
    let (half_w, half_h) = (to_f32(shape.width()) / 2.0, to_f32(shape.height()) / 2.0);
    let (sin, cos) = angle.sin_cos();
    let map = |x: f32, y: f32| {
        let (lx, ly) = ((x - half_w) * cell, (y - half_h) * cell);
        Point::new(
            center.x + lx * cos - ly * sin,
            center.y + lx * sin + ly * cos,
        )
    };
    let has = |row: usize, col: usize| shape.contains(row, col);
    let edge = theme::BRIGHT.alpha(f32::from(fill.a) / 255.0);
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
    for &(row, col) in shape.cells() {
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

fn pill(p: &mut dyn Painter, center: Point, label: &str, font: Font, stroke: Rgba, alpha: f32) {
    let w = p.text_width(label, font) + font.size * 2.4;
    let rect = Rect::centered(center, w, font.size * 2.6);
    p.fill_rect(rect, rect.h / 2.0, theme::PANEL.alpha(0.96 * alpha));
    p.stroke_rect(rect.inset(0.5), rect.h / 2.0, 1.5, stroke.alpha(alpha));
    p.text(label, center, font, Align::Center, theme::TEXT.alpha(alpha));
}

fn draw_overlays(app: &App, p: &mut dyn Painter) {
    let l = &app.layout;
    if app.is_generating() {
        p.fill_rect(l.board_block(), 6.0, theme::BACKGROUND.alpha(0.6));
        let label = text(app.lang, Text::Generating);
        pill(
            p,
            l.frame.center(),
            label,
            sans(l.font, true),
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
        let title_area = Rect::new(banner.x, banner.y, banner.w, l.next.y - banner.y);
        let label = text(app.lang, Text::Complete);
        let font = fit(p, label, mono(l.font * 1.3, true), banner.w - 24.0);
        p.text(
            label,
            title_area.center(),
            font,
            Align::Center,
            theme::BRIGHT.alpha(t),
        );
        button(
            p,
            l.next,
            text(app.lang, Text::Next),
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
        let font = fit(p, label, sans(l.font * 0.95, true), app.width * 0.8);
        pill(p, l.notice, label, font, stroke, alpha);
    }
}
