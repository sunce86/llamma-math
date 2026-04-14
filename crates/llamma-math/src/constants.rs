//! Constants ported from Curve LLAMMA `constants.vy` and `AMM.vy`.
//!
//! Source: <https://github.com/curvefi/curve-stablecoin/blob/master/curve_stablecoin/constants.vy>
//! Source: <https://github.com/curvefi/curve-stablecoin/blob/master/curve_stablecoin/AMM.vy>

use alloy_primitives::U256;

/// 1e18 — standard Curve/EVM precision unit.
///
/// `constants.vy`: `WAD: constant(uint256) = 10**18`
pub const WAD: U256 = U256::from_limbs([1_000_000_000_000_000_000, 0, 0, 0]);

/// Maximum number of bands a single user position can span.
///
/// `constants.vy`: `MAX_TICKS: constant(int256) = 50`
pub const MAX_TICKS: i64 = 50;

/// Same as MAX_TICKS but unsigned.
///
/// `constants.vy`: `MAX_TICKS_UINT: constant(uint256) = 50`
pub const MAX_TICKS_UINT: u64 = 50;

/// Maximum number of empty bands the AMM will skip during a swap.
///
/// `constants.vy`: `MAX_SKIP_TICKS: constant(int256) = 1024`
pub const MAX_SKIP_TICKS: i64 = 1024;

/// Same as MAX_SKIP_TICKS but unsigned.
///
/// `constants.vy`: `MAX_SKIP_TICKS_UINT: constant(uint256) = 1024`
pub const MAX_SKIP_TICKS_UINT: u64 = 1024;

/// Oracle price smoothing delay in seconds.
///
/// `AMM.vy`: `PREV_P_O_DELAY: constant(uint256) = 2 * 60  # s = 2 min`
pub const PREV_P_O_DELAY: U256 = U256::from_limbs([120, 0, 0, 0]);

/// Maximum relative oracle price change per update.
/// Approximately `2^(1/3) * 1e18 ≈ 1.2599e18`, ensuring fee < 50%.
///
/// `AMM.vy`: `MAX_P_O_CHG: constant(uint256) = 12500 * 10**14`
pub const MAX_P_O_CHG: U256 = U256::from_limbs([1_250_000_000_000_000_000, 0, 0, 0]);

/// Dead shares constant for initial band deposits.
///
/// `constants.vy`: `DEAD_SHARES: constant(uint256) = 1000`
pub const DEAD_SHARES: U256 = U256::from_limbs([1000, 0, 0, 0]);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wad_is_1e18() {
        assert_eq!(WAD, U256::from(10u64).pow(U256::from(18)));
    }

    #[test]
    fn max_p_o_chg_is_12500e14() {
        assert_eq!(MAX_P_O_CHG, U256::from(12500u64) * U256::from(10u64).pow(U256::from(14)));
    }

    #[test]
    fn prev_p_o_delay_is_120() {
        assert_eq!(PREV_P_O_DELAY, U256::from(120u64));
    }
}
