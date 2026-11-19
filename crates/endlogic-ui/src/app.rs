//! The trainer: game state, input handling and animation.

use endlogic_core::Puzzle;
use endlogic_core::code::{decode, encode};
use endlogic_core::generator::{Difficulty, Generator, Step};
use endlogic_core::rng::Rng;

use crate::board::{Board, PieceState, Turned};
use crate::draw;
use crate::editor::{EditControl, EditState, Editor};
use crate::hint::Hints;
use crate::layout::{Layout, LayoutInput, Mode, to_f32};
use crate::painter::{Painter, Point, Rect};
use crate::shell::{
    Budgets, CodeKind, Config, Cursor, Effect, Event, Insets, Key, Pointer, PointerKind,
    SETTING_COUNTERS, SETTING_DIFFICULTY, SETTING_EDITOR, SETTING_LANG,
};
use crate::text::{Lang, Text};
use crate::theme;

/// Movement, in logical pixels, that turns a mouse press into a drag.
pub const DRAG_THRESHOLD: f32 = 8.0;
/// Movement, in logical pixels, that turns a touch into a drag rather than
/// a tap.
pub const TOUCH_DRAG_THRESHOLD: f32 = 14.0;
/// How far from a touch, in cells, a piece cell still counts as touched.
pub const TOUCH_SLACK: f32 = 0.4;
/// How far above the finger a piece dragged by touch floats, in cells.
pub const TOUCH_LIFT: f32 = 1.5;
/// Solver nodes the generator may spend in one frame. Time cannot be
/// measured here, so this is a cautious fixed number.
pub const NODES_PER_FRAME: u64 = 20_000;
/// Longest frame step fed to animations, in seconds.
const MAX_FRAME_STEP: f32 = 0.1;

/// How line counters are shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CounterMode {
    /// One tick per clue unit, filled as the line fills.
    Grid,
    /// `filled/clue` as text.
    Numbers,
}

impl CounterMode {
    /// Value stored in the settings.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            CounterMode::Grid => "grid",
            CounterMode::Numbers => "numbers",
        }
    }

    /// Parses a stored [`code`](CounterMode::code).
    #[must_use]
    pub fn from_code(code: &str) -> Option<CounterMode> {
        match code {
            "grid" => Some(CounterMode::Grid),
            "numbers" => Some(CounterMode::Numbers),
            _ => None,
        }
    }

    fn toggled(self) -> CounterMode {
        match self {
            CounterMode::Grid => CounterMode::Numbers,
            CounterMode::Numbers => CounterMode::Grid,
        }
    }
}

/// Something on screen that reacts to a click or tap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Control {
    /// A difficulty tab.
    Tab(Difficulty),
    /// The custom tab.
    Custom,
    /// Back from playing a custom puzzle to its editor.
    Back,
    /// A control of the editor.
    Edit(EditControl),
    /// A half of the language switch.
    Lang(Lang),
    /// The share button.
    Share,
    /// The reset button.
    Reset,
    /// A half of the counter display switch.
    Counters(CounterMode),
    /// The new puzzle button.
    New,
    /// The next button on the win banner.
    Next,
    /// The hint button.
    Hint,
    /// The previous tray page.
    PagePrev,
    /// The next tray page.
    PageNext,
}

/// A short message over the board.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Toast {
    /// The message.
    pub text: Text,
    /// Shown as an error.
    pub error: bool,
    /// Seconds since it appeared.
    pub elapsed: f32,
}

/// A piece moving towards its resting place.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Fly {
    /// Center where the move started.
    pub center: Point,
    /// Cell size where the move started.
    pub cell: f32,
    pub elapsed: f32,
    pub duration: f32,
}

/// Running animations of one piece.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct PieceAnim {
    pub fly: Option<Fly>,
    /// Seconds into a quarter turn.
    pub spin: Option<f32>,
    /// Growth from tray to board scale after a pick-up: the starting cell
    /// size and the seconds elapsed.
    pub grow: Option<(f32, f32)>,
    /// Seconds into the shake of a turn that did not fit.
    pub wobble: Option<f32>,
    /// Seconds since a hint pointed this piece out as misplaced.
    pub blame: Option<f32>,
}

/// The puzzle being played.
#[derive(Debug, Clone)]
pub(crate) struct Game {
    pub board: Board,
    pub code: String,
    /// First letter of the difficulty, for the puzzle id.
    pub letter: char,
    pub won: bool,
    /// Seconds since the win.
    pub win_elapsed: f32,
    pub anims: Vec<PieceAnim>,
    pub hints: Hints,
}

/// A piece being dragged.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Drag {
    pub id: u32,
    pub kind: PointerKind,
    pub piece: usize,
    pub pos: Point,
    /// Where the piece is held, as a fraction of its bounding box.
    pub grab: Point,
    /// Where the piece was before the drag, for cancelling.
    pub origin: PieceState,
}

/// What the active pointer is doing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Gesture {
    Idle,
    /// Pressed on a control; it fires on release over the same control.
    Control {
        id: u32,
        control: Control,
    },
    /// Pressed on a piece, not moved far enough to drag yet.
    Pending {
        id: u32,
        kind: PointerKind,
        piece: usize,
        start: Point,
        grab: Point,
    },
    Dragging(Drag),
    /// The rest of this press does nothing (empty space, cancelled drag).
    Ignored {
        id: u32,
    },
}

impl Gesture {
    fn id(self) -> Option<u32> {
        match self {
            Gesture::Idle => None,
            Gesture::Control { id, .. }
            | Gesture::Pending { id, .. }
            | Gesture::Ignored { id }
            | Gesture::Dragging(Drag { id, .. }) => Some(id),
        }
    }
}

/// The whole trainer. A shell creates it with a [`Config`], feeds it
/// [`Event`]s, calls [`frame`](App::frame) to draw and performs the
/// [`Effect`]s it hands out.
#[derive(Debug, Clone)]
pub struct App {
    pub(crate) width: f32,
    pub(crate) height: f32,
    insets: Insets,
    pub(crate) touch: bool,
    pub(crate) budgets: Budgets,
    entropy: u64,
    puzzles_started: u64,
    pub(crate) lang: Lang,
    pub(crate) counters: CounterMode,
    pub(crate) difficulty: Difficulty,
    pub(crate) game: Option<Game>,
    pub(crate) generator: Option<Generator>,
    pub(crate) gesture: Gesture,
    /// Last mouse position, for hover, `R` and the cursor.
    pub(crate) hover: Option<Point>,
    pub(crate) toast: Option<Toast>,
    /// The tray page on screen.
    pub(crate) tray_page: usize,
    pub(crate) effects: Vec<Effect>,
    cursor: Cursor,
    /// The custom tab is open: editing when there is no game.
    pub(crate) custom: bool,
    /// The puzzle in the custom tab's editor.
    pub(crate) editor: Editor,
    pub(crate) edit: EditState,
    pub(crate) layout: Layout,
}

impl App {
    /// Starts the trainer: opens `config.code` if it is a valid puzzle,
    /// otherwise starts generating one.
    #[must_use]
    pub fn new(config: Config) -> App {
        let setting = |key: &str| {
            config
                .settings
                .iter()
                .rev()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.as_str())
        };
        let lang = setting(SETTING_LANG)
            .and_then(Lang::from_code)
            .unwrap_or_else(|| Lang::from_system(&config.system_language));
        let counters = setting(SETTING_COUNTERS)
            .and_then(CounterMode::from_code)
            .unwrap_or(CounterMode::Grid);
        let difficulty = setting(SETTING_DIFFICULTY)
            .and_then(Difficulty::from_name)
            .unwrap_or(Difficulty::Normal);
        let mut app = App {
            width: config.width,
            height: config.height,
            insets: config.insets,
            touch: config.touch,
            budgets: config.budgets,
            entropy: config.entropy,
            puzzles_started: 0,
            lang,
            counters,
            difficulty,
            game: None,
            generator: None,
            gesture: Gesture::Idle,
            hover: None,
            toast: None,
            tray_page: 0,
            effects: Vec::new(),
            cursor: Cursor::Default,
            custom: false,
            editor: setting(SETTING_EDITOR)
                .and_then(|code| decode(code).ok())
                .filter(|p| p.colors().len() <= 1)
                .map_or_else(Editor::default, |p| Editor::from_puzzle(&p)),
            edit: EditState::default(),
            layout: Layout::compute(&LayoutInput {
                width: 1.0,
                height: 1.0,
                insets: Insets::default(),
                touch: false,
                lang,
                grid_width: 1,
                grid_height: 1,
                pieces: 1,
                max_piece_side: 1,
                mode: Mode::Play,
            }),
        };
        let code = config.code;
        let opened = code
            .as_ref()
            .is_some_and(|(kind, code)| app.open_code(*kind, code));
        if !opened {
            if code.is_some() {
                app.show_toast(Text::InvalidLink, true);
            }
            app.start_generation();
        }
        app.relayout();
        app
    }

    /// Handles one input event.
    pub fn handle(&mut self, event: Event) {
        match event {
            Event::Resize {
                width,
                height,
                insets,
            } => {
                self.width = width;
                self.height = height;
                self.insets = insets;
                self.relayout();
            }
            Event::PointerDown(p) => self.pointer_down(p),
            Event::PointerMove(p) => self.pointer_move(p),
            Event::PointerUp(p) => self.pointer_up(p),
            Event::PointerCancel(p) => {
                if self.gesture.id() == Some(p.id) {
                    if let Gesture::Dragging(drag) = self.gesture {
                        self.cancel_drag(drag);
                    }
                    self.gesture = Gesture::Idle;
                }
            }
            Event::Key(key) => self.key(key),
            Event::CodeChanged(kind, code) => self.code_changed(kind, &code),
            Event::CopyFinished(ok) => {
                if ok {
                    self.show_toast(Text::LinkCopied, false);
                } else {
                    self.show_toast(Text::CopyFailed, true);
                }
            }
        }
        self.update_cursor();
    }

    /// Advances animations by `dt` seconds, runs one generator step if a
    /// puzzle is being generated, and draws everything.
    ///
    /// Returns whether another frame is needed soon. When it returns
    /// `false`, nothing changes until the next event.
    pub fn frame(&mut self, dt: f32, painter: &mut dyn Painter) -> bool {
        let dt = if dt.is_finite() {
            dt.clamp(0.0, MAX_FRAME_STEP)
        } else {
            0.0
        };
        self.step_generator();
        self.step_hint();
        self.step_editor(dt);
        self.animate(dt);
        draw::draw(self, painter);
        self.busy()
    }

    /// Takes the effects produced since the last call, oldest first.
    pub fn take_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.effects)
    }

    /// The current board, if a puzzle is loaded.
    #[must_use]
    pub fn board(&self) -> Option<&Board> {
        self.game.as_ref().map(|g| &g.board)
    }

    /// The code of the current puzzle.
    #[must_use]
    pub fn code(&self) -> Option<&str> {
        self.game.as_ref().map(|g| g.code.as_str())
    }

    /// The puzzle id shown under the title, e.g. `Δ-H-AWYAGP`.
    #[must_use]
    pub fn puzzle_id(&self) -> Option<String> {
        if self.is_editing() {
            let code = encode(&self.editor.puzzle()).unwrap_or_default();
            let short: String = code.chars().take(6).collect();
            return Some(format!("Δ-C-{short}"));
        }
        self.game.as_ref().map(|g| {
            let short: String = g.code.chars().take(6).collect();
            format!("Δ-{}-{short}", g.letter)
        })
    }

    /// Whether the current puzzle is solved.
    #[must_use]
    pub fn is_won(&self) -> bool {
        self.game.as_ref().is_some_and(|g| g.won)
    }

    /// Whether a puzzle is being generated.
    #[must_use]
    pub fn is_generating(&self) -> bool {
        self.generator.is_some()
    }

    /// Interface language.
    #[must_use]
    pub fn lang(&self) -> Lang {
        self.lang
    }

    /// Counter display.
    #[must_use]
    pub fn counter_mode(&self) -> CounterMode {
        self.counters
    }

    /// Selected difficulty.
    #[must_use]
    pub fn difficulty(&self) -> Difficulty {
        self.difficulty
    }

    /// The current layout.
    #[must_use]
    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// The toast on screen, if any.
    #[must_use]
    pub fn toast(&self) -> Option<Toast> {
        self.toast
    }

    /// The piece being dragged, if any.
    #[must_use]
    pub fn dragged_piece(&self) -> Option<usize> {
        match self.gesture {
            Gesture::Dragging(drag) => Some(drag.piece),
            _ => None,
        }
    }

    /// Screen center and cell size at which piece `i` rests: its cells on
    /// the board, or the middle of its tray card.
    #[must_use]
    pub fn piece_home(&self, i: usize) -> (Point, f32) {
        let Some(game) = &self.game else {
            return (Point::default(), 0.0);
        };
        let shape = game.board.shape(i);
        let (w, h) = (to_f32(shape.width()), to_f32(shape.height()));
        if let Some((row, col)) = game.board.piece(i).at {
            let cell = self.layout.cell;
            let center = Point::new(
                self.layout.cells.x + (to_f32(col) + w / 2.0) * cell,
                self.layout.cells.y + (to_f32(row) + h / 2.0) * cell,
            );
            (center, cell)
        } else {
            // A piece on another tray page rests behind the page number.
            let rest = self
                .card_of(i)
                .or_else(|| self.layout.pager.map(|p| p.label))
                .unwrap_or(self.layout.tray);
            (rest.center(), self.layout.tray_cell)
        }
    }

    /// The card of piece `i`, if it is on the tray page on screen.
    #[must_use]
    pub fn card_of(&self, i: usize) -> Option<Rect> {
        let per_page = self.layout.per_page.max(1);
        (i < self.card_count() && i / per_page == self.tray_page)
            .then(|| self.layout.cards.get(i % per_page).copied())
            .flatten()
    }

    /// The tray page on screen, from 0.
    #[must_use]
    pub fn tray_page(&self) -> usize {
        self.tray_page
    }

    /// Number of tray pages.
    #[must_use]
    pub fn tray_pages(&self) -> usize {
        self.layout.pages(self.card_count())
    }

    /// Cards in the tray: the pieces, and in the editor the card that adds
    /// one.
    fn card_count(&self) -> usize {
        if self.is_editing() {
            self.editor_cards()
        } else {
            self.board().map_or(0, Board::piece_count)
        }
    }

    /// Shows the tray page that holds piece `i`.
    fn show_card(&mut self, i: usize) {
        self.tray_page = i / self.layout.per_page.max(1);
    }

    fn turn_page(&mut self, forward: bool) {
        let pages = self.tray_pages();
        self.tray_page = if forward {
            (self.tray_page + 1) % pages
        } else {
            (self.tray_page + pages - 1) % pages
        };
    }

    /// Screen center and cell size at which piece `i` is drawn right now.
    #[must_use]
    pub fn piece_visual(&self, i: usize) -> (Point, f32) {
        if let Gesture::Dragging(drag) = self.gesture
            && drag.piece == i
        {
            return self.drag_visual(&drag);
        }
        let (home, cell) = self.piece_home(i);
        let fly = self
            .game
            .as_ref()
            .and_then(|g| g.anims.get(i))
            .and_then(|a| a.fly);
        match fly {
            Some(fly) => {
                let t = theme::ease_out(fly.elapsed / fly.duration);
                let center = Point::new(
                    fly.center.x + (home.x - fly.center.x) * t,
                    fly.center.y + (home.y - fly.center.y) * t,
                );
                (center, fly.cell + (cell - fly.cell) * t)
            }
            None => (home, cell),
        }
    }

    /// Bounding box of piece `i` on screen right now (ignoring any turn in
    /// progress).
    #[must_use]
    pub fn piece_rect(&self, i: usize) -> Rect {
        let Some(game) = &self.game else {
            return Rect::default();
        };
        let (center, cell) = self.piece_visual(i);
        let shape = game.board.shape(i);
        Rect::centered(
            center,
            to_f32(shape.width()) * cell,
            to_f32(shape.height()) * cell,
        )
    }

    /// Where the dragged piece is drawn: it grows from its tray size to
    /// board size just after being picked up.
    pub(crate) fn drag_visual(&self, drag: &Drag) -> (Point, f32) {
        let board = self.layout.cell;
        let grow = self
            .game
            .as_ref()
            .and_then(|g| g.anims.get(drag.piece))
            .and_then(|a| a.grow);
        let cell = grow.map_or(board, |(from, elapsed)| {
            from + (board - from) * theme::ease_out(elapsed / theme::GROW_TIME)
        });
        (self.drag_center(drag, cell), cell)
    }

    /// Center of the dragged piece drawn with cells of `cell`, keeping the
    /// grabbed point under the pointer.
    fn drag_center(&self, drag: &Drag, cell: f32) -> Point {
        let Some(game) = &self.game else {
            return drag.pos;
        };
        let shape = game.board.shape(drag.piece);
        let (w, h) = (to_f32(shape.width()) * cell, to_f32(shape.height()) * cell);
        let lift = match drag.kind {
            PointerKind::Mouse => 0.0,
            PointerKind::Touch => TOUCH_LIFT * self.layout.cell,
        };
        Point::new(
            drag.pos.x - drag.grab.x * w + w / 2.0,
            drag.pos.y - drag.grab.y * h + h / 2.0 - lift,
        )
    }

    /// The top-left cell the dragged piece would land on, if any of its
    /// cells is over the board.
    pub(crate) fn drop_target(&self, drag: &Drag) -> Option<(isize, isize)> {
        let game = self.game.as_ref()?;
        let shape = game.board.shape(drag.piece);
        // Where the piece lands does not depend on the growth animation.
        let cell = self.layout.cell;
        let rect = Rect::centered(
            self.drag_center(drag, cell),
            to_f32(shape.width()) * cell,
            to_f32(shape.height()) * cell,
        );
        let col = ((rect.x - self.layout.cells.x) / cell).round();
        let row = ((rect.y - self.layout.cells.y) / cell).round();
        // Rounded to a whole cell and far inside `isize`.
        #[allow(clippy::cast_possible_truncation)]
        let (row, col) = (row as isize, col as isize);
        let (height, width) = (
            isize::try_from(game.board.height()).unwrap_or(0),
            isize::try_from(game.board.width()).unwrap_or(0),
        );
        let over = shape.cells().iter().any(|&(dr, dc)| {
            let (r, c) = (
                row + isize::try_from(dr).unwrap_or(0),
                col + isize::try_from(dc).unwrap_or(0),
            );
            (0..height).contains(&r) && (0..width).contains(&c)
        });
        over.then_some((row, col))
    }

    /// Whether pieces may be moved: a puzzle is in play and nothing is
    /// being computed for it.
    pub(crate) fn can_move(&self) -> bool {
        self.generator.is_none()
            && self
                .game
                .as_ref()
                .is_some_and(|g| !g.won && g.hints.job.is_none())
    }

    fn busy(&self) -> bool {
        self.generator.is_some()
            || self.toast.is_some()
            || self.editor_busy()
            || self.game.as_ref().is_some_and(|g| {
                (g.won && g.win_elapsed < theme::WIN_TIME)
                    || g.hints.job.is_some()
                    || g.anims.iter().any(|a| {
                        a.fly.is_some()
                            || a.spin.is_some()
                            || a.grow.is_some()
                            || a.wobble.is_some()
                            || a.blame.is_some()
                    })
            })
    }

    fn layout_input(&self) -> LayoutInput {
        let (grid_width, grid_height, pieces, max_piece_side) = if self.is_editing() {
            (
                self.editor.width(),
                self.editor.height(),
                self.editor_cards(),
                self.editor.max_piece_side(),
            )
        } else if let Some(game) = &self.game {
            (
                game.board.width(),
                game.board.height(),
                game.board.piece_count(),
                game.board.max_piece_side(),
            )
        } else {
            let params = self.difficulty.params();
            (params.width, params.height, params.pieces.max, 3)
        };
        LayoutInput {
            width: self.width,
            height: self.height,
            insets: self.insets,
            touch: self.touch,
            lang: self.lang,
            grid_width,
            grid_height,
            pieces,
            max_piece_side,
            mode: self.edit_mode(),
        }
    }

    pub(crate) fn relayout(&mut self) {
        self.layout = Layout::compute(&self.layout_input());
        self.tray_page = self.tray_page.min(self.tray_pages() - 1);
    }

    pub(crate) fn show_toast(&mut self, text: Text, error: bool) {
        self.toast = Some(Toast {
            text,
            error,
            elapsed: 0.0,
        });
    }

    fn save(&mut self, key: &'static str, value: &str) {
        self.effects.push(Effect::SaveSetting {
            key,
            value: value.to_owned(),
        });
    }

    /// Opens a puzzle code: to play it, or in the custom tab's editor.
    /// Returns `false`, changing nothing, if the code is invalid or the
    /// puzzle cannot be used here.
    fn open_code(&mut self, kind: CodeKind, code: &str) -> bool {
        let Ok(puzzle) = decode(code) else {
            return false;
        };
        if puzzle.colors().len() > 1 {
            return false;
        }
        if kind == CodeKind::Edit {
            self.open_editor(Editor::from_puzzle(&puzzle));
            return true;
        }
        let grid = puzzle.grid();
        // A custom puzzle stays custom whatever its size.
        let matching = Difficulty::ALL.into_iter().find(|d| {
            let params = d.params();
            kind == CodeKind::Play && params.width == grid.width() && params.height == grid.height()
        });
        let letter = match matching {
            Some(d) => {
                self.difficulty = d;
                difficulty_letter(d)
            }
            None => 'C',
        };
        self.generator = None;
        self.install(puzzle, letter);
        true
    }

    /// Starts playing `puzzle`; `C` as the letter means the custom tab.
    pub(crate) fn install(&mut self, puzzle: Puzzle, letter: char) {
        let Ok(code) = encode(&puzzle) else {
            return;
        };
        let pieces = puzzle.pieces().len();
        self.game = Some(Game {
            board: Board::new(puzzle),
            code: code.clone(),
            letter,
            won: false,
            win_elapsed: 0.0,
            anims: vec![PieceAnim::default(); pieces],
            hints: Hints::default(),
        });
        self.gesture = Gesture::Idle;
        self.tray_page = 0;
        self.custom = letter == 'C';
        self.edit = EditState::default();
        let kind = if self.custom {
            CodeKind::Custom
        } else {
            CodeKind::Play
        };
        self.effects.push(Effect::SetCode(kind, code));
        self.relayout();
    }

    fn start_generation(&mut self) {
        if let Gesture::Dragging(drag) = self.gesture {
            self.cancel_drag(drag);
        }
        self.gesture = Gesture::Idle;
        self.custom = false;
        self.edit = EditState::default();
        self.puzzles_started += 1;
        let seed = Rng::for_stream(self.entropy, self.puzzles_started).next_u64();
        self.generator = Generator::new(self.difficulty.params(), seed).ok();
    }

    fn step_generator(&mut self) {
        let Some(generator) = &mut self.generator else {
            return;
        };
        match generator.step(self.budgets.per_frame) {
            Step::Working => {}
            Step::Done(generated) => {
                self.generator = None;
                self.install(generated.puzzle, difficulty_letter(self.difficulty));
            }
            Step::GaveUp { .. } => self.start_generation(),
        }
    }

    fn animate(&mut self, dt: f32) {
        if let Some(toast) = &mut self.toast {
            toast.elapsed += dt;
            if toast.elapsed >= theme::TOAST_TIME {
                self.toast = None;
            }
        }
        let Some(game) = &mut self.game else {
            return;
        };
        if game.won {
            game.win_elapsed = (game.win_elapsed + dt).min(theme::WIN_TIME);
        }
        for anim in &mut game.anims {
            if let Some(fly) = &mut anim.fly {
                fly.elapsed += dt;
                if fly.elapsed >= fly.duration {
                    anim.fly = None;
                }
            }
            if let Some(spin) = &mut anim.spin {
                *spin += dt;
                if *spin >= theme::ROTATE_TIME {
                    anim.spin = None;
                }
            }
            if let Some((_, elapsed)) = &mut anim.grow {
                *elapsed += dt;
                if *elapsed >= theme::GROW_TIME {
                    anim.grow = None;
                }
            }
            if let Some(elapsed) = &mut anim.wobble {
                *elapsed += dt;
                if *elapsed >= theme::WOBBLE_TIME {
                    anim.wobble = None;
                }
            }
            if let Some(elapsed) = &mut anim.blame {
                *elapsed += dt;
                if *elapsed >= theme::BLAME_TIME {
                    anim.blame = None;
                }
            }
        }
    }

    /// Starts moving piece `i` from where it is drawn now to its home.
    fn fly_from(&mut self, i: usize, from: (Point, f32), duration: f32) {
        if let Some(anim) = self.game.as_mut().and_then(|g| g.anims.get_mut(i)) {
            anim.fly = Some(Fly {
                center: from.0,
                cell: from.1,
                elapsed: 0.0,
                duration,
            });
        }
    }

    fn spin(&mut self, i: usize) {
        if let Some(anim) = self.game.as_mut().and_then(|g| g.anims.get_mut(i)) {
            anim.spin = Some(0.0);
        }
    }

    pub(crate) fn check_win(&mut self) {
        if let Some(game) = &mut self.game
            && !game.won
            && game.board.is_solved()
        {
            game.won = true;
            game.win_elapsed = 0.0;
        }
    }

    // ---- hit testing ----

    /// Controls in hit-test order, with whether each is enabled.
    fn controls(&self) -> Vec<(Control, Rect, bool)> {
        let l = &self.layout;
        let has_game = self.game.is_some();
        let mut controls = Vec::new();
        if self.is_won() && self.generator.is_none() {
            let next = if self.custom {
                Control::Back
            } else {
                Control::Next
            };
            controls.push((next, l.next, true));
        }
        for (d, rect) in Difficulty::ALL.into_iter().zip(l.tabs) {
            controls.push((Control::Tab(d), rect, true));
        }
        controls.push((Control::Custom, l.tabs[4], true));
        if self.is_editing() {
            if l.compact {
                controls.push((Control::Lang(self.lang.other()), l.lang, true));
            } else {
                let (en, ua) = Layout::halves(l.lang);
                controls.extend([
                    (Control::Lang(Lang::En), en, true),
                    (Control::Lang(Lang::Ua), ua, true),
                ]);
            }
            controls.push((Control::Share, l.share, true));
            controls.extend(self.edit_controls());
            if let Some(pager) = l.pager
                && self.edit.draft.is_none()
                && self.edit.selected.is_none()
            {
                controls.extend([
                    (Control::PagePrev, pager.prev, true),
                    (Control::PageNext, pager.next, true),
                ]);
            }
            return controls;
        }
        if l.compact {
            // One button each, switching to the other choice.
            controls.extend([
                (Control::Lang(self.lang.other()), l.lang, true),
                (Control::Counters(self.counters.toggled()), l.counters, true),
            ]);
        } else {
            let (en, ua) = Layout::halves(l.lang);
            let (grid, numbers) = Layout::halves(l.counters);
            controls.extend([
                (Control::Lang(Lang::En), en, true),
                (Control::Lang(Lang::Ua), ua, true),
                (Control::Counters(CounterMode::Grid), grid, true),
                (Control::Counters(CounterMode::Numbers), numbers, true),
            ]);
        }
        controls.extend([
            (Control::Share, l.share, has_game),
            (Control::Reset, l.reset, self.can_move()),
            (Control::Hint, l.hint, self.can_move()),
            (
                if self.custom {
                    Control::Back
                } else {
                    Control::New
                },
                l.new,
                true,
            ),
        ]);
        if let Some(pager) = l.pager {
            let paging = self.game.as_ref().is_some_and(|g| !g.won);
            controls.extend([
                (Control::PagePrev, pager.prev, paging),
                (Control::PageNext, pager.next, paging),
            ]);
        }
        controls
    }

    fn control_at(&self, p: Point) -> Option<Control> {
        self.controls()
            .into_iter()
            .find(|&(control, rect, enabled)| enabled && App::zone_of(control, rect).contains(p))
            .map(|(control, _, _)| control)
    }

    /// The piece under `p` that may be picked up. A mouse must hit a piece
    /// cell; a finger may miss by up to [`TOUCH_SLACK`] of a cell, and then
    /// the nearest piece cell counts.
    fn piece_at(&self, p: Point, kind: PointerKind) -> Option<usize> {
        if !self.can_move() {
            return None;
        }
        let game = self.game.as_ref()?;
        let l = &self.layout;
        let cell_at = |p: Point| {
            let col = ((p.x - l.cells.x) / l.cell).floor();
            let row = ((p.y - l.cells.y) / l.cell).floor();
            // Callers check that `p` is inside the cells first.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let cell = (row as usize, col as usize);
            cell
        };
        if l.cells.contains(p) {
            let (row, col) = cell_at(p);
            if let Some(owner) = game.board.owner(row, col) {
                return Some(owner);
            }
        }
        let slack = TOUCH_SLACK * l.cell;
        if kind == PointerKind::Touch && l.cells.inset(-slack).contains(p) {
            let mut best: Option<(f32, usize)> = None;
            for row in 0..game.board.height() {
                for col in 0..game.board.width() {
                    let Some(owner) = game.board.owner(row, col) else {
                        continue;
                    };
                    let r = l.cell_rect(row, col);
                    let dx = (r.x - p.x).max(p.x - r.right()).max(0.0);
                    let dy = (r.y - p.y).max(p.y - r.bottom()).max(0.0);
                    let d = dx.hypot(dy);
                    if d <= slack && best.is_none_or(|(b, _)| d < b) {
                        best = Some((d, owner));
                    }
                }
            }
            if let Some((_, owner)) = best {
                return Some(owner);
            }
        }
        (0..game.board.piece_count()).find(|&i| {
            game.board.piece(i).at.is_none()
                && self
                    .card_of(i)
                    .is_some_and(|card| Layout::zone(card).contains(p))
        })
    }

    fn update_cursor(&mut self) {
        let cursor = match (self.gesture, self.hover) {
            (Gesture::Dragging(_), _) => Cursor::Grabbing,
            (_, Some(p)) if self.control_at(p).is_some() => Cursor::Pointer,
            (_, Some(p)) if self.piece_at(p, PointerKind::Mouse).is_some() => Cursor::Grab,
            _ => Cursor::Default,
        };
        if cursor != self.cursor {
            self.cursor = cursor;
            self.effects.push(Effect::SetCursor(cursor));
        }
    }

    // ---- input ----

    fn pointer_down(&mut self, p: Pointer) {
        if self.gesture != Gesture::Idle {
            return;
        }
        let pos = Point::new(p.x, p.y);
        if p.kind == PointerKind::Mouse {
            self.hover = Some(pos);
        }
        self.gesture = if let Some(control) = self.control_at(pos) {
            Gesture::Control { id: p.id, control }
        } else if let Some(piece) = self.piece_at(pos, p.kind) {
            let rect = self.piece_rect(piece);
            let grab = Point::new(
                ((pos.x - rect.x) / rect.w).clamp(0.0, 1.0),
                ((pos.y - rect.y) / rect.h).clamp(0.0, 1.0),
            );
            Gesture::Pending {
                id: p.id,
                kind: p.kind,
                piece,
                start: pos,
                grab,
            }
        } else {
            Gesture::Ignored { id: p.id }
        };
    }

    fn pointer_move(&mut self, p: Pointer) {
        let pos = Point::new(p.x, p.y);
        if p.kind == PointerKind::Mouse {
            self.hover = Some(pos);
        }
        if self.gesture.id() != Some(p.id) {
            return;
        }
        match self.gesture {
            Gesture::Pending {
                id,
                kind,
                piece,
                start,
                grab,
            } => {
                let threshold = match kind {
                    PointerKind::Mouse => DRAG_THRESHOLD,
                    PointerKind::Touch => TOUCH_DRAG_THRESHOLD,
                };
                if (pos.x - start.x).hypot(pos.y - start.y) >= threshold {
                    self.begin_drag(id, kind, piece, grab, pos);
                }
            }
            Gesture::Dragging(mut drag) => {
                drag.pos = pos;
                self.gesture = Gesture::Dragging(drag);
            }
            _ => {}
        }
    }

    fn pointer_up(&mut self, p: Pointer) {
        let pos = Point::new(p.x, p.y);
        if self.gesture.id() != Some(p.id) {
            return;
        }
        let gesture = self.gesture;
        self.gesture = Gesture::Idle;
        match gesture {
            Gesture::Control { control, .. } => {
                if self.control_at(pos) == Some(control) {
                    self.activate(control);
                }
            }
            Gesture::Pending { kind, piece, .. } => {
                if kind == PointerKind::Touch {
                    self.rotate_piece(piece);
                }
            }
            Gesture::Dragging(mut drag) => {
                drag.pos = pos;
                self.drop_piece(drag);
            }
            Gesture::Idle | Gesture::Ignored { .. } => {}
        }
    }

    fn key(&mut self, key: Key) {
        match key {
            Key::R => match self.gesture {
                Gesture::Dragging(drag) => self.rotate_dragged(drag),
                Gesture::Pending { id, piece, .. } => {
                    self.rotate_piece(piece);
                    self.gesture = Gesture::Ignored { id };
                }
                _ => {
                    if let Some(piece) = self
                        .hover
                        .and_then(|p| self.piece_at(p, PointerKind::Mouse))
                    {
                        self.rotate_piece(piece);
                    }
                }
            },
            Key::Tab => self.set_counters(self.counters.toggled()),
            Key::H => self.request_hint(),
            Key::Escape => {
                self.edit.selected = None;
                self.edit.draft = None;
                if let Gesture::Dragging(drag) = self.gesture {
                    self.cancel_drag(drag);
                    self.gesture = Gesture::Ignored { id: drag.id };
                }
            }
        }
    }

    fn code_changed(&mut self, kind: CodeKind, code: &str) {
        let code = code.trim();
        let current = self.current_code();
        if current
            .as_ref()
            .is_some_and(|(k, c)| *k == kind && c == code)
        {
            return;
        }
        if let Gesture::Dragging(drag) = self.gesture {
            self.cancel_drag(drag);
        }
        if !self.open_code(kind, code) {
            self.show_toast(Text::InvalidLink, true);
            if let Some((kind, code)) = current {
                self.effects.push(Effect::SetCode(kind, code));
            }
        }
    }

    /// The code in the address: the editor's or the game's.
    fn current_code(&self) -> Option<(CodeKind, String)> {
        if self.is_editing() {
            encode(&self.editor.puzzle())
                .ok()
                .map(|code| (CodeKind::Edit, code))
        } else {
            let kind = if self.custom {
                CodeKind::Custom
            } else {
                CodeKind::Play
            };
            self.code().map(|code| (kind, code.to_owned()))
        }
    }

    fn activate(&mut self, control: Control) {
        match control {
            Control::Custom => {
                if !self.is_editing() {
                    let editor = self.editor.clone();
                    self.open_editor(editor);
                }
            }
            Control::Back => self.back_to_editor(),
            Control::Edit(control) => self.edit_action(control),
            Control::Tab(d) => {
                if d != self.difficulty || self.game.is_none() || self.custom {
                    self.difficulty = d;
                    self.save(SETTING_DIFFICULTY, d.name());
                    self.start_generation();
                    self.relayout();
                }
            }
            Control::Lang(lang) => {
                if lang != self.lang {
                    self.lang = lang;
                    self.save(SETTING_LANG, lang.code());
                    self.relayout();
                }
            }
            Control::Share => {
                if let Some((kind, code)) = self.current_code() {
                    // The toast waits for the shell's `CopyFinished`.
                    self.effects.push(Effect::CopyLink(kind, code));
                }
            }
            Control::Reset => self.reset(),
            Control::Counters(mode) => self.set_counters(mode),
            Control::New | Control::Next => self.start_generation(),
            Control::Hint => self.request_hint(),
            Control::PagePrev => self.turn_page(false),
            Control::PageNext => self.turn_page(true),
        }
    }

    fn set_counters(&mut self, mode: CounterMode) {
        if mode != self.counters {
            self.counters = mode;
            self.save(SETTING_COUNTERS, mode.code());
        }
    }

    fn reset(&mut self) {
        if !self.can_move() {
            return;
        }
        let count = self.board().map_or(0, Board::piece_count);
        for i in 0..count {
            let placed = self.board().is_some_and(|b| b.piece(i).at.is_some());
            if placed {
                let from = self.piece_visual(i);
                if let Some(game) = &mut self.game {
                    game.board.remove(i);
                }
                self.fly_from(i, from, theme::RETURN_TIME);
            }
        }
    }

    fn begin_drag(&mut self, id: u32, kind: PointerKind, piece: usize, grab: Point, pos: Point) {
        let (_, start_cell) = self.piece_visual(piece);
        let board_cell = self.layout.cell;
        let Some(game) = &mut self.game else {
            return;
        };
        let origin = game.board.piece(piece);
        game.board.remove(piece);
        if let Some(anim) = game.anims.get_mut(piece) {
            anim.fly = None;
            anim.grow = ((start_cell - board_cell).abs() > 0.5).then_some((start_cell, 0.0));
        }
        self.gesture = Gesture::Dragging(Drag {
            id,
            kind,
            piece,
            pos,
            grab,
            origin,
        });
    }

    /// Puts a released piece down: where it was dropped if it fits there;
    /// back where it came from if it was taken from the board and dropped
    /// over the grid; otherwise into the tray.
    fn drop_piece(&mut self, drag: Drag) {
        let from = self.drag_visual(&drag);
        let target = self.drop_target(&drag);
        let placed = match (target, &mut self.game) {
            (Some((row, col)), Some(game)) => game.board.place(drag.piece, row, col),
            _ => false,
        };
        if !placed
            && target.is_some()
            && let (Some((row, col)), Some(game)) = (drag.origin.at, &mut self.game)
        {
            game.board.set_turns(drag.piece, drag.origin.turns);
            let (row, col) = (
                isize::try_from(row).unwrap_or(0),
                isize::try_from(col).unwrap_or(0),
            );
            game.board.place(drag.piece, row, col);
        }
        let duration = if placed {
            theme::SNAP_TIME
        } else {
            theme::RETURN_TIME
        };
        if self
            .board()
            .is_some_and(|b| b.piece(drag.piece).at.is_none())
        {
            self.show_card(drag.piece);
        }
        self.fly_from(drag.piece, from, duration);
        self.check_win();
    }

    pub(crate) fn cancel_drag(&mut self, drag: Drag) {
        let from = self.drag_visual(&drag);
        if let Some(game) = &mut self.game {
            game.board.set_turns(drag.piece, drag.origin.turns);
            if let Some((row, col)) = drag.origin.at {
                let (row, col) = (
                    isize::try_from(row).unwrap_or(0),
                    isize::try_from(col).unwrap_or(0),
                );
                game.board.place(drag.piece, row, col);
            }
        }
        self.gesture = Gesture::Idle;
        if drag.origin.at.is_none() {
            self.show_card(drag.piece);
        }
        self.fly_from(drag.piece, from, theme::RETURN_TIME);
    }

    fn rotate_dragged(&mut self, mut drag: Drag) {
        let Some(game) = &mut self.game else {
            return;
        };
        game.board.rotate(drag.piece);
        // Keep the grabbed point under the pointer: (x, y) in the old box is
        // (1 - y, x) in the box turned clockwise.
        drag.grab = Point::new(1.0 - drag.grab.y, drag.grab.x);
        self.gesture = Gesture::Dragging(drag);
        self.spin(drag.piece);
    }

    fn rotate_piece(&mut self, piece: usize) {
        if !self.can_move() {
            return;
        }
        let from = self.piece_visual(piece);
        let Some(game) = &mut self.game else {
            return;
        };
        match game.board.rotate(piece) {
            Turned::InTray => self.spin(piece),
            Turned::Stays(..) => {
                self.fly_from(piece, from, theme::ROTATE_TIME);
                self.spin(piece);
            }
            Turned::Blocked => {
                if let Some(anim) = game.anims.get_mut(piece) {
                    anim.wobble = Some(0.0);
                }
            }
        }
        self.check_win();
    }
}

/// The letter used in puzzle ids; Expert is `X` so it differs from Easy.
fn difficulty_letter(d: Difficulty) -> char {
    match d {
        Difficulty::Easy => 'E',
        Difficulty::Normal => 'N',
        Difficulty::Hard => 'H',
        Difficulty::Expert => 'X',
    }
}
