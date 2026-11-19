//! Interface texts in English and Ukrainian.

use endlogic_core::Axis;
use endlogic_core::diagnose::Problem;
use endlogic_core::generator::Difficulty;

/// Interface language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lang {
    /// English.
    En,
    /// Ukrainian.
    Ua,
}

impl Lang {
    /// Value stored in the settings: `en` or `ua`.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Ua => "ua",
        }
    }

    /// Parses a stored [`code`](Lang::code).
    #[must_use]
    pub fn from_code(code: &str) -> Option<Lang> {
        match code {
            "en" => Some(Lang::En),
            "ua" => Some(Lang::Ua),
            _ => None,
        }
    }

    /// The language for a system language tag: Ukrainian for `uk`,
    /// English for everything else.
    #[must_use]
    pub fn from_system(tag: &str) -> Lang {
        let primary = tag.split(['-', '_']).next().unwrap_or("");
        if primary.eq_ignore_ascii_case("uk") {
            Lang::Ua
        } else {
            Lang::En
        }
    }

    /// The other language.
    #[must_use]
    pub const fn other(self) -> Lang {
        match self {
            Lang::En => Lang::Ua,
            Lang::Ua => Lang::En,
        }
    }

    /// Label on the language switch.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Lang::En => "EN",
            Lang::Ua => "UA",
        }
    }
}

/// A piece of interface text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Text {
    /// Line under the title.
    Subtitle,
    /// Help: dragging.
    HelpDrag,
    /// Help: rotating.
    HelpRotate,
    /// Help: counter display.
    HelpDisplay,
    /// Reset button.
    Reset,
    /// Counter display: ticks.
    Grid,
    /// Counter display: numbers.
    Numbers,
    /// New puzzle button.
    New,
    /// Next puzzle button on the win banner.
    Next,
    /// Win banner.
    Complete,
    /// Shown while a puzzle is generated.
    Generating,
    /// Toast after sharing.
    LinkCopied,
    /// Toast when the clipboard refused the link.
    CopyFailed,
    /// Toast for a bad code.
    InvalidLink,
    /// Share button.
    Share,
    /// Share button where space is short.
    ShareShort,
    /// Hint button.
    Hint,
    /// Line on the win banner, followed by the number of hints.
    HintsUsed,
    /// Toast when no hint could be computed.
    HintFailed,
    /// The custom tab.
    Custom,
    /// Board width stepper.
    Width,
    /// Board height stepper.
    Height,
    /// Closes the digit strip; accepts a piece.
    Done,
    /// Leaves the piece editor; stops the solver.
    Cancel,
    /// Adds a copy of the piece.
    Duplicate,
    /// Removes the piece.
    Delete,
    /// Clears the editor.
    Clear,
    /// The clear button asking for a second tap.
    Sure,
    /// Runs the solver.
    Solve,
    /// Plays the puzzle in the editor.
    Play,
    /// Back from playing to the editor.
    Edit,
    /// While the solver runs.
    Solving,
    /// The solver found nothing.
    NoSolution,
    /// The solver ran out of its budget.
    Stopped,
    /// Gives the solver more nodes.
    Continue,
    /// The piece being drawn has no cells.
    EmptyPiece,
    /// The piece being drawn falls apart.
    DisconnectedPiece,
    /// The puzzle has no pieces.
    NoPieces,
}

/// The text for `key` in `lang`.
#[must_use]
pub const fn text(lang: Lang, key: Text) -> &'static str {
    match lang {
        Lang::En => match key {
            Text::Subtitle => "Place components on the board to repair",
            Text::HelpDrag => "Drag to move a component",
            Text::HelpRotate => "Rotate component",
            Text::HelpDisplay => "Switch display mode",
            Text::Reset => "Reset",
            Text::Grid => "Grid",
            Text::Numbers => "Numbers",
            Text::New => "New",
            Text::Next => "Next",
            Text::Complete => "REPAIR COMPLETE",
            Text::Generating => "Generating…",
            Text::LinkCopied => "Link copied",
            Text::CopyFailed => "Couldn't copy the link",
            Text::InvalidLink => "Invalid puzzle link",
            Text::Share => "Share",
            Text::ShareShort => "Link",
            Text::Hint => "Hint",
            Text::HintsUsed => "Hints:",
            Text::HintFailed => "Couldn't compute a hint",
            Text::Custom => "CUSTOM",
            Text::Width => "Width",
            Text::Height => "Height",
            Text::Done => "Done",
            Text::Cancel => "Cancel",
            Text::Duplicate => "Duplicate",
            Text::Delete => "Delete",
            Text::Clear => "Clear",
            Text::Sure => "Sure?",
            Text::Solve => "Solve",
            Text::Play => "Play",
            Text::Edit => "Edit",
            Text::Solving => "Solving…",
            Text::NoSolution => "No solution",
            Text::Stopped => "Too complex, stopped",
            Text::Continue => "Continue",
            Text::EmptyPiece => "Add at least one cell",
            Text::DisconnectedPiece => "Cells must be connected",
            Text::NoPieces => "Add a piece",
        },
        Lang::Ua => match key {
            Text::Subtitle => "Розмістіть компоненти на платі, щоб відремонтувати",
            Text::HelpDrag => "Тягніть, щоб перемістити компонент",
            Text::HelpRotate => "Повернути компонент",
            Text::HelpDisplay => "Змінити режим відображення",
            Text::Reset => "Скинути",
            Text::Grid => "Сітка",
            Text::Numbers => "Числа",
            Text::New => "Нова",
            Text::Next => "Далі",
            Text::Complete => "РЕМОНТ ЗАВЕРШЕНО",
            Text::Generating => "Генерація…",
            Text::LinkCopied => "Посилання скопійовано",
            Text::CopyFailed => "Не вдалося скопіювати посилання",
            Text::InvalidLink => "Некоректне посилання",
            Text::Share => "Поділитися",
            Text::ShareShort => "Лінк",
            Text::Hint => "Підказка",
            Text::HintsUsed => "Підказок:",
            Text::HintFailed => "Не вдалося порахувати підказку",
            Text::Custom => "СВОЯ",
            Text::Width => "Ширина",
            Text::Height => "Висота",
            Text::Done => "Готово",
            Text::Cancel => "Скасувати",
            Text::Duplicate => "Дублювати",
            Text::Delete => "Видалити",
            Text::Clear => "Очистити",
            Text::Sure => "Точно?",
            Text::Solve => "Розв'язати",
            Text::Play => "Грати",
            Text::Edit => "Редагувати",
            Text::Solving => "Розв'язую…",
            Text::NoSolution => "Розв'язків немає",
            Text::Stopped => "Занадто складно, зупинено",
            Text::Continue => "Продовжити",
            Text::EmptyPiece => "Додайте хоча б одну клітинку",
            Text::DisconnectedPiece => "Клітинки мають бути з'єднані",
            Text::NoPieces => "Додайте фігуру",
        },
    }
}

/// The tab label for a difficulty.
#[must_use]
pub const fn difficulty_label(lang: Lang, difficulty: Difficulty) -> &'static str {
    match (lang, difficulty) {
        (Lang::En, Difficulty::Easy) => "EASY",
        (Lang::En, Difficulty::Normal) => "NORMAL",
        (Lang::En, Difficulty::Hard) => "HARD",
        (Lang::En, Difficulty::Expert) => "EXPERT",
        (Lang::Ua, Difficulty::Easy) => "ЛЕГКО",
        (Lang::Ua, Difficulty::Normal) => "НОРМА",
        (Lang::Ua, Difficulty::Hard) => "ВАЖКО",
        (Lang::Ua, Difficulty::Expert) => "ЕКСПЕРТ",
    }
}

/// `Solution n of m`, with `+` after `m` when there may be more.
#[must_use]
pub fn solution_of(lang: Lang, n: usize, m: usize, more: bool) -> String {
    let plus = if more { "+" } else { "" };
    match lang {
        Lang::En => format!("Solution {n} of {m}{plus}"),
        Lang::Ua => format!("Розв'язок {n} з {m}{plus}"),
    }
}

/// A structural problem of a puzzle in words. Lines count from 1.
#[must_use]
pub fn problem_text(lang: Lang, problem: &Problem) -> String {
    let line = |axis: Axis, index: usize| {
        let n = index + 1;
        match (lang, axis) {
            (Lang::En, Axis::Row) => format!("Row {n}"),
            (Lang::En, Axis::Column) => format!("Column {n}"),
            (Lang::Ua, Axis::Row) => format!("У рядку {n}"),
            (Lang::Ua, Axis::Column) => format!("У стовпці {n}"),
        }
    };
    match (lang, *problem) {
        (Lang::En, Problem::LineSums { rows, cols, .. }) => {
            format!("Rows give {rows}, columns give {cols}")
        }
        (Lang::Ua, Problem::LineSums { rows, cols, .. }) => {
            format!("Рядки дають {rows}, стовпці {cols}")
        }
        (Lang::En, Problem::Area { clues, cells, .. }) => {
            format!("Clues need {clues} cells, pieces and locks give {cells}")
        }
        (Lang::Ua, Problem::Area { clues, cells, .. }) => {
            format!("Підказки вимагають {clues} клітинок, фігури й замки дають {cells}")
        }
        (Lang::En, Problem::LineTooLong { axis, index, .. }) => {
            format!("{} needs more than fits", line(axis, index))
        }
        (Lang::Ua, Problem::LineTooLong { axis, index, .. }) => {
            format!("{} більше, ніж уміщається", line(axis, index))
        }
        (Lang::En, Problem::TooManyLocks { axis, index, .. }) => {
            format!("{} has more locks than its clue", line(axis, index))
        }
        (Lang::Ua, Problem::TooManyLocks { axis, index, .. }) => {
            format!("{} замків більше за підказку", line(axis, index))
        }
        (_, Problem::NoPieces) => text(lang, Text::NoPieces).to_owned(),
    }
}
