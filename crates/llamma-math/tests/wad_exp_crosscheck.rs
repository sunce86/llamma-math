//! Cross-validation of `wad_exp` against an independent Rust implementation.
//!
//! The reference function below is an independent port of the same
//! Remco Bloemen / snekmate `_wad_exp` algorithm. Both implementations
//! were written separately. Any discrepancy indicates a porting bug.

use alloy_primitives::{I256, U256};
use llamma_math::constants::WAD;
use llamma_math::core::wad_exp;

// Independent reference implementation of the same snekmate _wad_exp algorithm.
// Written separately for cross-validation purposes.

fn reference_exp(x: I256) -> Option<I256> {
    // Matches the V1 on-chain implementation (SDIV for all /2^96 divisions).
    if x <= I256::try_from(-41_446_531_673_892_822_313i128).unwrap() {
        return Some(I256::ZERO);
    }

    if x >= I256::from_raw(U256::from(135_305_999_368_893_231_589u128)) {
        return None;
    }

    let c = |v: u128| -> I256 { I256::from_raw(U256::from(v)) };
    let two_96: I256 = c(1u128 << 96);
    let wad: I256 = c(1_000_000_000_000_000_000);

    // x = (power * 2^96) / 10^18 via SDIV
    let mut x = x.wrapping_mul(two_96).wrapping_div(wad);

    // k = round(x / ln(2))
    let ln2_96 = c(54_916_777_467_707_473_351_141_471_128);
    let k: I256 = x
        .wrapping_mul(two_96)
        .wrapping_div(ln2_96)
        .wrapping_add(I256::from_raw(U256::from(2u64).pow(U256::from(95))))
        .wrapping_div(two_96);

    x = x.wrapping_sub(k.wrapping_mul(ln2_96));

    // Rational approximation (all /2^96 via SDIV)
    let mut y = x.wrapping_add(c(1_346_386_616_545_796_478_920_950_773_328));
    y = y
        .wrapping_mul(x)
        .wrapping_div(two_96)
        .wrapping_add(c(57_155_421_227_552_351_082_224_309_758_442));

    let mut p = y
        .wrapping_add(x)
        .wrapping_sub(c(94_201_549_194_550_492_254_356_042_504_812));
    p = p
        .wrapping_mul(y)
        .wrapping_div(two_96)
        .wrapping_add(c(28_719_021_644_029_726_153_956_944_680_412_240));
    p = p
        .wrapping_mul(x)
        .wrapping_add(c(4_385_272_521_454_847_904_659_076_985_693_276).wrapping_shl(96));

    let mut q = x.wrapping_sub(c(2_855_989_394_907_223_263_936_484_059_900));
    q = q
        .wrapping_mul(x)
        .wrapping_div(two_96)
        .wrapping_add(c(50_020_603_652_535_783_019_961_831_881_945));
    q = q
        .wrapping_mul(x)
        .wrapping_div(two_96)
        .wrapping_sub(c(533_845_033_583_426_703_283_633_433_725_380));
    q = q
        .wrapping_mul(x)
        .wrapping_div(two_96)
        .wrapping_add(c(3_604_857_256_930_695_427_073_651_918_091_429));
    q = q
        .wrapping_mul(x)
        .wrapping_div(two_96)
        .wrapping_sub(c(14_423_608_567_350_463_180_887_372_962_807_573));
    q = q
        .wrapping_mul(x)
        .wrapping_div(two_96)
        .wrapping_add(c(26_449_188_498_355_588_339_934_803_723_976_023));

    let r = p.wrapping_div(q);

    // Finalize with scale constant
    // 3_822_833_074_963_236_453_042_738_258_902_158_003_155_416_615_667
    let scale = U256::from_str_radix("29d9dc38563c32e5c2f6dc192ee70ef65f9978af3", 16).unwrap();

    let shift = I256::try_from(195).unwrap().wrapping_sub(k);
    let shift_u: usize = shift.as_i64() as usize;

    let r_uint: U256 = r.into_raw();
    let (product, _) = r_uint.overflowing_mul(scale);
    let result = product >> shift_u;

    Some(I256::from_raw(result))
}

// Cross-validation tests

/// Test that our wad_exp matches the reference for many values.
#[test]
fn crosscheck_wad_exp_positive_range() {
    // Test x from 0 to 130e18 in steps of 1e18
    for i in 0..=130 {
        let x = I256::try_from(i as i128).unwrap() * I256::try_from(WAD).unwrap();

        let our_result = wad_exp(x);
        let ref_result = reference_exp(x);

        match (our_result, ref_result) {
            (Some(ours), Some(theirs)) => {
                assert_eq!(
                    ours,
                    theirs.into_raw(),
                    "mismatch at x = {i}e18: ours = {ours}, reference = {theirs}"
                );
            }
            (None, None) => {} // both overflow, ok
            (ours, theirs) => {
                panic!("disagreement at x = {i}e18: ours = {ours:?}, reference = {theirs:?}");
            }
        }
    }
}

#[test]
fn crosscheck_wad_exp_negative_range() {
    // Test x from -1e18 to -42e18 in steps of 1e18
    for i in 1..=42 {
        let x = I256::try_from(-(i as i128)).unwrap() * I256::try_from(WAD).unwrap();

        let our_result = wad_exp(x);
        let ref_result = reference_exp(x);

        match (our_result, ref_result) {
            (Some(ours), Some(theirs)) => {
                assert_eq!(
                    ours,
                    theirs.into_raw(),
                    "mismatch at x = -{i}e18: ours = {ours}, reference = {theirs}"
                );
            }
            (None, None) => {}
            (ours, theirs) => {
                panic!("disagreement at x = -{i}e18: ours = {ours:?}, reference = {theirs:?}");
            }
        }
    }
}

#[test]
fn crosscheck_wad_exp_fractional_values() {
    // Test fractional inputs: 0.1e18, 0.2e18, ..., 0.9e18
    for i in 1..=9 {
        let x = I256::try_from((i as i128) * 100_000_000_000_000_000).unwrap(); // i * 0.1e18

        let our_result = wad_exp(x).unwrap();
        let ref_result = reference_exp(x).unwrap();

        assert_eq!(
            our_result,
            ref_result.into_raw(),
            "mismatch at x = 0.{i}e18: ours = {our_result}, reference = {ref_result}"
        );
    }

    // Negative fractional
    for i in 1..=9 {
        let x = I256::try_from(-((i as i128) * 100_000_000_000_000_000)).unwrap();

        let our_result = wad_exp(x).unwrap();
        let ref_result = reference_exp(x).unwrap();

        assert_eq!(
            our_result,
            ref_result.into_raw(),
            "mismatch at x = -0.{i}e18: ours = {our_result}, reference = {ref_result}"
        );
    }
}

#[test]
fn crosscheck_wad_exp_small_values() {
    // Very small positive and negative values
    let test_values: Vec<i128> = vec![
        1,
        100,
        1_000_000,
        1_000_000_000,
        1_000_000_000_000,
        -1,
        -100,
        -1_000_000,
        -1_000_000_000,
        -1_000_000_000_000,
    ];

    for v in test_values {
        let x = I256::try_from(v).unwrap();
        let our_result = wad_exp(x).unwrap();
        let ref_result = reference_exp(x).unwrap();

        assert_eq!(
            our_result,
            ref_result.into_raw(),
            "mismatch at x = {v}: ours = {our_result}, reference = {ref_result}"
        );
    }
}

#[test]
fn crosscheck_wad_exp_boundary_values() {
    // Near the zero-return threshold
    let near_threshold: Vec<i128> = vec![
        -41_000_000_000_000_000_000,
        -41_446_531_673_892_822_312, // just above original snekmate threshold
        -42_000_000_000_000_000_000,
        -42_139_678_854_452_767_550, // just above alternative threshold
        -42_139_678_854_452_767_551, // exactly at threshold
        -42_139_678_854_452_767_552, // just below threshold
    ];

    for v in near_threshold {
        let x = I256::try_from(v).unwrap();
        let our_result = wad_exp(x).unwrap();
        let ref_result = reference_exp(x).unwrap();

        assert_eq!(
            our_result,
            ref_result.into_raw(),
            "mismatch at x = {v}: ours = {our_result}, reference = {ref_result}"
        );
    }

    // Near overflow threshold
    let near_overflow = I256::from_raw(U256::from(135_305_999_368_893_231_588u128)); // just below
    let our_result = wad_exp(near_overflow);
    let ref_result = reference_exp(near_overflow);
    assert!(
        our_result.is_some() && ref_result.is_some(),
        "should not overflow just below threshold"
    );
    assert_eq!(
        our_result.unwrap(),
        ref_result.unwrap().into_raw(),
        "mismatch near overflow boundary"
    );
}
