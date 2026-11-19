//! Tests for the random number generators.
//!
//! The expected values were produced by compiling the reference C code by
//! Blackman and Vigna (`xoshiro256starstar.c` and `splitmix.c` from
//! <https://prng.di.unimi.it/>) and printing its first outputs.

use endlogic_core::rng::{Rng, SplitMix64};

#[test]
fn splitmix64_matches_reference() {
    let mut mix = SplitMix64::new(0);
    let expected = [
        0xe220_a839_7b1d_cdaf,
        0x6e78_9e6a_a1b9_65f4,
        0x06c4_5d18_8009_454f,
        0xf88b_b8a8_724c_81ec,
        0x1b39_896a_51a8_749b,
    ];
    for value in expected {
        assert_eq!(mix.next_u64(), value);
    }
}

#[test]
fn xoshiro256starstar_matches_reference_from_raw_state() {
    let mut rng = Rng::from_state([1, 2, 3, 4]);
    let expected = [
        0x0000_0000_0000_2d00,
        0x0000_0000_0000_0000,
        0x0000_0000_5a00_7080,
        0x10e0_0000_0000_9d80,
        0x10e0_b61c_e100_9d80,
        0x0870_021c_e143_ad00,
        0xe071_c3c2_e143_f089,
        0x75a1_690e_f7a2_0380,
        0x9309_685b_465c_23f9,
        0x284f_3cc2_e13e_3c88,
    ];
    for value in expected {
        assert_eq!(rng.next_u64(), value);
    }
}

#[test]
fn xoshiro256starstar_matches_reference_when_seeded_by_splitmix64() {
    let cases: [(u64, [u64; 6]); 3] = [
        (
            0,
            [
                0x99ec_5f36_cb75_f2b4,
                0xbf6e_1f78_4956_452a,
                0x1a5f_849d_4933_e6e0,
                0x6aa5_94f1_262d_2d2c,
                0xbba5_ad4a_1f84_2e59,
                0xffef_8375_d9eb_caca,
            ],
        ),
        (
            42,
            [
                0x1578_0b2e_0c2e_c716,
                0x6104_d986_6d11_3a7e,
                0xae17_5332_39e4_99a1,
                0xecb8_ad47_03b3_60a1,
                0xfde6_dc7f_e2ec_5e64,
                0xc50d_a531_0179_5238,
            ],
        ),
        (
            u64::MAX,
            [
                0x8f55_20d5_2a7e_ad08,
                0xc476_a018_caa1_802d,
                0x81de_31c0_d260_469e,
                0xbf65_8d7e_065f_3c2f,
                0x9135_93fd_a1bc_a32a,
                0xbb53_5e93_941b_a525,
            ],
        ),
    ];
    for (seed, expected) in cases {
        let mut rng = Rng::seed_from_u64(seed);
        for value in expected {
            assert_eq!(rng.next_u64(), value, "seed {seed}");
        }
    }
}

#[test]
fn below_stays_in_range_and_hits_every_value() {
    let mut rng = Rng::seed_from_u64(7);
    for bound in [1u64, 2, 3, 7, 10, 1000, u64::MAX] {
        for _ in 0..200 {
            assert!(rng.below(bound) < bound);
        }
    }
    let mut seen = [false; 7];
    for _ in 0..500 {
        seen[rng.index(7)] = true;
    }
    assert!(seen.iter().all(|&s| s));
}

#[test]
fn inclusive_ranges_reach_both_ends() {
    let mut rng = Rng::seed_from_u64(11);
    let (mut low, mut high) = (false, false);
    for _ in 0..500 {
        let v = rng.usize_inclusive(3, 6);
        assert!((3..=6).contains(&v));
        low |= v == 3;
        high |= v == 6;
    }
    assert!(low && high);
    assert_eq!(rng.usize_inclusive(5, 5), 5);
    let full = rng.range_inclusive(0, u64::MAX);
    let _ = full;
    assert!(rng.range_inclusive(u64::MAX - 1, u64::MAX) >= u64::MAX - 1);
}

#[test]
fn shuffle_keeps_elements_and_choose_picks_one() {
    let mut rng = Rng::seed_from_u64(3);
    let original: Vec<u32> = (0..50).collect();
    let mut shuffled = original.clone();
    rng.shuffle(&mut shuffled);
    assert_ne!(shuffled, original);
    let mut sorted = shuffled.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, original);

    let mut empty: [u8; 0] = [];
    rng.shuffle(&mut empty);
    assert_eq!(rng.choose(&empty), None);
    let picked = *rng.choose(&original).unwrap();
    assert!(original.contains(&picked));
}

#[test]
fn same_seed_same_sequence_and_streams_differ() {
    let a: Vec<u64> = {
        let mut r = Rng::seed_from_u64(99);
        (0..8).map(|_| r.next_u64()).collect()
    };
    let b: Vec<u64> = {
        let mut r = Rng::seed_from_u64(99);
        (0..8).map(|_| r.next_u64()).collect()
    };
    assert_eq!(a, b);
    let mut s0 = Rng::for_stream(99, 0);
    let mut s1 = Rng::for_stream(99, 1);
    assert_ne!(s0.next_u64(), s1.next_u64());
    assert_eq!(Rng::for_stream(5, 2), Rng::for_stream(5, 2));
}
