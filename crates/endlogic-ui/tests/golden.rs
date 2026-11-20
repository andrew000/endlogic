//! Golden results of the trainer: what hints answer in given positions, and
//! fingerprints of the drawing commands of many scenes.
//!
//! `tests/golden/hints.txt` holds the hint answers in full. The scenes are
//! kept as fingerprints only, one line per scene in `tests/golden/draw.txt`:
//! a hash of a text record of every painter call, with numbers rounded to
//! hundredths. When a fingerprint differs, the full record of that scene is
//! written under the target directory and its path is printed. `just bless`
//! rewrites both files, and keeps the records of the blessed scenes under
//! the target directory too, so a later difference can be compared.

mod common;

use std::collections::BTreeSet;
use std::fmt::Write as _;

use common::golden;
use common::*;
use endlogic_core::code::{decode, encode};
use endlogic_core::format::parse;
use endlogic_core::solver::{DEFAULT_NODE_BUDGET, search};
use endlogic_core::{Puzzle, Solution};
use endlogic_ui::{
    Align, App, CodeKind, Config, Event, Font, FontFamily, Key, Painter, Point, PointerKind, Rect,
    Rgba, SETTING_COUNTERS, SETTING_DIFFICULTY, SETTING_LANG,
};

/// Generated puzzles from the solver's fixed list in `endlogic-core`.
const HARD_00: &str = "AWYAGyOW-e1AAGEQjEDiABRHRJ4j5JMj8yDgzbA";
const EXPERT_00: &str = "AXcAElRkdmRXYjdgAAaAQeg5AUAZDGeIzmMeRyeiLiLkaXLn7w";

/// The theme's error red, which a blamed piece is filled with at first.
const BLAME: Rgba = Rgba::hex(0xFF_4D5E);

// ---- a painter that writes every call down ----

/// Writes every painter call as a line of text.
#[derive(Debug, Default)]
struct Writer {
    text: String,
    calls: usize,
    depth: i32,
    /// Fill polygons in the blame color: their centers.
    blamed: Vec<Point>,
}

/// A number rounded to hundredths, without a negative zero.
fn num(x: f32) -> String {
    let s = format!("{x:.2}");
    if s == "-0.00" { "0.00".to_owned() } else { s }
}

fn pt(p: Point) -> String {
    format!("{} {}", num(p.x), num(p.y))
}

fn rect(r: Rect) -> String {
    format!("{} {} {} {}", num(r.x), num(r.y), num(r.w), num(r.h))
}

fn color(c: Rgba) -> String {
    format!("#{:02x}{:02x}{:02x}{:02x}", c.r, c.g, c.b, c.a)
}

fn font(f: Font) -> String {
    let family = match f.family {
        FontFamily::Sans => "sans",
        FontFamily::Mono => "mono",
    };
    let weight = if f.bold { "bold" } else { "regular" };
    format!("{family} {} {weight}", num(f.size))
}

impl Writer {
    fn line(&mut self, line: &str) {
        self.calls += 1;
        self.text.push_str(line);
        self.text.push('\n');
    }
}

impl Painter for Writer {
    fn fill_rect(&mut self, r: Rect, radius: f32, c: Rgba) {
        self.line(&format!(
            "fill_rect {} r {} {}",
            rect(r),
            num(radius),
            color(c)
        ));
    }
    fn stroke_rect(&mut self, r: Rect, radius: f32, width: f32, c: Rgba) {
        self.line(&format!(
            "stroke_rect {} r {} w {} {}",
            rect(r),
            num(radius),
            num(width),
            color(c)
        ));
    }
    fn line(&mut self, from: Point, to: Point, width: f32, c: Rgba) {
        let text = format!("line {} {} w {} {}", pt(from), pt(to), num(width), color(c));
        Writer::line(self, &text);
    }
    fn fill_circle(&mut self, center: Point, radius: f32, c: Rgba) {
        self.line(&format!(
            "fill_circle {} r {} {}",
            pt(center),
            num(radius),
            color(c)
        ));
    }
    fn stroke_circle(&mut self, center: Point, radius: f32, width: f32, c: Rgba) {
        self.line(&format!(
            "stroke_circle {} r {} w {} {}",
            pt(center),
            num(radius),
            num(width),
            color(c)
        ));
    }
    fn fill_polygon(&mut self, points: &[Point], c: Rgba) {
        if c == BLAME && !points.is_empty() {
            let n = f32::from(u16::try_from(points.len()).unwrap());
            let x = points.iter().map(|p| p.x).sum::<f32>() / n;
            let y = points.iter().map(|p| p.y).sum::<f32>() / n;
            self.blamed.push(Point::new(x, y));
        }
        let list: Vec<String> = points.iter().map(|&p| pt(p)).collect();
        self.line(&format!("fill_polygon {} {}", list.join(" "), color(c)));
    }
    fn text(&mut self, text: &str, anchor: Point, f: Font, align: Align, c: Rgba) {
        self.line(&format!(
            "text {text:?} at {} {} {align:?} {}",
            pt(anchor),
            font(f),
            color(c)
        ));
    }
    fn text_width(&mut self, text: &str, f: Font) -> f32 {
        self.line(&format!("text_width {text:?} {}", font(f)));
        // A stand-in: every character a little over half an em wide.
        f32::from(u16::try_from(text.chars().count()).unwrap()) * f.size * 0.55
    }
    fn push_clip(&mut self, r: Rect) {
        self.depth += 1;
        self.line(&format!("push_clip {}", rect(r)));
    }
    fn pop_clip(&mut self) {
        self.depth -= 1;
        assert!(self.depth >= 0, "pop_clip without push_clip");
        self.line("pop_clip");
    }
}

/// Runs frames of `dt` seconds until the app is idle; returns the record of
/// the last one.
fn rest(app: &mut App) -> Writer {
    for _ in 0..2000 {
        let mut writer = Writer::default();
        let busy = app.frame(0.05, &mut writer);
        assert_eq!(writer.depth, 0, "unbalanced clips");
        if !busy {
            return writer;
        }
    }
    panic!("the app never became idle");
}

/// One frame of `dt` seconds.
fn one(app: &mut App, dt: f32) -> Writer {
    let mut writer = Writer::default();
    app.frame(dt, &mut writer);
    assert_eq!(writer.depth, 0, "unbalanced clips");
    writer
}

/// FNV-1a, 64 bits.
fn fingerprint(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

// ---- hints ----

fn puzzle_named(name: &str) -> Puzzle {
    match name {
        "hard-00" => decode(HARD_00).unwrap(),
        "expert-00" => decode(EXPERT_00).unwrap(),
        _ => sample(&format!("{name}.txt")),
    }
}

fn solutions(puzzle: &Puzzle) -> Vec<Solution> {
    search(puzzle, usize::MAX, DEFAULT_NODE_BUDGET).solutions
}

/// A piece put down before asking: piece, quarter turns, row, column.
type Put = (usize, u8, usize, usize);

/// `pieces` of `solution`, as puts.
fn puts(solution: &Solution, pieces: &[usize]) -> Vec<Put> {
    pieces
        .iter()
        .map(|&i| {
            let at = solution.placements[i];
            let turns = u8::try_from(at.rotation.quarter_turns()).unwrap();
            (i, turns, at.row, at.col)
        })
        .collect()
}

/// The first place, in reading order and turns 0 to 3, where piece `i`
/// fits on an empty board but covers cells no solution gives it.
fn wrong_put(puzzle: &Puzzle, i: usize) -> Put {
    let board = endlogic_ui::Board::new(puzzle.clone());
    let right: Vec<Vec<(usize, usize)>> = solutions(puzzle)
        .iter()
        .map(|s| {
            let mut board = board.clone();
            let at = s.placements[i];
            board.set_turns(i, u8::try_from(at.rotation.quarter_turns()).unwrap());
            board.place(
                i,
                isize::try_from(at.row).unwrap(),
                isize::try_from(at.col).unwrap(),
            );
            board.cells_of(i)
        })
        .collect();
    for row in 0..board.height() {
        for col in 0..board.width() {
            for turns in 0..4u8 {
                let (r, c) = (isize::try_from(row).unwrap(), isize::try_from(col).unwrap());
                if !board.fits(i, turns, r, c) {
                    continue;
                }
                let mut trial = board.clone();
                trial.set_turns(i, turns);
                trial.place(i, r, c);
                if !right.contains(&trial.cells_of(i)) {
                    return (i, turns, row, col);
                }
            }
        }
    }
    panic!("piece {i} fits nowhere wrong");
}

fn placed(app: &App) -> Vec<Option<(usize, usize)>> {
    let board = app.board().unwrap();
    (0..board.piece_count())
        .map(|i| board.piece(i).at)
        .collect()
}

/// Presses `H` and runs frames until the hint is given. Returns the pieces
/// drawn in the blame color on the first frame after it.
fn ask(app: &mut App) -> BTreeSet<usize> {
    app.handle(Event::Key(Key::H));
    for _ in 0..10_000 {
        if !app.is_hinting() {
            break;
        }
        one(app, 0.0);
    }
    assert!(!app.is_hinting(), "the hint never came");
    let drawing = one(app, 0.0);
    let layout = app.layout().clone();
    let board = app.board().unwrap();
    let mut blamed = BTreeSet::new();
    for center in drawing.blamed {
        for row in 0..board.height() {
            for col in 0..board.width() {
                if layout.cell_rect(row, col).contains(center) {
                    blamed.extend(board.owner(row, col));
                }
            }
        }
    }
    rest(app);
    blamed
}

fn silhouette(app: &App) -> String {
    let width = app.board().unwrap().width();
    let mut text = String::new();
    for row in app.silhouette().chunks(width) {
        text.extend(row.iter().map(|&on| if on { 'o' } else { '.' }));
        text.push('\n');
    }
    text
}

/// The positions hints are asked about: a title, a puzzle and the pieces
/// put down first.
fn hint_cases() -> Vec<(String, Puzzle, Vec<Put>)> {
    let mut cases = Vec::new();
    let wln = puzzle_named("wln006-2");
    let wln_solution = solutions(&wln).remove(0);
    cases.push(("wln006-2, nothing placed".to_owned(), wln.clone(), vec![]));
    cases.push((
        "wln006-2, one piece where the solution has it".to_owned(),
        wln.clone(),
        puts(&wln_solution, &[4]),
    ));
    cases.push((
        "wln006-2, the square overfills a row".to_owned(),
        wln.clone(),
        vec![(3, 0, 5, 2)],
    ));
    cases.push((
        "wln006-2, a right piece and a wrong one".to_owned(),
        wln.clone(),
        [puts(&wln_solution, &[0]), vec![wrong_put(&wln, 2)]].concat(),
    ));
    let wlh = puzzle_named("wlh9000-1");
    let wlh_solutions = solutions(&wlh);
    cases.push(("wlh9000-1, nothing placed".to_owned(), wlh.clone(), vec![]));
    cases.push((
        "wlh9000-1, three pieces of its last solution".to_owned(),
        wlh.clone(),
        puts(wlh_solutions.last().unwrap(), &[0, 1, 2]),
    ));
    let perm = puzzle_named("permutations-4");
    cases.push((
        "permutations-4, two pieces in one row".to_owned(),
        perm.clone(),
        vec![(0, 0, 0, 0), (1, 0, 0, 2)],
    ));
    cases.push((
        "permutations-4, one piece on the diagonal".to_owned(),
        perm,
        vec![(2, 0, 1, 1)],
    ));
    let hard = puzzle_named("hard-00");
    cases.push(("hard-00, nothing placed".to_owned(), hard.clone(), vec![]));
    cases.push((
        "hard-00, one piece in a wrong place".to_owned(),
        hard.clone(),
        vec![wrong_put(&hard, 0)],
    ));
    let expert = puzzle_named("expert-00");
    let expert_solution = solutions(&expert).remove(0);
    cases.push((
        "expert-00, two pieces where the solution has them".to_owned(),
        expert.clone(),
        puts(&expert_solution, &[1, 3]),
    ));
    cases
}

#[test]
fn hints() {
    let mut out = String::new();
    for (title, puzzle, put) in hint_cases() {
        let mut app = open(&puzzle);
        for &(i, turns, row, col) in &put {
            app.place(i, turns, row, col, PointerKind::Mouse);
        }
        writeln!(out, "== {title}").unwrap();
        let list: Vec<String> = put
            .iter()
            .map(|(i, t, r, c)| format!("piece {i} turned {t} at {r} {c}"))
            .collect();
        writeln!(
            out,
            "placed: {}",
            if list.is_empty() {
                "nothing".to_owned()
            } else {
                list.join(", ")
            }
        )
        .unwrap();
        let blamed = ask(&mut app);
        assert!(blamed.is_empty(), "{title}: the first hint blames");
        writeln!(out, "first hint, silhouette:").unwrap();
        out.push_str(&silhouette(&app));
        let before = placed(&app);
        let blamed = ask(&mut app);
        let after = placed(&app);
        let moved: Vec<usize> = (0..after.len())
            .filter(|&i| before[i] != after[i])
            .collect();
        let board = app.board().unwrap();
        for &i in &moved {
            let piece = board.piece(i);
            match piece.at {
                Some((row, col)) => writeln!(
                    out,
                    "second hint: piece {i} turned {} to {row} {col}",
                    piece.turns
                ),
                None => writeln!(out, "second hint: piece {i} taken off"),
            }
            .unwrap();
        }
        for i in &blamed {
            writeln!(out, "second hint: piece {i} is wrong").unwrap();
        }
        if moved.is_empty() && blamed.is_empty() {
            let toast = app.toast().map(|t| format!("{:?}", t.text));
            writeln!(
                out,
                "second hint: nothing moves, message {}",
                toast.unwrap_or_else(|| "none".to_owned())
            )
            .unwrap();
        }
        writeln!(
            out,
            "hints used {}, solved {}\n",
            app.hints_used(),
            app.is_won()
        )
        .unwrap();
    }
    golden::check("hints.txt", &out);
}

// ---- drawing ----

fn level(name: &str, w: f32, h: f32, lang: &str, touch: bool) -> Config {
    let mut config = config(w, h);
    config.settings = vec![
        (SETTING_DIFFICULTY.to_owned(), name.to_owned()),
        (SETTING_LANG.to_owned(), lang.to_owned()),
    ];
    config.touch = touch;
    config
}

fn with_code(kind: CodeKind, puzzle: &Puzzle, w: f32, h: f32, lang: &str) -> App {
    let mut config = config(w, h);
    config.code = Some((kind, encode(puzzle).unwrap()));
    config.settings = vec![(SETTING_LANG.to_owned(), lang.to_owned())];
    let mut app = App::new(config);
    rest(&mut app);
    app
}

fn press(app: &mut App, r: Rect) {
    app.click(r.center());
    rest(app);
}

/// A 16×16 editor puzzle with 31 dominoes: the tray needs pages.
fn dominoes() -> Puzzle {
    let mut text = String::from("endlogic 1\ngrid\n");
    for _ in 0..16 {
        text.push_str(&".".repeat(16));
        text.push('\n');
    }
    let clues = vec!["0"; 16].join(" ");
    writeln!(text, "rows {clues}\ncols {clues}").unwrap();
    for _ in 0..31 {
        text.push_str("piece\n##\n");
    }
    parse(&text).unwrap()
}

/// Every scene by name, each drawn to rest unless its name says otherwise:
/// first the levels at every size.
fn scenes() -> Vec<(String, Writer)> {
    let mut out: Vec<(String, Writer)> = Vec::new();
    let sizes = [
        ("1366x768", 1366.0, 768.0, false),
        ("1920x1080", 1920.0, 1080.0, false),
        ("390x844-touch", 390.0, 844.0, true),
        ("844x390", 844.0, 390.0, false),
    ];
    for name in ["easy", "normal", "hard", "expert"] {
        for (size, w, h, touch) in sizes {
            let mut app = App::new(level(name, w, h, "en", touch));
            out.push((format!("{name} {size} en"), rest(&mut app)));
        }
    }
    for (size, w, h, touch) in sizes {
        let mut app = App::new(level("hard", w, h, "ua", touch));
        out.push((format!("hard {size} ua"), rest(&mut app)));
    }
    for (size, w, h, touch) in [sizes[0], sizes[2]] {
        let mut config = level("expert", w, h, "en", touch);
        config
            .settings
            .push((SETTING_COUNTERS.to_owned(), "numbers".to_owned()));
        let mut app = App::new(config);
        out.push((format!("expert {size} numbers"), rest(&mut app)));
    }
    out.extend(play_scenes());
    out.extend(editor_scenes());
    out
}

/// Moments of play on `wln006-2`: dragging, an overfilled row, hints.
fn play_scenes() -> Vec<(String, Writer)> {
    let mut out: Vec<(String, Writer)> = Vec::new();
    let wln = sample("wln006-2.txt");
    // A piece held halfway to the board.
    let mut app = with_code(CodeKind::Play, &wln, 1366.0, 768.0, "en");
    let from = card(&app, 4);
    let to = cell(&app, 3, 3);
    app.start_drag(from, to, PointerKind::Mouse);
    out.push(("wln006-2 dragging".to_owned(), rest(&mut app)));
    let mut app = with_code(CodeKind::Play, &wln, 390.0, 844.0, "ua");
    let from = card(&app, 1);
    let to = cell(&app, 2, 2);
    app.start_drag(from, to, PointerKind::Touch);
    out.push((
        "wln006-2 390x844 dragging by touch".to_owned(),
        rest(&mut app),
    ));
    // The square overfills the last row; its counter turns red.
    let mut app = with_code(CodeKind::Play, &wln, 1366.0, 768.0, "en");
    app.place(3, 0, 5, 2, PointerKind::Mouse);
    out.push(("wln006-2 overfilled row".to_owned(), rest(&mut app)));
    // The silhouette, then the square blamed at once and 1.3 s later,
    // when the red fades.
    ask(&mut app);
    out.push(("wln006-2 silhouette".to_owned(), rest(&mut app)));
    app.handle(Event::Key(Key::H));
    while app.is_hinting() {
        one(&mut app, 0.0);
    }
    out.push(("wln006-2 blamed piece".to_owned(), one(&mut app, 0.0)));
    for _ in 0..13 {
        one(&mut app, 0.1);
    }
    out.push((
        "wln006-2 blamed piece fading".to_owned(),
        one(&mut app, 0.0),
    ));
    // Solved with hints only.
    let mut app = with_code(CodeKind::Play, &wln, 1366.0, 768.0, "en");
    for _ in 0..10 {
        if app.is_won() {
            break;
        }
        ask(&mut app);
    }
    assert!(app.is_won());
    out.push(("wln006-2 won with hints".to_owned(), rest(&mut app)));
    out
}

/// The custom tab: the editor in its states, and playing a custom puzzle.
fn editor_scenes() -> Vec<(String, Writer)> {
    let mut out: Vec<(String, Writer)> = Vec::new();
    let wln = sample("wln006-2.txt");
    // The custom tab.
    let mut app = App::new(config(1366.0, 768.0));
    rest(&mut app);
    let tab = app.layout().tabs[4];
    press(&mut app, tab);
    out.push(("editor empty".to_owned(), rest(&mut app)));
    for (size, w, h, lang) in [
        ("1366x768", 1366.0, 768.0, "en"),
        ("390x844", 390.0, 844.0, "ua"),
    ] {
        let mut app = with_code(CodeKind::Edit, &wln, w, h, lang);
        out.push((format!("editor wln006-2 {size} {lang}"), rest(&mut app)));
        // The digit strip of row 1.
        let l = app.layout().clone();
        let row1 = l.cell_rect(1, 0);
        press(
            &mut app,
            Rect::new(l.row_counters.x, row1.y, l.row_counters.w, row1.h),
        );
        out.push((format!("editor digit strip {size} {lang}"), rest(&mut app)));
        let mut app = with_code(CodeKind::Edit, &wln, w, h, lang);
        let card = app.card_of(0).unwrap();
        press(&mut app, card);
        out.push((format!("editor piece editor {size} {lang}"), rest(&mut app)));
    }
    let wlh = sample("wlh9000-1.txt");
    for (size, w, h) in [("1366x768", 1366.0, 768.0), ("390x844", 390.0, 844.0)] {
        let mut app = with_code(CodeKind::Edit, &wlh, w, h, "en");
        let solve = app.layout().edit.unwrap().solve;
        press(&mut app, solve);
        out.push((
            format!("editor solutions of wlh9000-1 {size}"),
            rest(&mut app),
        ));
    }
    let mut app = with_code(CodeKind::Edit, &dominoes(), 390.0, 844.0, "en");
    assert!(app.tray_pages() > 1);
    out.push(("editor tray pages".to_owned(), rest(&mut app)));
    let next = app.layout().pager.unwrap().next;
    press(&mut app, next);
    out.push(("editor tray page 2".to_owned(), rest(&mut app)));
    for (size, w, h, lang) in [
        ("1366x768", 1366.0, 768.0, "en"),
        ("844x390", 844.0, 390.0, "ua"),
    ] {
        let mut app = with_code(CodeKind::Custom, &wln, w, h, lang);
        out.push((
            format!("custom play wln006-2 {size} {lang}"),
            rest(&mut app),
        ));
    }
    out
}

#[test]
fn drawing() {
    let mut lines = String::new();
    let mut differ = Vec::new();
    let expected = std::fs::read_to_string(golden::golden_path("draw.txt")).unwrap_or_default();
    let expected: Vec<&str> = expected.lines().collect();
    let records = golden::actual_path("draw");
    for (name, writer) in scenes() {
        let line = format!(
            "{name}: {:016x}, {} calls",
            fingerprint(&writer.text),
            writer.calls
        );
        let file = records.join(format!("{}.txt", name.replace(' ', "_")));
        if golden::blessing() {
            golden::write(
                &records.join("blessed").join(file.file_name().unwrap()),
                &writer.text,
            );
        } else if !expected.contains(&line.as_str()) {
            golden::write(&file, &writer.text);
            let before = expected
                .iter()
                .find(|l| l.starts_with(&format!("{name}: ")))
                .copied()
                .unwrap_or("(no such scene)");
            differ.push(format!(
                "  {name}\n    expected: {before}\n    actual:   {line}\n    record:   {}",
                file.display()
            ));
        }
        lines.push_str(&line);
        lines.push('\n');
    }
    if golden::blessing() {
        golden::check("draw.txt", &lines);
        return;
    }
    let removed: Vec<&&str> = expected
        .iter()
        .filter(|l| !lines.lines().any(|a| a == **l))
        .filter(|l| {
            let name = l.split(": ").next().unwrap_or("");
            !lines.lines().any(|a| a.starts_with(&format!("{name}: ")))
        })
        .collect();
    for l in removed {
        differ.push(format!("  scene no longer drawn: {l}"));
    }
    let shown = differ.len().min(10);
    let more = if differ.len() > shown {
        format!(
            "\n  ... and {} more, with records in {}",
            differ.len() - shown,
            records.display()
        )
    } else {
        String::new()
    };
    assert!(
        differ.is_empty(),
        "{} scenes draw differently:\n{}{more}\nexpected: {}\nblessed records, if any: {}\n\
         if the change is intended, run `just bless`",
        differ.len(),
        differ[..shown].join("\n"),
        golden::golden_path("draw.txt").display(),
        records.join("blessed").display()
    );
}
