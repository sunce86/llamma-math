//! Constants ported from Curve LLAMMA `constants.vy` and `AMM.vy`.
//!
//! Source: <https://github.com/curvefi/curve-stablecoin/blob/master/curve_stablecoin/constants.vy>
//! Source: <https://github.com/curvefi/curve-stablecoin/blob/master/curve_stablecoin/AMM.vy>

use alloy_primitives::U256;

/// 1e18
pub const WAD: U256 = U256::from_limbs([1_000_000_000_000_000_000, 0, 0, 0]);

/// Max bands per user position.
pub const MAX_TICKS: i64 = 50;
pub const MAX_TICKS_UINT: u64 = 50;

/// Max empty bands to skip during swap.
pub const MAX_SKIP_TICKS: i64 = 1024;
pub const MAX_SKIP_TICKS_UINT: u64 = 1024;

/// Oracle EMA smoothing delay (seconds).
pub const PREV_P_O_DELAY: U256 = U256::from_limbs([120, 0, 0, 0]);

/// Max oracle price change per update (`12500 * 10^14`).
pub const MAX_P_O_CHG: U256 = U256::from_limbs([1_250_000_000_000_000_000, 0, 0, 0]);

/// Dead shares for initial band deposits.
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
        assert_eq!(
            MAX_P_O_CHG,
            U256::from(12500u64) * U256::from(10u64).pow(U256::from(14))
        );
    }

    #[test]
    fn prev_p_o_delay_is_120() {
        assert_eq!(PREV_P_O_DELAY, U256::from(120u64));
    }
}
