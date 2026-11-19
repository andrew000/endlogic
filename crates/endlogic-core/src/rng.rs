//! Deterministic pseudo-random numbers: xoshiro256** seeded through
//! `SplitMix64`.
//!
//! Both algorithms follow the public-domain reference code by David Blackman
//! and Sebastiano Vigna (<https://prng.di.unimi.it/>). All arithmetic is on
//! `u64`, so a given seed produces the same sequence on every platform.

/// `SplitMix64`, used to expand a single `u64` seed into generator state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// Creates a generator with the given state.
    #[must_use]
    pub fn new(seed: u64) -> SplitMix64 {
        SplitMix64 { state: seed }
    }

    /// Returns the next output.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}

/// The xoshiro256** 1.0 generator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rng {
    s: [u64; 4],
}

impl Rng {
    /// Seeds the generator by filling its state with four `SplitMix64` outputs,
    /// as the authors recommend.
    #[must_use]
    pub fn seed_from_u64(seed: u64) -> Rng {
        let mut mix = SplitMix64::new(seed);
        Rng {
            s: [
                mix.next_u64(),
                mix.next_u64(),
                mix.next_u64(),
                mix.next_u64(),
            ],
        }
    }

    /// Creates a generator from raw state. The state must not be all zero;
    /// an all-zero state is replaced by the state for seed 0.
    #[must_use]
    pub fn from_state(state: [u64; 4]) -> Rng {
        if state == [0; 4] {
            Rng::seed_from_u64(0)
        } else {
            Rng { s: state }
        }
    }

    /// Derives an independent generator for stream number `stream` of
    /// `seed`, for example one stream per generator attempt.
    #[must_use]
    pub fn for_stream(seed: u64, stream: u64) -> Rng {
        let mut mix = SplitMix64::new(stream);
        Rng::seed_from_u64(seed ^ mix.next_u64())
    }

    /// Returns the next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        let s = &mut self.s;
        let result = s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = s[1] << 17;
        s[2] ^= s[0];
        s[3] ^= s[1];
        s[1] ^= s[2];
        s[0] ^= s[3];
        s[2] ^= t;
        s[3] = s[3].rotate_left(45);
        result
    }

    /// Returns a uniformly distributed integer in `0..bound`, without
    /// modulo bias (Lemire's multiply-and-reject method).
    ///
    /// # Panics
    ///
    /// Panics if `bound == 0`.
    // The casts split a 128-bit product into its two 64-bit halves.
    #[allow(clippy::cast_possible_truncation)]
    pub fn below(&mut self, bound: u64) -> u64 {
        assert!(bound > 0, "bound must be positive");
        let threshold = bound.wrapping_neg() % bound;
        loop {
            let product = u128::from(self.next_u64()) * u128::from(bound);
            let low = product as u64;
            if low >= threshold {
                return (product >> 64) as u64;
            }
        }
    }

    /// Returns a uniformly distributed integer in `min..=max`.
    ///
    /// # Panics
    ///
    /// Panics if `min > max`.
    pub fn range_inclusive(&mut self, min: u64, max: u64) -> u64 {
        assert!(min <= max, "empty range {min}..={max}");
        match (max - min).checked_add(1) {
            Some(span) => min + self.below(span),
            None => self.next_u64(),
        }
    }

    /// Returns a uniformly distributed index in `0..len`.
    ///
    /// # Panics
    ///
    /// Panics if `len == 0`.
    pub fn index(&mut self, len: usize) -> usize {
        let len = u64::try_from(len).expect("usize fits in u64");
        usize::try_from(self.below(len)).expect("value below a usize fits in usize")
    }

    /// Returns a uniformly distributed `usize` in `min..=max`.
    ///
    /// # Panics
    ///
    /// Panics if `min > max`.
    pub fn usize_inclusive(&mut self, min: usize, max: usize) -> usize {
        let to_u64 = |v: usize| u64::try_from(v).expect("usize fits in u64");
        let value = self.range_inclusive(to_u64(min), to_u64(max));
        usize::try_from(value).expect("value within usize bounds fits in usize")
    }

    /// Shuffles a slice uniformly (Fisher–Yates).
    pub fn shuffle<T>(&mut self, slice: &mut [T]) {
        for i in (1..slice.len()).rev() {
            let j = self.index(i + 1);
            slice.swap(i, j);
        }
    }

    /// Returns a uniformly chosen element, or `None` for an empty slice.
    pub fn choose<'a, T>(&mut self, slice: &'a [T]) -> Option<&'a T> {
        if slice.is_empty() {
            None
        } else {
            Some(&slice[self.index(slice.len())])
        }
    }

    /// Returns `true` with probability `numerator / denominator`.
    ///
    /// # Panics
    ///
    /// Panics if `denominator == 0`.
    pub fn chance(&mut self, numerator: u64, denominator: u64) -> bool {
        self.below(denominator) < numerator
    }
}
