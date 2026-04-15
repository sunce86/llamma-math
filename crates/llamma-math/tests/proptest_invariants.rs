//! Property-based tests for LLAMMA pool invariants.
//!
//! Uses proptest to generate random pool states and verify that
//! fundamental mathematical invariants hold for all inputs.

use alloy_primitives::{I256, U256};
use llamma_math::constants::WAD;
use llamma_math::pool::LlammaPool;
use proptest::prelude::*;
use std::collections::HashMap;

/// Registry of (A, log_a_ratio) pairs from real deployments.
const A_CONFIGS: &[(u64, i128)] = &[
    (2, 693_147_180_559_945_309),  // theoretical minimum
    (10, 105_360_515_657_826_298), // UwU
    (30, 33_901_551_675_681_339),  // CRV
    (70, 14_388_737_452_099_598),  // wstETH Llamalend
    (100, 10_050_335_853_501_431), // crvUSD mint
    (200, 5_012_541_823_544_273),  // sUSDe
    (500, 2_002_002_670_673_068),  // USDe
];

fn arb_a_config() -> impl Strategy<Value = (u64, i128)> {
    prop::sample::select(A_CONFIGS)
}

fn arb_band_balance() -> impl Strategy<Value = U256> {
    prop_oneof![
        Just(U256::ZERO),
        (1u64..=1_000_000).prop_map(|v| WAD * U256::from(v)),
    ]
}

fn arb_price() -> impl Strategy<Value = U256> {
    // prices from $0.01 to $100,000 in WAD
    (1u64..=10_000_000).prop_map(|v| WAD * U256::from(v) / U256::from(100u64))
}

fn arb_fee() -> impl Strategy<Value = U256> {
    prop_oneof![
        Just(U256::ZERO),
        // 0.01% to 5%
        (1u64..=500).prop_map(|bps| U256::from(bps) * WAD / U256::from(10000u64)),
    ]
}

fn arb_pool() -> impl Strategy<Value = LlammaPool> {
    (
        arb_a_config(),
        arb_price(),
        arb_fee(),
        prop::bool::ANY,
        // 3 bands around active_band=0: -1, 0, 1
        arb_band_balance(),
        arb_band_balance(),
        arb_band_balance(),
        arb_band_balance(),
        arb_band_balance(),
        arb_band_balance(),
    )
        .prop_filter_map(
            "valid pool",
            |(
                (a, log_a_ratio),
                base_price,
                fee,
                static_antifee,
                bx_neg1,
                bx_0,
                bx_1,
                by_neg1,
                by_0,
                by_1,
            )| {
                let a_val = U256::from(a);
                let a_minus_1 = U256::from(a - 1);
                let mut pow = WAD;
                for _ in 0..50 {
                    pow = pow * a_val / a_minus_1;
                }

                let mut bands_x = HashMap::new();
                let mut bands_y = HashMap::new();
                if !bx_neg1.is_zero() {
                    bands_x.insert(-1, bx_neg1);
                }
                if !bx_0.is_zero() {
                    bands_x.insert(0, bx_0);
                }
                if !bx_1.is_zero() {
                    bands_x.insert(1, bx_1);
                }
                if !by_neg1.is_zero() {
                    bands_y.insert(-1, by_neg1);
                }
                if !by_0.is_zero() {
                    bands_y.insert(0, by_0);
                }
                if !by_1.is_zero() {
                    bands_y.insert(1, by_1);
                }

                LlammaPool::new(
                    a_val,
                    a_minus_1,
                    base_price,
                    I256::try_from(log_a_ratio).unwrap(),
                    pow,
                    U256::ZERO,
                    U256::from(1u64),
                    U256::from(1u64),
                    fee,
                    0,
                    -1,
                    1,
                    bands_x,
                    bands_y,
                    base_price,
                    U256::ZERO,
                    static_antifee,
                )
                .ok()
            },
        )
}

fn arb_dx() -> impl Strategy<Value = U256> {
    prop_oneof![
        Just(U256::from(1u64)),
        (1u64..=1_000_000).prop_map(|v| WAD * U256::from(v)),
        Just(WAD / U256::from(1000u64)),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn zero_input_zero_output(pool in arb_pool()) {
        prop_assert_eq!(pool.get_amount_out(0, 1, U256::ZERO).unwrap(), U256::ZERO);
        prop_assert_eq!(pool.get_amount_out(1, 0, U256::ZERO).unwrap(), U256::ZERO);
    }

    #[test]
    fn monotonicity_pump(pool in arb_pool(), small in arb_dx(), factor in 2u64..=100) {
        let large = small * U256::from(factor);
        let dy_small = pool.get_amount_out(0, 1, small);
        let dy_large = pool.get_amount_out(0, 1, large);
        if let (Ok(s), Ok(l)) = (dy_small, dy_large) {
            prop_assert!(l >= s, "monotonicity violated: dx={small} → {s}, dx={large} → {l}");
        }
    }

    #[test]
    fn monotonicity_dump(pool in arb_pool(), small in arb_dx(), factor in 2u64..=100) {
        let large = small * U256::from(factor);
        let dy_small = pool.get_amount_out(1, 0, small);
        let dy_large = pool.get_amount_out(1, 0, large);
        if let (Ok(s), Ok(l)) = (dy_small, dy_large) {
            prop_assert!(l >= s, "monotonicity violated: dx={small} → {s}, dx={large} → {l}");
        }
    }

    #[test]
    fn output_bounded_by_liquidity_pump(pool in arb_pool(), dx in arb_dx()) {
        if let Ok(dy) = pool.get_amount_out(0, 1, dx) {
            // Pump (0→1): output is collateral (y), bounded by total bands_y
            let total_y: U256 = pool.bands_y.values().sum();
            prop_assert!(dy <= total_y, "output {dy} exceeds total liquidity {total_y}");
        }
    }

    #[test]
    fn output_bounded_by_liquidity_dump(pool in arb_pool(), dx in arb_dx()) {
        if let Ok(dy) = pool.get_amount_out(1, 0, dx) {
            // Dump (1→0): output is borrowed (x), bounded by total bands_x
            let total_x: U256 = pool.bands_x.values().sum();
            prop_assert!(dy <= total_x, "output {dy} exceeds total liquidity {total_x}");
        }
    }

    #[test]
    fn fee_reduces_output(
        (a, log_a_ratio) in arb_a_config(),
        base_price in arb_price(),
        dx in arb_dx(),
    ) {
        let a_val = U256::from(a);
        let a_minus_1 = U256::from(a - 1);
        let mut pow = WAD;
        for _ in 0..50 {
            pow = pow * a_val / a_minus_1;
        }

        let mut bands_x = HashMap::new();
        let mut bands_y = HashMap::new();
        bands_x.insert(0i64, WAD * U256::from(1000u64));
        bands_y.insert(0i64, WAD * U256::from(10u64));

        let log_a = I256::try_from(log_a_ratio).unwrap();

        let pool_no_fee = LlammaPool::new(
            a_val, a_minus_1, base_price, log_a, pow, U256::ZERO,
            U256::from(1u64), U256::from(1u64),
            U256::ZERO, // no fee
            0, -1, 1,
            bands_x.clone(), bands_y.clone(),
            base_price, U256::ZERO, false,
        );

        let pool_with_fee = LlammaPool::new(
            a_val, a_minus_1, base_price, log_a, pow, U256::ZERO,
            U256::from(1u64), U256::from(1u64),
            WAD / U256::from(100u64), // 1% fee
            0, -1, 1,
            bands_x, bands_y,
            base_price, U256::ZERO, false,
        );

        if let (Ok(pool_nf), Ok(pool_wf)) = (pool_no_fee, pool_with_fee) {
            if let (Ok(dy_no_fee), Ok(dy_with_fee)) = (
                pool_nf.get_amount_out(0, 1, dx),
                pool_wf.get_amount_out(0, 1, dx),
            ) {
                prop_assert!(
                    dy_with_fee <= dy_no_fee,
                    "fee should reduce output: no_fee={dy_no_fee}, with_fee={dy_with_fee}"
                );
            }
        }
    }
}
