//! Build a [`LlammaPool`] from raw pool state.
//!
//! The consumer fills in [`RawLlammaState`] from any data source (RPC,
//! substream indexer, database, hardcoded values), then calls [`build_pool`]
//! to get a `LlammaPool` ready for swap computation.

use alloy_primitives::{I256, U256};
use llamma_math::constants::WAD;
use llamma_math::pool::{LlammaPool, PoolError};
use std::collections::HashMap;

/// Raw LLAMMA pool state, data-source agnostic.
/// Populate from RPC, substream, database, or registry.
#[derive(Debug, Clone)]
pub struct RawLlammaState {
    pub a: U256,
    /// `ln(A / (A-1)) * 1e18`
    pub log_a_ratio: I256,
    /// `10^(18 - collateral_decimals)`
    pub collateral_precision: U256,
    /// `10^(18 - borrowed_decimals)`
    pub borrowed_precision: U256,
    pub static_antifee: bool,
    pub fee: U256,
    pub active_band: i64,
    pub min_band: i64,
    pub max_band: i64,
    pub p_oracle: U256,
    /// `BASE_PRICE * rate_mul / 1e18`
    pub base_price: U256,
    /// Dynamic fee from oracle limiting. `0` if ≤ static fee.
    pub oracle_fee: U256,
    pub bands_x: HashMap<i64, U256>,
    pub bands_y: HashMap<i64, U256>,
}

/// Build a `LlammaPool` from raw state. Pure, no I/O.
pub fn build_pool(state: &RawLlammaState) -> Result<LlammaPool, PoolError> {
    if state.a <= U256::from(1u64) {
        return Err(PoolError::InvalidParams);
    }
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
