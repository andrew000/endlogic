//! The "Numbers" display: counters drawn as colored digits instead of ticks.
//! Digits are not read; seeing them is enough to tell the player to switch
//! the game to "Grid".

use alloc::vec::Vec;

use crate::blobs::Blob;

/// Whether colored glyphs of one height stand both in a row (above the
/// columns) and in a column (left of the rows).
pub(crate) fn numbers(blobs: &[Blob]) -> bool {
    let glyphs: Vec<&Blob> = blobs
        .iter()
        .filter(|b| {
            let ratio = b.h() / b.w();
            b.h() >= 8.0 && (1.1..=6.0).contains(&ratio)
        })
        .take(2000)
        .collect();
    // Free-standing: digits in colored boxes are endlogic's editor.
    let glyphs: Vec<&Blob> = glyphs
        .into_iter()
        .filter(|g| !blobs.iter().any(|o| o.area > g.area && o.contains(g)))
        .collect();
    // Glyphs of one height on one baseline (above the columns) and glyphs
    // of that height ending at one x (left of the rows).
    let like = |g: &Blob, o: &Blob| (o.h() / g.h() - 1.0).abs() <= 0.15;
    let in_row = |g: &Blob| {
        glyphs
            .iter()
            .filter(|o| like(g, o) && (o.bottom() - g.bottom()).abs() <= 0.15 * g.h())
            .count()
            >= 3
    };
    let in_col = |g: &Blob| {
        glyphs
            .iter()
            .filter(|o| like(g, o) && (o.right() - g.right()).abs() <= 0.6 * g.h())
            .count()
            >= 3
    };
    // Where the two meet is the grid's top-left corner; endlogic frames it
    // with a colored bracket, the game marks it in grey.
    let bracket = |x: f32, y: f32, h: f32| {
        blobs.iter().any(|b| {
            let (bx, by) = (b.right() - b.w(), b.bottom() - b.h());
            bx >= x
                && bx <= x + 2.0 * h
                && by >= y
                && by <= y + 2.0 * h
                && b.w() >= 0.5 * h
                && b.h() >= 0.5 * h
                && b.fill() < 0.5
        })
    };
    glyphs.iter().any(|g| {
        in_row(g)
            && glyphs.iter().any(|o| {
                like(g, o) && in_col(o) && o.x1 < g.x0 && !bracket(o.right(), g.bottom(), g.h())
            })
    })
}
