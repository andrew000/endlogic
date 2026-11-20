//! The little floating-point math the trainer needs, without the standard
//! library: rounding, and sine and cosine for the turning pieces.

use core::f32::consts::FRAC_PI_2;

/// Above this every `f32` is a whole number.
const WHOLE: f32 = 8_388_608.0;

/// `x` without its fraction, toward zero.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn trunc(x: f32) -> f32 {
    if x.abs() < WHOLE {
        // Exact: the value fits an `i32`, and so does its whole part.
        (x as i32) as f32
    } else {
        // Already whole, infinite or not a number.
        x
    }
}

/// The largest whole number not above `x`, as `f32::floor`.
pub(crate) fn floor(x: f32) -> f32 {
    let t = trunc(x);
    if t > x { t - 1.0 } else { t }
}

/// The nearest whole number, halves away from zero, as `f32::round`.
pub(crate) fn round(x: f32) -> f32 {
    let t = trunc(x);
    let rest = x - t;
    if rest >= 0.5 {
        t + 1.0
    } else if rest <= -0.5 {
        t - 1.0
    } else {
        t
    }
}

/// Sine and cosine of `x` radians, within a few units in the last place
/// for the angles the trainer uses (a few turns at most).
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn sin_cos(x: f32) -> (f32, f32) {
    // x = turns·π/2 + rest with |rest| ≤ π/4.
    let turns = round(x / FRAC_PI_2);
    let rest = x - turns * FRAC_PI_2;
    let sq = rest * rest;
    let sine = rest * (1.0 - sq / 6.0 * (1.0 - sq / 20.0 * (1.0 - sq / 42.0 * (1.0 - sq / 72.0))));
    let cosine = 1.0 - sq / 2.0 * (1.0 - sq / 12.0 * (1.0 - sq / 30.0 * (1.0 - sq / 56.0)));
    match (turns as i32).rem_euclid(4) {
        0 => (sine, cosine),
        1 => (cosine, -sine),
        2 => (-sine, -cosine),
        _ => (-cosine, sine),
    }
}

/// Sine of `x` radians.
pub(crate) fn sin(x: f32) -> f32 {
    sin_cos(x).0
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;

    /// Many values between `-limit` and `limit`, halves and edges included.
    fn values(limit: f32) -> impl Iterator<Item = f32> {
        (-40_000..=40_000).map(move |i| {
            #[allow(clippy::cast_precision_loss)]
            let t = i as f32 / 40_000.0;
            t * limit
        })
    }

    #[test]
    fn rounding_matches_the_standard_library() {
        for x in values(1000.0).chain([0.5, -0.5, 1.5, -1.5, 2.5, -2.5, 1e9, -1e9, 0.0]) {
            assert_eq!(floor(x), std::primitive::f32::floor(x), "floor {x}");
            assert_eq!(round(x), std::primitive::f32::round(x), "round {x}");
        }
        assert!(floor(f32::NAN).is_nan());
    }

    #[test]
    fn sine_and_cosine_are_close_to_the_standard_library() {
        let mut worst = 0.0f32;
        for x in values(6.0 * core::f32::consts::TAU) {
            let (s, c) = sin_cos(x);
            let (ws, wc) = std::primitive::f32::sin_cos(x);
            worst = worst.max((s - ws).abs()).max((c - wc).abs());
        }
        assert!(worst < 1e-5, "worst {worst}");
        std::println!("worst difference {worst:e}");
    }
}
