//! Reads an endlogic puzzle from a screenshot of the game's repair screen
//! in the "Grid" display.
//!
//! [`recognize`] takes the pixels and returns the puzzle with a list of
//! places it is not sure about ([`Doubt`]), or why it could not read one
//! ([`Failure`]). Nothing is assumed about where things are on the screen:
//! the counters, the grid and the tray are found by what they look like, so
//! any size from 1280×720 to 3840×2160, full screen or in a window, works.
//!
//! The steps, in short:
//!
//! 1. Connected patches of clearly colored pixels.
//! 2. Among them the ticks of the counters: many bars of one size. Their
//!    stacks next to the grid give the rows and columns, and the cell pitch.
//! 3. The grid: the lines with counters, widened by lines whose counter is a
//!    grey `∅`; each cell is free, dead or a lock of some color.
//! 4. The tray right of the grid: one piece per card, read at the scale its
//!    card draws it.
//! 5. The clue sums: for each color the row clues, the column clues and the
//!    locks plus piece cells must agree; other readings of the pieces (and
//!    of unclear counters) are tried until they do.
#![no_std]
#![forbid(unsafe_code)]
// Pixel arithmetic reads best with x, y, r, g, b and the like.
#![allow(clippy::many_single_char_names, clippy::similar_names)]

extern crate alloc;

mod blobs;
mod board;
mod counters;
mod image;
mod numbers;
mod reconcile;
mod sort;
pub mod synth;
mod tray;

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use endlogic_core::{Cell, Color, Grid, LineClues, MAX_COLORS, MAX_PIECES, Piece, Puzzle};

pub use image::Image;

use board::Look;
use counters::hue_gap;

/// A place in the recognized puzzle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Place {
    /// The clue of a color in a row (`row == true`) or a column.
    Clue {
        /// Color of the clue.
        color: Color,
        /// A row clue, else a column clue.
        row: bool,
        /// Row or column index.
        index: usize,
    },
    /// A cell of the grid.
    Cell {
        /// Row.
        row: usize,
        /// Column.
        col: usize,
    },
    /// A piece, by its index in the puzzle.
    Piece(usize),
}

/// Why a place is doubtful.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Reason {
    /// The picture could be read more than one way here.
    Unclear,
    /// A color that is not in the palette; it got the next free letter.
    NewColor,
    /// The clue sums of its color do not add up.
    Sums,
}

/// A place the recognizer is not sure about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Doubt {
    /// Where.
    pub place: Place,
    /// Why.
    pub reason: Reason,
}

/// A recognized puzzle.
#[derive(Debug, Clone, PartialEq)]
pub struct Recognized {
    /// The puzzle.
    pub puzzle: Puzzle,
    /// Places to check, sorted, each once.
    pub doubts: Vec<Doubt>,
    /// Where the grid's cells are.
    pub grid: Bounds,
    /// Where the tray's cards are.
    pub tray: Bounds,
}

/// A rectangle in the picture, in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Bounds {
    /// Left.
    pub x0: f32,
    /// Top.
    pub y0: f32,
    /// Right.
    pub x1: f32,
    /// Bottom.
    pub y1: f32,
}

/// Why no puzzle was read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    /// Nothing like the repair screen.
    NotPuzzle,
    /// The game shows numbers ("Numbers" display), not ticks.
    NumbersMode,
    /// Some pieces are already on the grid.
    PiecesOnBoard,
    /// Counters, but no grid where they point.
    NoBoard,
    /// No pieces in the tray.
    NoTray,
    /// The picture is too small or could not be read.
    Unreadable,
}

/// The palette letter of a hue: teal is `a`, orange is `b`.
fn palette(hue: f32) -> Option<Color> {
    if hue_gap(hue, 168.0) <= 25.0 {
        Some(Color(0))
    } else if hue_gap(hue, 32.0) <= 22.0 {
        Some(Color(1))
    } else {
        None
    }
}

/// Colors given out so far: palette colors by hue, others by first sight.
#[derive(Default)]
struct Colors {
    others: Vec<(f32, Color)>,
}

impl Colors {
    fn of(&mut self, hue: f32) -> (Color, bool) {
        if let Some(color) = palette(hue) {
            return (color, false);
        }
        if let Some(&(_, color)) = self.others.iter().find(|(h, _)| hue_gap(*h, hue) <= 20.0) {
            return (color, true);
        }
        let next = 2 + self.others.len();
        // At most four colors; any more share the last letter.
        #[allow(clippy::cast_possible_truncation)]
        let color = Color(next.min(MAX_COLORS - 1) as u8);
        self.others.push((hue, color));
        (color, true)
    }
}

/// Reads the puzzle from a screenshot.
///
/// # Errors
///
/// A [`Failure`] saying why no puzzle could be read.
pub fn recognize(image: &Image) -> Result<Recognized, Failure> {
    if image.width() < 320 || image.height() < 180 {
        return Err(Failure::Unreadable);
    }
    let blobs = blobs::blobs(image).ok_or(Failure::NotPuzzle)?;
    let Some(counters) = counters::counters(&blobs) else {
        return Err(if numbers::numbers(&blobs) {
            Failure::NumbersMode
        } else {
            Failure::NotPuzzle
        });
    };
    let board = board::board(image, &counters).map_err(|e| match e {
        board::NoGrid::Small => Failure::Unreadable,
        board::NoGrid::Cells => Failure::NoBoard,
    })?;
    if board::framed(image, &board) {
        return Err(Failure::NotPuzzle);
    }
    if board
        .cells
        .iter()
        .any(|c| matches!(c, Look::Colored { padlock: false, .. }))
    {
        return Err(Failure::PiecesOnBoard);
    }
    let pieces = match tray::tray(image, &blobs, board.right, board.pitch, counters.long) {
        tray::Tray::Pieces(pieces) => pieces,
        tray::Tray::Empty => return Err(Failure::NoTray),
        tray::Tray::EmptyCard => return Err(Failure::PiecesOnBoard),
    };
    if pieces.len() > MAX_PIECES || pieces.iter().any(|p| p.readings.is_empty()) {
        return Err(Failure::Unreadable);
    }
    assemble(&counters, &board, &pieces)
}

/// Puts the parts together and settles the sums.
#[allow(clippy::too_many_lines)]
fn assemble(
    counters: &counters::Counters,
    board: &board::Board,
    tray_pieces: &[tray::TrayPiece],
) -> Result<Recognized, Failure> {
    let pieces = tray_pieces;
    let mut colors = Colors::default();
    let mut doubts: Vec<Doubt> = Vec::new();
    let mut new_colors: Vec<Color> = Vec::new();
    let mut color_of = |hue: f32, colors: &mut Colors| {
        let (color, new) = colors.of(hue);
        if new && !new_colors.contains(&color) {
            new_colors.push(color);
        }
        color
    };
    // Clues: (color, row?, index) -> (count, other reading).
    let mut clues: BTreeMap<Color, LineClues> = BTreeMap::new();
    let mut alternatives: Vec<(Place, usize)> = Vec::new();
    let mut filled: Vec<(Place, usize)> = Vec::new();
    for (row, side, first, len) in [
        (true, &counters.rows, board.row0, board.height),
        (false, &counters.cols, board.col0, board.width),
    ] {
        for line in &side.lines {
            let Ok(index) = usize::try_from(line.index - first) else {
                continue;
            };
            if index >= len {
                continue;
            }
            for stack in &line.stacks {
                let color = color_of(stack.hue, &mut colors);
                let entry = clues.entry(color).or_insert_with(|| LineClues {
                    rows: alloc::vec![0; board.height],
                    cols: alloc::vec![0; board.width],
                });
                let list = if row {
                    &mut entry.rows
                } else {
                    &mut entry.cols
                };
                list[index] = stack.count;
                let place = Place::Clue { color, row, index };
                if stack.blobs != stack.count {
                    alternatives.push((place, stack.blobs));
                }
                filled.push((place, stack.filled));
            }
        }
    }
    let mut cells = Vec::with_capacity(board.cells.len());
    for look in &board.cells {
        cells.push(match *look {
            Look::Free(_) => Cell::Free,
            Look::Dead => Cell::Dead,
            Look::Colored { hue, .. } => Cell::Lock(color_of(hue, &mut colors)),
        });
    }
    let grid =
        Grid::from_cells(board.width, board.height, cells).map_err(|_| Failure::Unreadable)?;
    let piece_colors: Vec<Color> = pieces
        .iter()
        .map(|p| color_of(p.hue, &mut colors))
        .collect();
    for &color in &piece_colors {
        clues.entry(color).or_insert_with(|| LineClues {
            rows: alloc::vec![0; board.height],
            cols: alloc::vec![0; board.width],
        });
    }
    for r in 0..board.height {
        for c in 0..board.width {
            if let Some(Cell::Lock(color)) = grid.get(r, c) {
                clues.entry(color).or_insert_with(|| LineClues {
                    rows: alloc::vec![0; board.height],
                    cols: alloc::vec![0; board.width],
                });
            }
        }
    }
    // Every letter up to the last one used has clues, as in the text format.
    let last = clues.keys().map(|c| c.index()).max().unwrap_or(0);
    for k in 0..last {
        // At most four colors.
        #[allow(clippy::cast_possible_truncation)]
        clues.entry(Color(k as u8)).or_insert_with(|| LineClues {
            rows: alloc::vec![0; board.height],
            cols: alloc::vec![0; board.width],
        });
    }
    // Filled ticks count the locks of their color in their line.
    for &(place, n) in &filled {
        if let Place::Clue { color, row, index } = place {
            let locks = (0..if row { board.width } else { board.height })
                .filter(|&k| {
                    let (r, c) = if row { (index, k) } else { (k, index) };
                    grid.get(r, c) == Some(Cell::Lock(color))
                })
                .count();
            if locks != n {
                doubts.push(Doubt {
                    place,
                    reason: Reason::Unclear,
                });
            }
        }
    }
    for &(row, index) in &board.unmarked {
        for &color in clues.keys() {
            doubts.push(Doubt {
                place: Place::Clue { color, row, index },
                reason: Reason::Unclear,
            });
        }
    }
    let settled = reconcile::settle(&grid, &mut clues, pieces, &piece_colors, &alternatives);
    doubts.extend(settled.doubts);
    for &color in &new_colors {
        for (i, &c) in piece_colors.iter().enumerate() {
            if c == color {
                doubts.push(Doubt {
                    place: Place::Piece(i),
                    reason: Reason::NewColor,
                });
            }
        }
    }
    let pieces: Vec<Piece> = settled
        .shapes
        .into_iter()
        .zip(&piece_colors)
        .map(|(shape, &color)| Piece::new(color, shape))
        .collect();
    let puzzle = Puzzle::new(grid, pieces, clues).map_err(|_| Failure::Unreadable)?;
    doubts.sort();
    doubts.dedup();
    // The cards reach about 0.8 cell beyond their pieces.
    let card = 0.8 * board.pitch;
    let mut tray = Bounds {
        x0: f32::MAX,
        y0: f32::MAX,
        x1: f32::MIN,
        y1: f32::MIN,
    };
    for p in tray_pieces {
        let (cx, cy) = (
            f32::midpoint(p.bounds.x0, p.bounds.x1),
            f32::midpoint(p.bounds.y0, p.bounds.y1),
        );
        tray.x0 = tray.x0.min(cx - card);
        tray.y0 = tray.y0.min(cy - card);
        tray.x1 = tray.x1.max(cx + card);
        tray.y1 = tray.y1.max(cy + card);
    }
    Ok(Recognized {
        puzzle,
        doubts,
        grid: Bounds {
            x0: board.left,
            y0: board.top,
            x1: board.right,
            y1: board.bottom,
        },
        tray,
    })
}
