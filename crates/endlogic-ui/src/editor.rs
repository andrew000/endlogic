//! The custom tab: an editor for puzzles copied from the game, a solver
//! that lists their solutions, and a way to play them.

use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;

use endlogic_core::code::encode;
use endlogic_core::diagnose::{Problem, diagnose};
use endlogic_core::solver::{Progress, Search, StopReason};
use endlogic_core::{
    Cell, Color, Grid, LineClues, MAX_COLORS, MAX_GRID_SIZE, MAX_PIECES, Piece, Puzzle, Shape,
    Solution,
};

use crate::app::{App, Control, Gesture};
use crate::layout::{EditLayout, Layout, Mode, to_f32};
use crate::painter::Rect;
use crate::shell::{CodeKind, Effect, SETTING_EDITOR};
use crate::theme;

/// Side of the piece editor's grid for a new piece.
pub const DRAFT_SIDE: usize = 6;
/// Most solutions the solver lists.
pub const SOLUTION_LIMIT: usize = 50;

/// A puzzle being edited: any size, any clues of any of the four colors,
/// any pieces.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Editor {
    width: usize,
    height: usize,
    cells: Vec<Cell>,
    /// Row clues of each color, `rows[color][row]`.
    rows: [Vec<usize>; MAX_COLORS],
    /// Column clues of each color.
    cols: [Vec<usize>; MAX_COLORS],
    pieces: Vec<Piece>,
}

impl Default for Editor {
    fn default() -> Editor {
        Editor::empty(5, 5)
    }
}

impl Editor {
    /// An empty board of `width` × `height`: free cells, zero clues, no
    /// pieces.
    #[must_use]
    pub fn empty(width: usize, height: usize) -> Editor {
        Editor {
            width,
            height,
            cells: vec![Cell::Free; width * height],
            rows: core::array::from_fn(|_| vec![0; height]),
            cols: core::array::from_fn(|_| vec![0; width]),
            pieces: Vec::new(),
        }
    }

    /// The editor holding `puzzle`: its cells, every color's clues and
    /// every piece.
    #[must_use]
    pub fn from_puzzle(puzzle: &Puzzle) -> Editor {
        let grid = puzzle.grid();
        Editor {
            width: grid.width(),
            height: grid.height(),
            cells: grid.cells().to_vec(),
            rows: core::array::from_fn(|k| {
                (0..grid.height())
                    .map(|r| puzzle.row_clue(Color::ALL[k], r))
                    .collect()
            }),
            cols: core::array::from_fn(|k| {
                (0..grid.width())
                    .map(|c| puzzle.col_clue(Color::ALL[k], c))
                    .collect()
            }),
            pieces: puzzle.pieces().to_vec(),
        }
    }

    /// The puzzle as it stands. It has clues for every color in use (see
    /// [`colors`](Editor::colors)), or for the first color when none is.
    ///
    /// # Panics
    ///
    /// Never: the editor keeps its size, cells and clues consistent.
    #[must_use]
    pub fn puzzle(&self) -> Puzzle {
        let grid = Grid::from_cells(self.width, self.height, self.cells.clone())
            .expect("the size stays within 1..=16");
        let mut colors = self.colors();
        if colors.is_empty() {
            colors.push(Color::DEFAULT);
        }
        let clues: BTreeMap<Color, LineClues> = colors
            .into_iter()
            .map(|color| {
                let k = color.index();
                let clues = LineClues {
                    rows: self.rows[k].clone(),
                    cols: self.cols[k].clone(),
                };
                (color, clues)
            })
            .collect();
        Puzzle::new(grid, self.pieces.clone(), clues).expect("clues match the grid")
    }

    /// The colors in use: those with a piece, a lock or a clue above 0, in
    /// order.
    #[must_use]
    pub fn colors(&self) -> Vec<Color> {
        Color::ALL
            .into_iter()
            .filter(|&color| {
                let k = color.index();
                self.pieces.iter().any(|p| p.color == color)
                    || self.cells.contains(&Cell::Lock(color))
                    || self.rows[k].iter().chain(&self.cols[k]).any(|&v| v > 0)
            })
            .collect()
    }

    /// Columns.
    #[must_use]
    pub fn width(&self) -> usize {
        self.width
    }

    /// Rows.
    #[must_use]
    pub fn height(&self) -> usize {
        self.height
    }

    /// The cell at `(row, col)`.
    #[must_use]
    pub fn cell(&self, row: usize, col: usize) -> Cell {
        self.cells[row * self.width + col]
    }

    /// The clue of `color` for row `index` (`row`) or column `index`.
    #[must_use]
    pub fn clue(&self, color: Color, row: bool, index: usize) -> usize {
        let k = color.index().min(MAX_COLORS - 1);
        if row {
            self.rows[k][index]
        } else {
            self.cols[k][index]
        }
    }

    /// The pieces.
    #[must_use]
    pub fn pieces(&self) -> &[Piece] {
        &self.pieces
    }

    /// Changes the size, keeping the cells and clues that stay inside.
    /// Clues longer than their new line are cut to it.
    pub fn resize(&mut self, width: usize, height: usize) {
        let width = width.clamp(1, MAX_GRID_SIZE);
        let height = height.clamp(1, MAX_GRID_SIZE);
        let mut cells = vec![Cell::Free; width * height];
        for r in 0..height.min(self.height) {
            for c in 0..width.min(self.width) {
                cells[r * width + c] = self.cell(r, c);
            }
        }
        self.cells = cells;
        for rows in &mut self.rows {
            rows.resize(height, 0);
            for clue in rows.iter_mut() {
                *clue = (*clue).min(width);
            }
        }
        for cols in &mut self.cols {
            cols.resize(width, 0);
            for clue in cols.iter_mut() {
                *clue = (*clue).min(height);
            }
        }
        self.width = width;
        self.height = height;
    }

    /// Turns a cell free → lock of `active` → dead → free; a lock of
    /// another color becomes a lock of `active` at once.
    pub fn cycle(&mut self, row: usize, col: usize, active: Color) {
        let cell = &mut self.cells[row * self.width + col];
        *cell = match *cell {
            Cell::Free => Cell::Lock(active),
            Cell::Lock(color) if color != active => Cell::Lock(active),
            Cell::Lock(_) => Cell::Dead,
            Cell::Dead => Cell::Free,
        };
    }

    /// Sets a clue of `color`, cut to the length of its line.
    pub fn set_clue(&mut self, color: Color, row: bool, index: usize, value: usize) {
        let k = color.index().min(MAX_COLORS - 1);
        if row {
            self.rows[k][index] = value.min(self.width);
        } else {
            self.cols[k][index] = value.min(self.height);
        }
    }

    /// The longest side of any piece, at least 1.
    #[must_use]
    pub fn max_piece_side(&self) -> usize {
        self.pieces
            .iter()
            .map(|p| p.shape.width().max(p.shape.height()))
            .max()
            .unwrap_or(1)
    }
}

/// The clue field of one color for a row (`row`) or a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClueRef {
    /// The color.
    pub color: Color,
    /// A row counter, else a column counter.
    pub row: bool,
    /// The line, from 0.
    pub index: usize,
}

/// A piece in the piece editor.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Draft {
    /// Cells per side of the grid.
    pub side: usize,
    /// Row-major, `side * side`.
    pub cells: Vec<bool>,
    /// The piece being changed, or `None` for a new one.
    pub index: Option<usize>,
    /// The piece's color.
    pub color: Color,
}

/// Why a drawn piece cannot be accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DraftError {
    /// No cell.
    Empty,
    /// Cells not joined by sides.
    Disconnected,
}

impl Draft {
    fn new(shape: Option<&Shape>, index: Option<usize>, color: Color) -> Draft {
        let side = shape
            .map_or(0, |s| s.width().max(s.height()))
            .max(DRAFT_SIDE);
        let mut cells = vec![false; side * side];
        if let Some(shape) = shape {
            for &(r, c) in shape.cells() {
                cells[r * side + c] = true;
            }
        }
        Draft {
            side,
            cells,
            index,
            color,
        }
    }

    /// The piece drawn, or why there is none.
    ///
    /// # Errors
    ///
    /// [`DraftError`] when the cells are empty or not connected.
    pub fn shape(&self) -> Result<Shape, DraftError> {
        let cells = self
            .cells
            .iter()
            .enumerate()
            .filter(|(_, on)| **on)
            .map(|(k, _)| (k / self.side, k % self.side));
        Shape::new(cells).map_err(|err| match err {
            endlogic_core::ModelError::EmptyShape => DraftError::Empty,
            _ => DraftError::Disconnected,
        })
    }
}

/// How the solver of the custom tab stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SolveState {
    /// Searching.
    Running,
    /// Done: every solution is listed, or the limit was reached.
    Finished(StopReason),
    /// The budget ran out; it can continue.
    Stopped,
}

/// The solver of the custom tab.
#[derive(Debug, Clone)]
pub(crate) struct Solver {
    search: Search,
    /// Node count at which the solver stops and offers to continue.
    limit: u64,
    pub state: SolveState,
    /// The solution on the board.
    pub view: usize,
}

impl Solver {
    pub fn solutions(&self) -> &[Solution] {
        self.search.solutions()
    }
}

/// The editor's state around the puzzle.
#[derive(Debug, Clone, Default)]
pub(crate) struct EditState {
    /// The counter whose digit strip is open.
    pub selected: Option<ClueRef>,
    /// The piece editor, when open.
    pub draft: Option<Draft>,
    /// Seconds since the clear button asked for a second tap.
    pub clear_armed: Option<f32>,
    pub solver: Option<Solver>,
    /// The color new locks and pieces get.
    pub active: Color,
}

/// An editor action from a control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditControl {
    /// − (`false`) or + on the width (`width`) or height stepper.
    Size {
        /// The width stepper, else the height stepper.
        width: bool,
        /// +, else −.
        plus: bool,
    },
    /// A mark of the active color row.
    Active(Color),
    /// A board cell.
    Cell(usize, usize),
    /// A counter.
    Clue(ClueRef),
    /// A key of the digit strip.
    Digit(usize),
    /// "Done" on the digit strip.
    DigitsDone,
    /// Closes the digit strip (a tap anywhere else).
    Dismiss,
    /// A piece card.
    Card(usize),
    /// The card that adds a piece.
    AddPiece,
    /// A cell of the piece editor.
    DraftCell(usize),
    /// A color mark of the piece editor.
    DraftColor(Color),
    /// Accept the drawn piece.
    DraftDone,
    /// Leave the piece editor.
    DraftCancel,
    /// Accept and add a copy.
    DraftDuplicate,
    /// Remove the piece.
    DraftDelete,
    /// Clear the editor (twice).
    Clear,
    /// Play the puzzle.
    Play,
    /// Run the solver.
    Solve,
    /// Stop the solver, or close its answer.
    SolveCancel,
    /// Give the solver more nodes.
    SolveContinue,
    /// Previous solution.
    SolutionPrev,
    /// Next solution.
    SolutionNext,
}

impl EditControl {
    /// Controls on a grid of cells, hit exactly rather than by touch zone.
    pub(crate) fn exact(self) -> bool {
        matches!(
            self,
            EditControl::Cell(..)
                | EditControl::Clue(_)
                | EditControl::DraftCell(_)
                | EditControl::Dismiss
        )
    }
}

impl App {
    /// The puzzle in the editor, while the custom tab is editing.
    #[must_use]
    pub fn editor(&self) -> Option<&Editor> {
        self.is_editing().then_some(&self.editor)
    }

    /// Whether the custom tab is editing.
    #[must_use]
    pub fn is_editing(&self) -> bool {
        self.custom && self.game.is_none()
    }

    /// Whether the custom tab is open, editing or playing.
    #[must_use]
    pub fn is_custom(&self) -> bool {
        self.custom
    }

    /// The counter whose digit strip is open.
    #[must_use]
    pub fn selected_clue(&self) -> Option<ClueRef> {
        self.edit.selected
    }

    /// The piece in the piece editor, when it is open.
    #[must_use]
    pub fn draft(&self) -> Option<&Draft> {
        self.edit.draft.as_ref()
    }

    /// Whether the clear button waits for its second tap.
    #[must_use]
    pub fn clear_armed(&self) -> bool {
        self.edit.clear_armed.is_some()
    }

    /// The solver's state, if it ran since the last change.
    #[must_use]
    pub fn solve_state(&self) -> Option<SolveState> {
        self.edit.solver.as_ref().map(|s| s.state)
    }

    /// The solutions found, and which one is on the board.
    #[must_use]
    pub fn solutions(&self) -> (&[Solution], usize) {
        self.edit
            .solver
            .as_ref()
            .map_or((&[], 0), |s| (s.solutions(), s.view))
    }

    /// The solution on the board, if any.
    #[must_use]
    pub fn shown_solution(&self) -> Option<&Solution> {
        let (solutions, view) = self.solutions();
        solutions.get(view)
    }

    /// The first structural problem of the puzzle in the editor.
    #[must_use]
    pub fn first_problem(&self) -> Option<Problem> {
        diagnose(&self.editor.puzzle()).into_iter().next()
    }

    /// The color new locks and pieces get in the editor.
    #[must_use]
    pub fn active_color(&self) -> Color {
        self.edit.active
    }

    /// The colors that have clue fields: the first color, those in use and
    /// the active one, in order.
    #[must_use]
    pub fn clue_colors(&self) -> Vec<Color> {
        let used = self.editor.colors();
        Color::ALL
            .into_iter()
            .filter(|&c| c == Color::DEFAULT || used.contains(&c) || c == self.edit.active)
            .collect()
    }

    /// Cards in the editor's tray: the pieces, then the one that adds a
    /// piece unless there are already [`MAX_PIECES`].
    pub(crate) fn editor_cards(&self) -> usize {
        let n = self.editor.pieces.len();
        if n < MAX_PIECES { n + 1 } else { n }
    }

    /// Opens the custom tab's editor with `editor`.
    pub(crate) fn open_editor(&mut self, editor: Editor) {
        if let Gesture::Dragging(drag) = self.gesture {
            self.cancel_drag(drag);
        }
        self.gesture = Gesture::Idle;
        self.custom = true;
        self.game = None;
        self.generator = None;
        self.editor = editor;
        self.edit = EditState::default();
        self.tray_page = 0;
        self.publish_editor();
        self.relayout();
    }

    /// Shows the editor's puzzle in the address and remembers it.
    fn publish_editor(&mut self) {
        if let Ok(code) = encode(&self.editor.puzzle()) {
            self.effects
                .push(Effect::SetCode(CodeKind::Edit, code.clone()));
            self.effects.push(Effect::SaveSetting {
                key: SETTING_EDITOR,
                value: code,
            });
        }
    }

    /// After a change to the puzzle: the solver's answer goes.
    fn edited(&mut self) {
        self.edit.solver = None;
        self.edit.clear_armed = None;
        self.publish_editor();
        self.relayout();
    }

    /// The layout input of the editor.
    pub(crate) fn edit_mode(&self) -> Mode {
        if self.is_editing() {
            Mode::Edit
        } else {
            Mode::Play
        }
    }

    /// Runs the solver for one frame and counts down the clear button.
    pub(crate) fn step_editor(&mut self, dt: f32) {
        if let Some(armed) = &mut self.edit.clear_armed {
            *armed += dt;
            if *armed >= theme::CLEAR_TIME {
                self.edit.clear_armed = None;
            }
        }
        let per_frame = self.frame_budget();
        let Some(solver) = &mut self.edit.solver else {
            return;
        };
        if solver.state != SolveState::Running {
            return;
        }
        let allowed = per_frame.min(solver.limit.saturating_sub(solver.search.nodes()));
        match solver.search.step(allowed) {
            Progress::Done(result) => solver.state = SolveState::Finished(result.stop),
            Progress::Working if solver.search.nodes() >= solver.limit => {
                solver.state = SolveState::Stopped;
            }
            Progress::Working => {}
        }
    }

    /// Whether the editor needs frames: the solver runs or the clear button
    /// counts down.
    pub(crate) fn editor_busy(&self) -> bool {
        self.edit.clear_armed.is_some()
            || self
                .edit
                .solver
                .as_ref()
                .is_some_and(|s| s.state == SolveState::Running)
    }

    /// The editor's controls in hit-test order, with whether each is
    /// enabled. The header and tabs come first from the caller.
    pub(crate) fn edit_controls(&self) -> Vec<(Control, Rect, bool)> {
        let l = &self.layout;
        let Some(e) = l.edit else {
            return Vec::new();
        };
        let ed = |c: EditControl| Control::Edit(c);
        let mut controls = Vec::new();
        if let Some(draft) = &self.edit.draft {
            return self.draft_controls(&e, draft);
        }
        if let Some(selected) = self.edit.selected {
            let digits = self.line_len(selected) + 1;
            let keys = e.digit_keys(digits);
            for (k, &key) in keys.iter().enumerate() {
                let control = if k < digits {
                    EditControl::Digit(k)
                } else {
                    EditControl::DigitsDone
                };
                controls.push((ed(control), key, true));
            }
            controls.extend(self.clue_controls());
            let screen = Rect::new(0.0, 0.0, self.width, self.height);
            controls.push((ed(EditControl::Dismiss), screen, true));
            return controls;
        }
        let ed_size = |width: bool, plus: bool| ed(EditControl::Size { width, plus });
        let size = (self.editor.width, self.editor.height);
        for (color, mark) in Color::ALL.into_iter().zip(e.palette) {
            controls.push((ed(EditControl::Active(color)), mark, true));
        }
        controls.extend([
            (ed_size(true, false), e.width.minus, size.0 > 1),
            (ed_size(true, true), e.width.plus, size.0 < MAX_GRID_SIZE),
            (ed_size(false, false), e.height.minus, size.1 > 1),
            (ed_size(false, true), e.height.plus, size.1 < MAX_GRID_SIZE),
        ]);
        let has_pieces = !self.editor.pieces.is_empty();
        let running = self.solve_state() == Some(SolveState::Running);
        controls.extend([
            (ed(EditControl::Clear), e.clear, true),
            (ed(EditControl::Play), e.play, has_pieces),
            (ed(EditControl::Solve), e.solve, has_pieces && !running),
        ]);
        if let Some(solver) = &self.edit.solver {
            let panel = e.result_panel(self.lang, l.font);
            let many = solver.solutions().len() > 1;
            match solver.state {
                // "Done" on a finished answer puts the tray back.
                SolveState::Running | SolveState::Finished(_) => {
                    controls.push((ed(EditControl::SolveCancel), panel.action, true));
                }
                SolveState::Stopped => {
                    controls.push((ed(EditControl::SolveContinue), panel.action, true));
                }
            }
            controls.extend([
                (ed(EditControl::SolutionPrev), panel.prev, many),
                (ed(EditControl::SolutionNext), panel.next, many),
            ]);
            // The answer takes the tray's place, except in the wide
            // arrangement, where it sits under the board.
            if e.result.intersects(l.tray) {
                controls.extend(self.cell_controls());
                controls.extend(self.clue_controls());
                return controls;
            }
        }
        for i in 0..self.editor_cards() {
            if let Some(card) = self.card_of(i) {
                let control = if i < self.editor.pieces.len() {
                    EditControl::Card(i)
                } else {
                    EditControl::AddPiece
                };
                controls.push((ed(control), card, true));
            }
        }
        controls.extend(self.cell_controls());
        controls.extend(self.clue_controls());
        controls
    }

    /// The piece editor's controls.
    fn draft_controls(&self, e: &EditLayout, draft: &Draft) -> Vec<(Control, Rect, bool)> {
        let ed = |c: EditControl| Control::Edit(c);
        let mut controls = Vec::new();
        let parts = e.piece_editor(draft.side);
        for k in 0..draft.side * draft.side {
            let (r, c) = (k / draft.side, k % draft.side);
            let cell = Rect::new(
                parts.grid.x + crate::layout::to_f32(c) * parts.cell,
                parts.grid.y + crate::layout::to_f32(r) * parts.cell,
                parts.cell,
                parts.cell,
            );
            controls.push((ed(EditControl::DraftCell(k)), cell, true));
        }
        for (color, mark) in Color::ALL.into_iter().zip(parts.colors) {
            controls.push((ed(EditControl::DraftColor(color)), mark, true));
        }
        let valid = draft.shape().is_ok();
        let existing = draft.index.is_some();
        let room = self.editor.pieces.len() < MAX_PIECES;
        controls.extend([
            (ed(EditControl::DraftDone), parts.done, valid),
            (ed(EditControl::DraftCancel), parts.cancel, true),
            (
                ed(EditControl::DraftDuplicate),
                parts.duplicate,
                existing && valid && room,
            ),
            (ed(EditControl::DraftDelete), parts.delete, existing),
        ]);
        controls
    }

    fn cell_controls(&self) -> Vec<(Control, Rect, bool)> {
        let l = &self.layout;
        (0..self.editor.height)
            .flat_map(|r| (0..self.editor.width).map(move |c| (r, c)))
            .map(|(r, c)| {
                (
                    Control::Edit(EditControl::Cell(r, c)),
                    l.cell_rect(r, c),
                    true,
                )
            })
            .collect()
    }

    /// The clue fields: the band part beside each row and above each column,
    /// split among the colors that have fields like the counters in play
    /// (for a row the first color at the bottom, for a column on the left).
    #[must_use]
    pub fn clue_fields(&self) -> Vec<(ClueRef, Rect)> {
        let layout = &self.layout;
        let colors = self.clue_colors();
        let share = to_f32(colors.len().max(1));
        let mut fields = Vec::new();
        for (slot, &color) in colors.iter().enumerate() {
            let slot = to_f32(slot);
            for index in 0..self.editor.height {
                let cell = layout.cell_rect(index, 0);
                let h = cell.h / share;
                let y = cell.y + (share - 1.0 - slot) * h;
                let band = layout.row_counters;
                let clue = ClueRef {
                    color,
                    row: true,
                    index,
                };
                fields.push((clue, Rect::new(band.x, y, band.w, h)));
            }
            for index in 0..self.editor.width {
                let cell = layout.cell_rect(0, index);
                let w = cell.w / share;
                let band = layout.col_counters;
                let clue = ClueRef {
                    color,
                    row: false,
                    index,
                };
                fields.push((clue, Rect::new(cell.x + slot * w, band.y, w, band.h)));
            }
        }
        fields
    }

    /// The clue fields as controls.
    pub(crate) fn clue_controls(&self) -> Vec<(Control, Rect, bool)> {
        self.clue_fields()
            .into_iter()
            .map(|(clue, area)| (Control::Edit(EditControl::Clue(clue)), area, true))
            .collect()
    }

    /// Cells in the line of `clue`: the largest value it may take.
    fn line_len(&self, clue: ClueRef) -> usize {
        if clue.row {
            self.editor.width
        } else {
            self.editor.height
        }
    }

    /// Carries out an editor control.
    pub(crate) fn edit_action(&mut self, control: EditControl) {
        match control {
            EditControl::Size { width, plus } => {
                let (w, h) = (self.editor.width, self.editor.height);
                let step = |n: usize| if plus { n + 1 } else { n.saturating_sub(1) };
                if width {
                    self.editor.resize(step(w), h);
                } else {
                    self.editor.resize(w, step(h));
                }
                self.edited();
            }
            EditControl::Active(color) => self.edit.active = color,
            EditControl::Cell(r, c) => {
                self.editor.cycle(r, c, self.edit.active);
                self.edited();
            }
            EditControl::Clue(clue) => self.edit.selected = Some(clue),
            EditControl::Digit(value) => {
                let Some(clue) = self.edit.selected else {
                    return;
                };
                self.editor
                    .set_clue(clue.color, clue.row, clue.index, value);
                self.edit.selected = Some(self.next_clue(clue));
                self.edited();
            }
            EditControl::DigitsDone | EditControl::Dismiss => self.edit.selected = None,
            EditControl::Card(i) => {
                let piece = self.editor.pieces.get(i).cloned();
                let color = piece.as_ref().map_or(self.edit.active, |p| p.color);
                let shape = piece.map(|p| p.shape);
                self.edit.draft = Some(Draft::new(shape.as_ref(), Some(i), color));
            }
            EditControl::AddPiece => {
                self.edit.draft = Some(Draft::new(None, None, self.edit.active));
            }
            EditControl::DraftCell(_)
            | EditControl::DraftColor(_)
            | EditControl::DraftDone
            | EditControl::DraftDuplicate
            | EditControl::DraftCancel
            | EditControl::DraftDelete => self.draft_action(control),
            EditControl::Clear => {
                if self.edit.clear_armed.is_some() {
                    self.editor = Editor::empty(self.editor.width, self.editor.height);
                    self.edit.active = Color::DEFAULT;
                    self.tray_page = 0;
                    self.edited();
                } else {
                    self.edit.clear_armed = Some(0.0);
                }
            }
            EditControl::Play => {
                let puzzle = self.editor.puzzle();
                if !puzzle.pieces().is_empty() {
                    self.edit = EditState::default();
                    self.install(puzzle, 'C');
                }
            }
            EditControl::Solve => {
                self.edit.selected = None;
                self.edit.solver = Some(Solver {
                    search: Search::new(&self.editor.puzzle(), SOLUTION_LIMIT),
                    limit: self.budgets.solve,
                    state: SolveState::Running,
                    view: 0,
                });
            }
            EditControl::SolveCancel => self.edit.solver = None,
            EditControl::SolveContinue => {
                let budget = self.budgets.solve;
                if let Some(solver) = &mut self.edit.solver {
                    solver.limit = solver.search.nodes() + budget;
                    solver.state = SolveState::Running;
                }
            }
            EditControl::SolutionPrev | EditControl::SolutionNext => {
                if let Some(solver) = &mut self.edit.solver {
                    let n = solver.solutions().len().max(1);
                    solver.view = if control == EditControl::SolutionNext {
                        (solver.view + 1) % n
                    } else {
                        (solver.view + n - 1) % n
                    };
                }
            }
        }
        if !matches!(control, EditControl::Clear) {
            self.edit.clear_armed = None;
        }
    }

    /// Carries out a control of the piece editor.
    fn draft_action(&mut self, control: EditControl) {
        match control {
            EditControl::DraftCell(k) => {
                if let Some(cell) = self.edit.draft.as_mut().and_then(|d| d.cells.get_mut(k)) {
                    *cell = !*cell;
                }
            }
            EditControl::DraftColor(color) => {
                if let Some(draft) = &mut self.edit.draft {
                    draft.color = color;
                }
            }
            EditControl::DraftDone | EditControl::DraftDuplicate => {
                let Some(draft) = self.edit.draft.take() else {
                    return;
                };
                let Ok(shape) = draft.shape() else {
                    self.edit.draft = Some(draft);
                    return;
                };
                let piece = Piece::new(draft.color, shape);
                match draft.index {
                    Some(i) => self.editor.pieces[i] = piece.clone(),
                    None => self.editor.pieces.push(piece.clone()),
                }
                if control == EditControl::DraftDuplicate && self.editor.pieces.len() < MAX_PIECES {
                    self.editor.pieces.push(piece);
                }
                self.edited();
                let last = self.editor.pieces.len().saturating_sub(1);
                self.tray_page = draft.index.unwrap_or(last) / self.layout.per_page.max(1);
            }
            EditControl::DraftCancel => self.edit.draft = None,
            EditControl::DraftDelete => {
                if let Some(i) = self.edit.draft.take().and_then(|d| d.index) {
                    self.editor.pieces.remove(i);
                    self.edited();
                }
            }
            _ => {}
        }
    }

    /// The clue field after `clue`: the rows of its color top to bottom,
    /// then its columns left to right, then the next color with fields,
    /// and after the last color the first row of the first again.
    fn next_clue(&self, clue: ClueRef) -> ClueRef {
        let (width, height) = (self.editor.width, self.editor.height);
        let colors = self.clue_colors();
        let at = |color: Color, row: bool, index: usize| ClueRef { color, row, index };
        match (clue.row, clue.index + 1) {
            (true, i) if i < height => return at(clue.color, true, i),
            (true, _) => return at(clue.color, false, 0),
            (false, i) if i < width => return at(clue.color, false, i),
            (false, _) => {}
        }
        // The next color with fields; the first if `clue`'s color was the
        // last or no longer has fields.
        let next = colors
            .iter()
            .copied()
            .find(|&c| c > clue.color)
            .or_else(|| colors.first().copied())
            .unwrap_or(Color::DEFAULT);
        at(next, true, 0)
    }

    /// Leaves play in the custom tab for the editor, with the same puzzle.
    pub(crate) fn back_to_editor(&mut self) {
        let Some(game) = &self.game else {
            return;
        };
        let editor = Editor::from_puzzle(game.board.puzzle());
        self.open_editor(editor);
    }

    /// Whether `rect` is under an open overlay of the editor, where the
    /// controls below do not work and are not drawn.
    #[must_use]
    pub fn under_overlay(&self, rect: Rect) -> bool {
        let Some(e) = self.layout.edit else {
            return false;
        };
        let area = if self.edit.draft.is_some() {
            e.piece
        } else if self.edit.selected.is_some() {
            e.strip
        } else {
            return false;
        };
        area.intersects(rect)
    }

    /// The layout's touch zone of a control: exact for grid cells and the
    /// dismiss area, grown to a full target for buttons.
    pub(crate) fn zone_of(control: Control, rect: Rect) -> Rect {
        match control {
            Control::Edit(c) if c.exact() => rect,
            _ => Layout::zone(rect),
        }
    }
}
