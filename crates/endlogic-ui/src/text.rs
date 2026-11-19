//! Interface texts in English and Ukrainian.

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
