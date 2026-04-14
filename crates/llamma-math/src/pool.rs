//! `LlammaPool` — high-level LLAMMA pool interface.
//!
//! Mirrors the on-chain `get_dy` / `get_p` public functions,
//! handling precision scaling and direction mapping internally.
//!
//! # Token indexing
//!
//! Matches the on-chain convention:
//! - `i = 0` — borrowed token (e.g. crvUSD)
//! - `i = 1` — collateral token (e.g. WETH)
//!
//! # Precision
//!
//! Amounts passed to and returned from `get_amount_out` / `spot_price`
//! are in **native token decimals** (e.g. 1 USDC = 1_000_000,
//! 1 WETH = 1_000_000_000_000_000_000). Internal scaling is handled
//! automatically using `borrowed_precision` and `collateral_precision`.

use alloy_primitives::{I256, U256};
use std::collections::HashMap;

use crate::core::{get_p, p_oracle_up};
use crate::swap::{self, SwapState};

/// Error returned by LlammaPool methods.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PoolError {
    /// Invalid token index (must be 0 or 1).
    InvalidIndex,
    /// Invalid pool parameters (e.g. A ≤ 1, zero precision).
    InvalidParams,
    /// Math error during computation.
    MathError,
}

impl std::fmt::Display for PoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidIndex => f.write_str("invalid token index: must be 0 or 1"),
            Self::InvalidParams => f.write_str("invalid pool parameters"),
            Self::MathError => f.write_str("math error during computation"),
        }
    }
}

impl std::error::Error for PoolError {}

/// LLAMMA pool state snapshot at a specific block.
///
/// All fields correspond to on-chain storage variables in `AMM.vy`.
/// Oracle price is an external input (read separately from the oracle contract).
///
/// Construct via [`llamma_adapter::build_pool`] or [`LlammaPool::new`].
#[derive(Debug, Clone)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LlammaPool {
    /// Amplification parameter.
    ///
    /// `AMM.vy`: `A: public(immutable(uint256))`
    pub a: U256,

    /// `A - 1`. Precomputed to avoid repeated subtraction.
    ///
    /// `AMM.vy`: `Aminus1: immutable(uint256)`
    pub a_minus_1: U256,

    /// Base price — price corresponding to band 0.
    /// Already adjusted for `rate_mul` (i.e. `BASE_PRICE * rate_mul / 1e18`).
    ///
    /// `AMM.vy`: `self._base_price()` = `BASE_PRICE * self._rate_mul() / 10**18`
    pub base_price: U256,

    /// `ln(A / (A - 1)) * 1e18`. Precomputed, passed to constructor.
    ///
    /// `AMM.vy`: `LOG_A_RATIO: immutable(int256)`
    pub log_a_ratio: I256,

    /// `(A / (A - 1))^50`. Precomputed, used for band traversal bounds.
    ///
    /// `AMM.vy`: `MAX_ORACLE_DN_POW: immutable(uint256)`
    pub max_oracle_dn_pow: U256,

    /// `sqrt(A / (A - 1)) * 1e18`. Precomputed.
    ///
    /// `AMM.vy`: `SQRT_BAND_RATIO: immutable(uint256)`
    pub sqrt_band_ratio: U256,

    /// Precision multiplier for borrowed token: `10^(18 - borrowed_decimals)`.
    ///
    /// `AMM.vy`: `BORROWED_PRECISION: immutable(uint256)`
    pub borrowed_precision: U256,

    /// Precision multiplier for collateral token: `10^(18 - collateral_decimals)`.
    ///
    /// `AMM.vy`: `COLLATERAL_PRECISION: immutable(uint256)`
    pub collateral_precision: U256,

    /// Static fee parameter.
    ///
    /// `AMM.vy`: `fee: public(uint256)`
    pub fee: U256,

    /// Current active band.
    ///
    /// `AMM.vy`: `active_band: public(int256)`
    pub active_band: i64,

    /// Lowest non-empty band.
    ///
    /// `AMM.vy`: `min_band: public(int256)`
    pub min_band: i64,

    /// Highest non-empty band.
    ///
    /// `AMM.vy`: `max_band: public(int256)`
    pub max_band: i64,

    /// Borrowed token (x) amounts per band.
    ///
    /// `AMM.vy`: `bands_x: public(HashMap[int256, uint256])`
    pub bands_x: HashMap<i64, U256>,

    /// Collateral token (y) amounts per band.
    ///
    /// `AMM.vy`: `bands_y: public(HashMap[int256, uint256])`
    pub bands_y: HashMap<i64, U256>,

    /// Oracle price, already limited by `limit_p_o`.
    /// This is `p_o[0]` from `_price_oracle_ro()`.
    pub p_oracle: U256,

    /// Dynamic fee component from oracle price limiting.
    /// This is `p_o[1]` from `_price_oracle_ro()`.
    pub oracle_fee: U256,

    /// Whether to use static antifee (computed once before the loop)
    /// or dynamic antifee (recomputed per band via `get_dynamic_fee`).
    ///
    /// - `true` — crvUSD mint markets (Vyper 0.3.7): antifee is static.
    /// - `false` — Llamalend markets (Vyper 0.4.x): antifee is per-band.
    pub static_antifee: bool,
}

impl LlammaPool {
    /// Create a new `LlammaPool` from all required parameters.
    ///
    /// Returns `Err(PoolError::InvalidParams)` if `a <= 1` or precision values are zero.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        a: U256,
        a_minus_1: U256,
        base_price: U256,
        log_a_ratio: I256,
        max_oracle_dn_pow: U256,
        sqrt_band_ratio: U256,
        borrowed_precision: U256,
        collateral_precision: U256,
        fee: U256,
        active_band: i64,
        min_band: i64,
        max_band: i64,
        bands_x: HashMap<i64, U256>,
        bands_y: HashMap<i64, U256>,
        p_oracle: U256,
        oracle_fee: U256,
        static_antifee: bool,
    ) -> Result<Self, PoolError> {
        if a <= U256::from(1u64)
            || a_minus_1.is_zero()
            || borrowed_precision.is_zero()
            || collateral_precision.is_zero()
        {
            return Err(PoolError::InvalidParams);
        }
        Ok(Self {
            a,
            a_minus_1,
            base_price,
            log_a_ratio,
            max_oracle_dn_pow,
            sqrt_band_ratio,
            borrowed_precision,
            collateral_precision,
            fee,
            active_band,
            min_band,
            max_band,
            bands_x,
            bands_y,
            p_oracle,
            oracle_fee,
            static_antifee,
        })
    }

    /// Compute amount of token `j` received for swapping `dx` of token `i`.
    ///
    /// Mirrors `AMM.vy::get_dy(i, j, in_amount)`.
    ///
    /// # Arguments
    /// * `i` — input token index (0 = borrowed, 1 = collateral)
    /// * `j` — output token index
    /// * `dx` — input amount in native token decimals
    ///
    /// # Returns
    /// Output amount in native token decimals, or error.
    pub fn get_amount_out(&self, i: usize, j: usize, dx: U256) -> Result<U256, PoolError> {
        // AMM.vy L943: assert (i == 0 and j == 1) or (i == 1 and j == 0)
        if !((i == 0 && j == 1) || (i == 1 && j == 0)) {
            return Err(PoolError::InvalidIndex);
        }

        // AMM.vy L945: if amount == 0: return empty
        if dx.is_zero() {
            return Ok(U256::ZERO);
        }

        // AMM.vy L947-951: determine precisions based on direction
        let (in_precision, out_precision) = if i == 0 {
            // i=0: borrowable in → in_precision = BORROWED_PRECISION
            (self.borrowed_precision, self.collateral_precision)
        } else {
            // i=1: collateral in → in_precision = COLLATERAL_PRECISION
            (self.collateral_precision, self.borrowed_precision)
        };

        // AMM.vy L952: p_o = self._price_oracle_ro()
        // Already provided as self.p_oracle and self.oracle_fee

        // AMM.vy L953-954: pump = (i == 0), scale input
        let pump = i == 0;
        let scaled_in = dx * in_precision;

        // Compute p_oracle_up for active band
        let p_o_up = self.compute_p_oracle_up(self.active_band)?;

        // Build SwapState
        let state = SwapState {
            a: self.a,
            a_minus_1: self.a_minus_1,
            fee: self.fee,
            bands_x: &self.bands_x,
            bands_y: &self.bands_y,
            active_band: self.active_band,
            min_band: self.min_band,
            max_band: self.max_band,
            p_oracle: self.p_oracle,
            oracle_fee: self.oracle_fee,
            p_oracle_up_active: p_o_up,
            max_oracle_dn_pow: self.max_oracle_dn_pow,
            in_precision,
            out_precision,
            static_antifee: self.static_antifee,
        };

        // AMM.vy L954: out = self.calc_swap_out(i == 0, amount * in_precision, p_o, ...)
        let result = swap::calc_swap_out(pump, scaled_in, &state).ok_or(PoolError::MathError)?;

        // AMM.vy L958: out.out_amount = unsafe_div(out.out_amount, out_precision)
        Ok(result.out_amount / out_precision)
    }

    /// Current spot price in the active band.
    ///
    /// Mirrors `AMM.vy::get_p()`.
    ///
    /// # Returns
    /// Price at 1e18 base, or error.
    pub fn spot_price(&self) -> Result<U256, PoolError> {
        let p_o_up = self.compute_p_oracle_up(self.active_band)?;
        let x = *self.bands_x.get(&self.active_band).unwrap_or(&U256::ZERO);
        let y = *self.bands_y.get(&self.active_band).unwrap_or(&U256::ZERO);

        get_p(x, y, self.p_oracle, p_o_up, self.a, self.a_minus_1).ok_or(PoolError::MathError)
    }

    /// Compute `_p_oracle_up(n)` for a given band.
    ///
    /// Internal helper — computes `base_price * exp(-n * log_a_ratio) / WAD`.
    fn compute_p_oracle_up(&self, n: i64) -> Result<U256, PoolError> {
        p_oracle_up(n, self.base_price, self.log_a_ratio).ok_or(PoolError::MathError)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::WAD;

    /// Helper: create a minimal pool for testing.
    fn test_pool(
        a: u64,
        base_price: U256,
        bands_x: HashMap<i64, U256>,
        bands_y: HashMap<i64, U256>,
        p_oracle: U256,
    ) -> LlammaPool {
        let a_val = U256::from(a);
        let a_minus_1 = U256::from(a - 1);

        // log_a_ratio = ln(A / (A-1)) * 1e18
        // For A=100: ln(100/99) ≈ 0.01005034... * 1e18 ≈ 10050335853501...
        // We compute an approximation for testing; real values come from on-chain
        let log_a_ratio = I256::try_from(10_050_335_853_501i128).unwrap();

        // max_oracle_dn_pow = (A / (A-1))^50
        let mut pow = WAD;
        for _ in 0..50 {
            pow = pow * a_val / a_minus_1;
        }

        // sqrt_band_ratio = sqrt(A / (A-1)) * 1e18
        // For A=100: sqrt(100/99) ≈ 1.00503... * 1e18
        let sqrt_band_ratio = WAD * U256::from(10050u64) / U256::from(10000u64); // rough

        LlammaPool::new(
            a_val,
            a_minus_1,
            base_price,
            log_a_ratio,
            pow,
            sqrt_band_ratio,
            U256::from(1u64),          // 18 decimals
            U256::from(1u64),          // 18 decimals
            WAD / U256::from(1000u64), // 0.1%
            0,
            -10,
            10,
            bands_x,
            bands_y,
            p_oracle,
            U256::ZERO,
            false,
        )
        .expect("test pool params are valid")
    }

    #[test]
    fn get_amount_out_invalid_index() {
        let pool = test_pool(
            100,
            WAD * U256::from(2000u64),
            HashMap::new(),
            HashMap::new(),
            WAD * U256::from(2000u64),
        );
        assert_eq!(
            pool.get_amount_out(0, 0, WAD).unwrap_err(),
            PoolError::InvalidIndex
        );
        assert_eq!(
            pool.get_amount_out(2, 0, WAD).unwrap_err(),
            PoolError::InvalidIndex
        );
    }

    #[test]
    fn get_amount_out_zero_returns_zero() {
        let pool = test_pool(
            100,
            WAD * U256::from(2000u64),
            HashMap::new(),
            HashMap::new(),
            WAD * U256::from(2000u64),
        );
        assert_eq!(pool.get_amount_out(0, 1, U256::ZERO).unwrap(), U256::ZERO);
        assert_eq!(pool.get_amount_out(1, 0, U256::ZERO).unwrap(), U256::ZERO);
    }

    #[test]
    fn get_amount_out_no_liquidity_returns_zero() {
        let pool = test_pool(
            100,
            WAD * U256::from(2000u64),
            HashMap::new(),
            HashMap::new(),
            WAD * U256::from(2000u64),
        );
        let result = pool.get_amount_out(0, 1, WAD * U256::from(100u64)).unwrap();
        assert_eq!(result, U256::ZERO);
    }

    #[test]
    fn get_amount_out_pump_produces_output() {
        let mut bx = HashMap::new();
        let mut by = HashMap::new();
        bx.insert(0i64, WAD * U256::from(100u64));
        by.insert(0i64, WAD * U256::from(10u64));

        let base_price = WAD * U256::from(2000u64);
        let p_oracle = WAD * U256::from(2000u64);
        let pool = test_pool(100, base_price, bx, by, p_oracle);

        // Swap borrowed (i=0) for collateral (j=1)
        let dx = WAD * U256::from(100u64);
        let dy = pool.get_amount_out(0, 1, dx).unwrap();

        assert!(dy > U256::ZERO, "should get collateral output, got {dy}");
    }

    #[test]
    fn get_amount_out_dump_produces_output() {
        let mut bx = HashMap::new();
        let mut by = HashMap::new();
        bx.insert(0i64, WAD * U256::from(10000u64));
        by.insert(0i64, WAD * U256::from(5u64));

        let base_price = WAD * U256::from(2000u64);
        let p_oracle = WAD * U256::from(2000u64);
        let pool = test_pool(100, base_price, bx, by, p_oracle);

        // Swap collateral (i=1) for borrowed (j=0)
        let dy = WAD; // 1 ETH
        let dx = pool.get_amount_out(1, 0, dy).unwrap();

        assert!(dx > U256::ZERO, "should get borrowed output, got {dx}");
    }

    #[test]
    fn spot_price_returns_nonzero() {
        let mut bx = HashMap::new();
        let mut by = HashMap::new();
        bx.insert(0i64, WAD * U256::from(1000u64));
        by.insert(0i64, WAD * U256::from(5u64));

        let base_price = WAD * U256::from(2000u64);
        let p_oracle = WAD * U256::from(2000u64);
        let pool = test_pool(100, base_price, bx, by, p_oracle);

        let price = pool.spot_price().unwrap();
        assert!(price > U256::ZERO, "spot price should be > 0, got {price}");
    }

    #[test]
    fn get_amount_out_larger_input_gives_more_output() {
        let mut bx = HashMap::new();
        let mut by = HashMap::new();
        bx.insert(0i64, WAD * U256::from(1000u64));
        by.insert(0i64, WAD * U256::from(50u64));

        let base_price = WAD * U256::from(2000u64);
        let p_oracle = WAD * U256::from(2000u64);
        let pool = test_pool(100, base_price, bx, by, p_oracle);

        let small_dx = WAD * U256::from(10u64);
        let large_dx = WAD * U256::from(100u64);

        let small_dy = pool.get_amount_out(0, 1, small_dx).unwrap();
        let large_dy = pool.get_amount_out(0, 1, large_dx).unwrap();

        assert!(
            large_dy > small_dy,
            "larger input should give more output: {small_dy} vs {large_dy}"
        );
    }
}
