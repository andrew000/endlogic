//! Drawing the editor of the custom tab.

use alloc::borrow::ToOwned;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::math;

use endlogic_core::{Cell, Color};

use super::{
    Style, Tint, brackets, button, centered_text, dead_cell, draw_pager, fit, free_cell, lock_cell,
    piece, text_font,
};
use crate::app::App;
use crate::editor::{ClueRef, Draft, DraftError, Editor, SolveState};
use crate::font::{Align, Font};
use crate::ink::Ink;
use crate::layout::{EditLayout, Stepper, to_f32};
use crate::painter::{Point, Rect, Rgba};
use crate::text::{Text, problem_line, problem_text, solution_of, text};
use crate::theme::{self, palette};
use endlogic_vision::Place;

pub(super) fn draw(app: &App, p: &mut Ink<'_>) {
    let l = &app.layout;
    let (Some(e), Some(editor)) = (l.edit, app.editor()) else {
        return;
    };
    draw_board(app, editor, p);
    draw_clues(app, editor, p);
    draw_solution(app, p);
    draw_status(app, &e, p);
    if app.asks_replace() {
        if !e.result.intersects(l.tray) {
            draw_tray(app, editor, p);
        }
        draw_controls(app, &e, editor, p);
        draw_doubts(app, p, false);
        draw_prompt(app, &e, p);
        return;
    }
    let overlay = if app.draft().is_some() {
        Some(e.piece)
    } else if app.selected_clue().is_some() {
        Some(e.strip)
    } else {
        None
    };
    let answer = app.solve_state().is_some();
    let answer_on_tray = answer && e.result.intersects(l.tray);
    if overlay.is_none_or(|o| !o.intersects(l.tray)) && !answer_on_tray {
        draw_tray(app, editor, p);
    }
    if answer && overlay.is_none_or(|o| !o.intersects(e.result)) {
        draw_answer(app, &e, p);
    }
    draw_controls(app, &e, editor, p);
    let tray_shown = overlay.is_none_or(|o| !o.intersects(l.tray)) && !answer_on_tray;
    draw_doubts(app, p, tray_shown);
    if let Some(draft) = app.draft() {
        draw_piece_editor(app, &e, draft, p);
    } else if let Some(clue) = app.selected_clue() {
        draw_strip(app, &e, editor, clue, p);
    }
}

/// Yellow outlines around the places a screenshot reading is not sure
/// about: clue fields, cells, and (when the tray is on screen) cards.
fn draw_doubts(app: &App, p: &mut Ink<'_>, tray: bool) {
    let l = &app.layout;
    let fields = app.clue_fields();
    let seam = (l.cell * 0.03).max(1.0);
    for doubt in app.doubts() {
        let rect = match doubt.place {
            Place::Clue { color, row, index } => fields
                .iter()
                .find(|(c, _)| c.color == color && c.row == row && c.index == index)
                .map(|&(_, area)| Rect::centered(area.center(), area.w * 0.9, area.h * 0.9)),
            Place::Cell { row, col } => Some(l.cell_rect(row, col).inset(seam / 2.0)),
            Place::Piece(i) => app.card_of(i).filter(|_| tray),
        };
        if let Some(rect) = rect
            && !app.under_overlay(rect)
        {
            p.stroke_rect(rect.inset(-1.0), 4.0, 2.0, theme::MARKER);
        }
    }
}

/// "Replace the current puzzle?" with Yes and No, in the answer's place.
fn draw_prompt(app: &App, e: &EditLayout, p: &mut Ink<'_>) {
    let lang = app.lang;
    let font = app.layout.font;
    let parts = e.prompt(lang, font);
    panel(p, e.result);
    block(
        p,
        text(lang, Text::Replace),
        Rect::new(
            parts.text.x,
            parts.text.center().y - font * 0.65,
            parts.text.w,
            parts.text.h.min(font * 2.6),
        ),
        font,
        theme::TEXT,
        Align::Left,
    );
    button(p, parts.yes, text(lang, Text::Yes), Style::Primary, font);
    button(p, parts.no, text(lang, Text::No), Style::Normal, font);
}

fn panel(p: &mut Ink<'_>, rect: Rect) {
    p.fill_rect(rect, 12.0, theme::PANEL);
    p.stroke_rect(rect.inset(0.5), 12.0, 1.0, theme::CELL_MARK);
}

fn draw_board(app: &App, editor: &Editor, p: &mut Ink<'_>) {
    let l = &app.layout;
    brackets(p, l.frame, l.cell * 0.25, theme::ACCENT);
    p.fill_rect(l.cells, 0.0, theme::GRID_LINE);
    let seam = (l.cell * 0.03).max(1.0);
    for row in 0..editor.height() {
        for col in 0..editor.width() {
            let r = l.cell_rect(row, col).inset(seam / 2.0);
            match editor.cell(row, col) {
                Cell::Free => free_cell(p, r),
                Cell::Lock(color) => lock_cell(p, r, color),
                Cell::Dead => dead_cell(p, r),
            }
        }
    }
}

/// The clue fields as plain numbers in the color of their clue, split
/// among the colors like the counters in play; the one being edited is
/// highlighted.
fn draw_clues(app: &App, editor: &Editor, p: &mut Ink<'_>) {
    let l = &app.layout;
    let selected = app.selected_clue();
    let fields = app.clue_fields();
    let several = fields.iter().any(|(c, _)| c.color != Color::DEFAULT);
    let shake = app.shaking();
    for (clue, area) in fields {
        // A field that refused a value shakes side to side, fading out.
        let area = match shake {
            Some((c, t)) if c == clue => {
                let dx = math::sin(t * core::f32::consts::TAU * 3.0) * 4.0 * (1.0 - t);
                Rect::new(area.x + dx, area.y, area.w, area.h)
            }
            _ => area,
        };
        let accent = palette(clue.color).accent;
        let value = editor.clue(clue.color, clue.row, clue.index);
        let label = value.to_string();
        let size = (area.w.min(area.h) * 0.6).min(l.font * 1.1).max(6.0);
        let mark = Rect::centered(area.center(), area.w * 0.9, area.h * 0.9);
        let radius = (mark.w.min(mark.h) * 0.25).min(4.0);
        let color = if selected == Some(clue) {
            p.fill_rect(mark, radius, accent);
            theme::BACKGROUND
        } else {
            // With several colors each field is outlined in its color.
            let outline = if several {
                accent.alpha(0.5)
            } else {
                theme::CELL_MARK
            };
            p.stroke_rect(mark.inset(0.5), radius, 1.0, outline);
            if value == 0 { theme::MUTED } else { accent }
        };
        centered_text(p, &label, area, text_font(size, true), color);
    }
}

/// The solution on view, its pieces laid out on the board.
fn draw_solution(app: &App, p: &mut Ink<'_>) {
    let (Some(solution), Some(editor)) = (app.shown_solution(), app.editor()) else {
        return;
    };
    let l = &app.layout;
    for (piece_of, at) in editor.pieces().iter().zip(&solution.placements) {
        let shape = piece_of.shape.rotated(at.rotation);
        let center = Point::new(
            l.cells.x + (to_f32(at.col) + to_f32(shape.width()) / 2.0) * l.cell,
            l.cells.y + (to_f32(at.row) + to_f32(shape.height()) / 2.0) * l.cell,
        );
        piece(p, &shape, center, l.cell, 0.0, Tint::of(piece_of.color));
    }
}

/// Four color marks; `active` is ringed.
fn color_marks(p: &mut Ink<'_>, marks: &[Rect; 4], active: Color) {
    for (color, mark) in Color::ALL.into_iter().zip(marks) {
        let look = palette(color);
        let swatch = mark.inset(mark.w * 0.22);
        p.fill_rect(swatch, 3.0, look.fill);
        p.stroke_rect(swatch.inset(0.5), 3.0, 1.0, look.bright);
        if color == active {
            p.stroke_rect(mark.inset(2.0), 6.0, 2.0, theme::TEXT);
        }
    }
}

/// Breaks `text` into lines no wider than `width` in `font`.
fn wrap(p: &mut Ink<'_>, text: &str, font: Font, width: f32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split(' ') {
        let candidate = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if line.is_empty() || p.text_width(&candidate, font) <= width {
            line = candidate;
        } else {
            lines.push(core::mem::replace(&mut line, word.to_owned()));
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// Text in `rect`, wrapped, as many lines as fit; the last one shrinks to
/// fit the width.
fn block(p: &mut Ink<'_>, text: &str, rect: Rect, size: f32, color: Rgba, align: Align) {
    let font = text_font(size, false);
    let line_h = size * 1.3;
    let room = math::floor(rect.h / line_h).clamp(1.0, 8.0);
    let lines = wrap(p, text, font, rect.w);
    let mut y = rect.y + line_h / 2.0;
    for (k, line) in lines.iter().enumerate() {
        let last = to_f32(k + 1) >= room || k + 1 == lines.len();
        let shown = if last && k + 1 < lines.len() {
            lines[k..].join(" ")
        } else {
            line.clone()
        };
        let font = fit(p, &shown, font, rect.w);
        let x = match align {
            Align::Left => rect.x,
            Align::Center => rect.center().x,
            Align::Right => rect.right(),
        };
        p.text(&shown, Point::new(x, y), font, align, color);
        if last {
            break;
        }
        y += line_h;
    }
}

/// The status line: the marks to check after reading a screenshot (a tap
/// closes them), and the puzzle's first problem.
fn draw_status(app: &App, e: &EditLayout, p: &mut Ink<'_>) {
    let font = app.layout.font * 0.9;
    let problem = app.first_problem();
    let mut area = e.status;
    if !app.doubts().is_empty() {
        let line = format!("{} ×", text(app.lang, Text::CheckDoubts));
        let lines = if problem.is_some() {
            (area.h / 2.0).max(font * 1.3)
        } else {
            area.h
        };
        block(
            p,
            &line,
            Rect::new(area.x, area.y, area.w, lines),
            font,
            theme::MARKER,
            Align::Left,
        );
        area = Rect::new(area.x, area.y + lines, area.w, (area.h - lines).max(0.0));
    }
    // A blank editor first wants its size.
    let blank = app.editor().is_some_and(Editor::is_blank);
    if blank && area.h > 0.0 {
        block(
            p,
            text(app.lang, Text::SizeFirst),
            area,
            font,
            theme::ACCENT,
            Align::Left,
        );
    } else if let Some(problem) = problem
        && area.h > 0.0
    {
        let colored = app.editor().is_some_and(|ed| ed.colors().len() > 1);
        block(
            p,
            &problem_line(app.lang, &problem, colored),
            area,
            font,
            theme::ERROR,
            Align::Left,
        );
    }
}

fn draw_tray(app: &App, editor: &Editor, p: &mut Ink<'_>) {
    let l = &app.layout;
    p.fill_rect(l.tray, 12.0, theme::TRAY);
    let cards = app.editor_cards();
    draw_pager(app, cards, p);
    for i in 0..cards {
        let Some(card) = app.card_of(i) else {
            continue;
        };
        p.fill_rect(card, 8.0, theme::CARD);
        if let Some(shown) = editor.pieces().get(i) {
            piece(
                p,
                &shown.shape,
                card.center(),
                l.tray_cell,
                0.0,
                Tint::of(shown.color),
            );
        } else {
            p.stroke_rect(card.inset(3.5), 6.0, 1.0, theme::ACCENT.alpha(0.6));
            centered_text(
                p,
                "+",
                card,
                text_font((card.h * 0.5).min(32.0), true),
                theme::ACCENT,
            );
        }
    }
}

fn draw_answer(app: &App, e: &EditLayout, p: &mut Ink<'_>) {
    let Some(state) = app.solve_state() else {
        return;
    };
    let lang = app.lang;
    let font = app.layout.font;
    let parts = e.result_panel(lang, font);
    panel(p, e.result);
    let (solutions, view) = app.solutions();
    let found = solutions.len();
    let which = |more: bool| solution_of(lang, view + 1, found, more);
    let (title, detail, action) = match state {
        SolveState::Running => (
            text(lang, Text::Solving).to_owned(),
            None,
            Some(Text::Cancel),
        ),
        SolveState::Stopped => (
            text(lang, Text::Stopped).to_owned(),
            (found > 0).then(|| which(true)),
            Some(Text::Continue),
        ),
        SolveState::Finished(_) if found == 0 => (
            text(lang, Text::NoSolution).to_owned(),
            app.first_problem().map(|pr| problem_text(lang, &pr)),
            Some(Text::Done),
        ),
        SolveState::Finished(stop) => (
            which(stop == endlogic_core::solver::StopReason::SolutionLimit),
            None,
            Some(Text::Done),
        ),
    };
    let title_font = fit(p, &title, text_font(font, true), parts.title.w);
    p.text(
        &title,
        Point::new(parts.title.x, parts.title.center().y),
        title_font,
        Align::Left,
        theme::TEXT,
    );
    if let Some(detail) = detail {
        let detail_font = fit(p, &detail, text_font(font * 0.85, false), parts.detail.w);
        p.text(
            &detail,
            Point::new(parts.detail.x, parts.detail.center().y),
            detail_font,
            Align::Left,
            theme::MUTED,
        );
    }
    if found > 1 {
        button(p, parts.prev, "‹", Style::Normal, font * 1.2);
        button(p, parts.next, "›", Style::Normal, font * 1.2);
    }
    if let Some(action) = action {
        button(p, parts.action, text(lang, action), Style::Normal, font);
    }
}

fn stepper(p: &mut Ink<'_>, s: &Stepper, label: &str, value: usize, font: f32) {
    if s.label.w > 0.0 {
        let f = fit(p, label, text_font(font * 0.9, false), s.label.w - 4.0);
        p.text(
            label,
            Point::new(s.label.x, s.label.center().y),
            f,
            Align::Left,
            theme::MUTED,
        );
    }
    let minus = if value > 1 {
        Style::Normal
    } else {
        Style::Disabled
    };
    let plus = if value < endlogic_core::MAX_GRID_SIZE {
        Style::Normal
    } else {
        Style::Disabled
    };
    button(p, s.minus, "−", minus, font * 1.2);
    centered_text(
        p,
        &value.to_string(),
        s.value.inset(-4.0),
        text_font(font, true),
        theme::TEXT,
    );
    button(p, s.plus, "+", plus, font * 1.2);
}

fn draw_controls(app: &App, e: &EditLayout, editor: &Editor, p: &mut Ink<'_>) {
    let lang = app.lang;
    let font = app.layout.font;
    if !e.palette.iter().any(|&m| app.under_overlay(m)) {
        color_marks(p, &e.palette, app.active_color());
    }
    stepper(p, &e.width, text(lang, Text::Width), editor.width(), font);
    stepper(
        p,
        &e.height,
        text(lang, Text::Height),
        editor.height(),
        font,
    );
    if let Some(times) = e.times {
        centered_text(
            p,
            "×",
            times.inset(-4.0),
            text_font(font, false),
            theme::MUTED,
        );
    }
    let has_pieces = !editor.pieces().is_empty();
    let enabled = |on: bool| if on { Style::Normal } else { Style::Disabled };
    let running = app.solve_state() == Some(SolveState::Running);
    let clear = if app.clear_armed() {
        Text::Sure
    } else {
        Text::Clear
    };
    if !app.under_overlay(e.import) {
        tight_button(p, e.import, text(lang, Text::Import), font);
    }
    for (rect, label, style) in [
        (e.clear, clear, Style::Normal),
        (e.play, Text::Play, enabled(has_pieces)),
        (
            e.solve,
            Text::Solve,
            if has_pieces && !running {
                Style::Primary
            } else {
                Style::Disabled
            },
        ),
    ] {
        if !app.under_overlay(rect) {
            button(p, rect, text(lang, label), style, font);
        }
    }
}

/// A button whose label shrinks a little, or takes two lines, where the
/// button had to give up room.
fn tight_button(p: &mut Ink<'_>, rect: Rect, label: &str, font: f32) {
    let size = fit(p, label, text_font(font, true), rect.w - font * 0.8).size;
    if size >= font * 0.85 || !label.contains(' ') {
        button(p, rect, label, Style::Normal, size);
        return;
    }
    button(p, rect, "", Style::Normal, font);
    let (first, second) = super::halves(label);
    let base = text_font((rect.h * 0.32).min(font), true);
    let room = rect.w - font * 0.6;
    let shrunk = fit(p, first, fit(p, second, base, room), room);
    let dy = shrunk.size * 0.6;
    let mid = rect.center();
    let at = |y: f32| Point::new(mid.x, y);
    p.text(first, at(mid.y - dy), shrunk, Align::Center, theme::TEXT);
    p.text(second, at(mid.y + dy), shrunk, Align::Center, theme::TEXT);
}

fn draw_piece_editor(app: &App, e: &EditLayout, draft: &Draft, p: &mut Ink<'_>) {
    let lang = app.lang;
    let font = app.layout.font;
    let parts = e.piece_editor(draft.side);
    panel(p, e.piece);
    color_marks(p, &parts.colors, draft.color);
    let look = palette(draft.color);
    p.fill_rect(parts.grid, 0.0, theme::GRID_LINE);
    for (k, &on) in draft.cells.iter().enumerate() {
        let (r, c) = (k / draft.side, k % draft.side);
        let cell = Rect::new(
            parts.grid.x + to_f32(c) * parts.cell,
            parts.grid.y + to_f32(r) * parts.cell,
            parts.cell,
            parts.cell,
        )
        .inset(1.0);
        if on {
            p.fill_rect(cell, 2.0, look.fill);
            p.stroke_rect(cell.inset(0.5), 2.0, 1.0, look.bright);
        } else {
            p.fill_rect(cell, 2.0, theme::CELL);
        }
    }
    let shape = draft.shape();
    if let Err(err) = shape {
        let reason = match err {
            DraftError::Empty => Text::EmptyPiece,
            DraftError::Disconnected => Text::DisconnectedPiece,
        };
        block(
            p,
            text(lang, reason),
            parts.reason,
            font * 0.9,
            theme::ERROR,
            Align::Left,
        );
    }
    let valid = shape.is_ok();
    let done = if valid {
        Style::Primary
    } else {
        Style::Disabled
    };
    button(
        p,
        parts.cancel,
        text(lang, Text::Cancel),
        Style::Normal,
        font,
    );
    button(p, parts.done, text(lang, Text::Done), done, font);
    if draft.index.is_some() {
        let room = app
            .editor()
            .is_some_and(|ed| ed.pieces().len() < endlogic_core::MAX_PIECES);
        let duplicate = if valid && room {
            Style::Normal
        } else {
            Style::Disabled
        };
        button(
            p,
            parts.duplicate,
            text(lang, Text::Duplicate),
            duplicate,
            font,
        );
        button(
            p,
            parts.delete,
            text(lang, Text::Delete),
            Style::Normal,
            font,
        );
    }
}

fn draw_strip(app: &App, e: &EditLayout, editor: &Editor, clue: ClueRef, p: &mut Ink<'_>) {
    let font = app.layout.font;
    let len = if clue.row {
        editor.width()
    } else {
        editor.height()
    };
    let keys = e.digit_keys(len + 1);
    panel(p, e.strip);
    let current = editor.clue(clue.color, clue.row, clue.index);
    for (k, &key) in keys.iter().enumerate() {
        if k <= len {
            let style = if k == current {
                Style::Primary
            } else {
                Style::Normal
            };
            button(p, key, &k.to_string(), style, font);
        } else {
            button(p, key, text(app.lang, Text::Done), Style::Normal, font);
        }
    }
}
