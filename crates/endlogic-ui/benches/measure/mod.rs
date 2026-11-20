//! A small time measurer for the benchmarks, without outside crates.
//!
//! A case runs in batches long enough for the clock to be exact; the
//! batches go on until the case has run for a while. The result is the time
//! of one run: the mean over all batches and the best batch.
#![allow(dead_code)]

use std::hint::black_box;
use std::time::{Duration, Instant};

/// How long one case is run, at least.
const CASE_TIME: Duration = Duration::from_millis(150);
/// How long one batch takes, at least.
const BATCH_TIME: Duration = Duration::from_millis(2);

/// The time of one run.
#[derive(Debug, Clone, Copy)]
pub struct Timing {
    /// Mean over all batches.
    pub mean: Duration,
    /// The best batch.
    pub best: Duration,
    /// Runs made, warm-up excluded.
    pub runs: u64,
}

/// Times `f`.
pub fn time<R>(mut f: impl FnMut() -> R) -> Timing {
    black_box(f());
    let mut batch = 1u64;
    loop {
        let start = Instant::now();
        for _ in 0..batch {
            black_box(f());
        }
        if start.elapsed() >= BATCH_TIME || batch >= 1 << 30 {
            break;
        }
        batch *= 2;
    }
    let started = Instant::now();
    let mut runs = 0u64;
    let mut total = Duration::ZERO;
    let mut best = Duration::MAX;
    while runs == 0 || started.elapsed() < CASE_TIME {
        let start = Instant::now();
        for _ in 0..batch {
            black_box(f());
        }
        let took = start.elapsed();
        total += took;
        runs += batch;
        best = best.min(took / u32::try_from(batch).unwrap_or(u32::MAX));
    }
    Timing {
        mean: total / u32::try_from(runs).unwrap_or(u32::MAX),
        best,
        runs,
    }
}

/// A duration with a unit that keeps three or four digits.
pub fn show(d: Duration) -> String {
    let ns = d.as_secs_f64() * 1e9;
    if ns < 1e3 {
        format!("{ns:.0} ns")
    } else if ns < 1e6 {
        format!("{:.1} µs", ns / 1e3)
    } else if ns < 1e9 {
        format!("{:.2} ms", ns / 1e6)
    } else {
        format!("{:.2} s", ns / 1e9)
    }
}

/// Nodes per second, in millions.
pub fn rate(nodes: u64, d: Duration) -> String {
    #[allow(clippy::cast_precision_loss)]
    let per_second = nodes as f64 / d.as_secs_f64().max(1e-12);
    format!("{:.2} M nodes/s", per_second / 1e6)
}

/// A table printed as Markdown.
pub struct Table {
    title: String,
    head: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl Table {
    /// A table with a title and column names.
    pub fn new(title: &str, head: &[&str]) -> Table {
        Table {
            title: title.to_owned(),
            head: head.iter().map(|h| (*h).to_owned()).collect(),
            rows: Vec::new(),
        }
    }

    /// Adds a row.
    pub fn row(&mut self, cells: Vec<String>) {
        self.rows.push(cells);
    }

    /// Prints the table.
    pub fn print(&self) {
        let widths: Vec<usize> = (0..self.head.len())
            .map(|i| {
                self.rows
                    .iter()
                    .map(|r| r[i].chars().count())
                    .chain([self.head[i].chars().count()])
                    .max()
                    .unwrap_or(0)
            })
            .collect();
        let line = |cells: &[String]| {
            let parts: Vec<String> = cells
                .iter()
                .zip(&widths)
                .map(|(c, &w)| format!("{c}{}", " ".repeat(w - c.chars().count())))
                .collect();
            println!("| {} |", parts.join(" | "));
        };
        println!("\n### {}\n", self.title);
        line(&self.head);
        let rule: Vec<String> = widths.iter().map(|&w| "-".repeat(w)).collect();
        println!("|-{}-|", rule.join("-|-"));
        for row in &self.rows {
            line(row);
        }
    }
}
