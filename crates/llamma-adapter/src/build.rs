//! Build a [`LlammaPool`] from raw pool state.
//!
//! The consumer fills in [`RawLlammaState`] from any data source (RPC,
//! substream indexer, database, hardcoded values), then calls [`build_pool`]
//! to get a `LlammaPool` ready for swap computation.

use alloy_primitives::{I256, U256};
use llamma_math::constants::WAD;
use llamma_math::pool::LlammaPool;
use std::collections::HashMap;

/// Raw on-chain state of a LLAMMA pool, ready to be converted into a
/// [`LlammaPool`] via [`build_pool`].
///
/// This struct is data-source agnostic — it can be populated from RPC calls,
/// a substream indexer, a database snapshot, or hardcoded test values.
/// All fields are flat (no nested structs) so any data source can fill them.
#[derive(Debug, Clone)]
pub struct RawLlammaState {
    /// Amplification parameter (`A()` view function).
    pub a: U256,
    /// `ln(A / (A - 1)) * 1e18`. Read from bytecode immutables or registry.
    pub log_a_ratio: I256,
    /// `10^(18 - collateral_decimals)`.
    pub collateral_precision: U256,
    /// `10^(18 - borrowed_decimals)`.
    pub borrowed_precision: U256,
    /// Whether the contract uses static antifee (Vyper 0.3.x) or per-band (0.4.x).
    pub static_antifee: bool,
    /// Static fee parameter (`fee()` view function).
    pub fee: U256,
    /// Current active band.
    pub active_band: i64,
    /// Lowest non-empty band.
    pub min_band: i64,
    /// Highest non-empty band.
    pub max_band: i64,
    /// Oracle price (limited by `limit_p_o`).
    pub p_oracle: U256,
    /// Base price (includes `rate_mul`).
    pub base_price: U256,
    /// Dynamic fee from oracle limiting. `0` if ≤ static fee.
    pub oracle_fee: U256,
    /// Non-zero borrowed token (x) amounts per band.
    pub bands_x: HashMap<i64, U256>,
    /// Non-zero collateral token (y) amounts per band.
    pub bands_y: HashMap<i64, U256>,
}

/// Build a [`LlammaPool`] from raw state. Pure function — no I/O.
pub fn build_pool(state: &RawLlammaState) -> LlammaPool {
    let a = state.a;
    let a_minus_1 = a - U256::from(1u64);

    let mut max_oracle_dn_pow = WAD;
    for _ in 0..50 {
        max_oracle_dn_pow = max_oracle_dn_pow * a / a_minus_1;
    }

    LlammaPool::new(
        a,
        a_minus_1,
        state.base_price,
        state.log_a_ratio,
        max_oracle_dn_pow,
        U256::ZERO,
        state.borrowed_precision,
        state.collateral_precision,
        state.fee,
        state.active_band,
        state.min_band,
        state.max_band,
        state.bands_x.clone(),
        state.bands_y.clone(),
        state.p_oracle,
        state.oracle_fee,
        state.static_antifee,
    )
}
