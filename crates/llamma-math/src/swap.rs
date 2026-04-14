//! Swap math for LLAMMA — band traversal and amount computation.
//!
//! Ported line-by-line from `AMM.vy` L787–927 (`calc_swap_out`)
//! and L1079–1220 (`calc_swap_in`).
//!
//! Source: <https://github.com/curvefi/curve-stablecoin/blob/master/curve_stablecoin/AMM.vy>

use alloy_primitives::U256;
use std::collections::HashMap;

use crate::constants::{MAX_SKIP_TICKS_UINT, MAX_TICKS_UINT, WAD};
use crate::core::{get_dynamic_fee, get_y0};
use crate::pool::PoolError;

/// Result of a swap calculation, mirroring `IAMM.DetailedTrade` from Vyper.
///
/// ```vyper
/// struct DetailedTrade:
///     in_amount: uint256
///     out_amount: uint256
///     n1: int256
///     n2: int256
///     ticks_in: DynArray[uint256, 50]
///     last_tick_j: uint256
/// ```
#[derive(Debug, Clone, Default)]
pub struct DetailedTrade {
    /// Amount of input token actually consumed (may be less than requested).
    pub in_amount: U256,
    /// Amount of output token received.
    pub out_amount: U256,
    /// First band with liquidity (initial band of the swap).
    pub n1: i64,
    /// Last band reached by the swap.
    pub n2: i64,
    /// New token amounts in each band touched by the swap.
    pub ticks_in: Vec<U256>,
    /// Remaining amount of output token in the last band.
    pub last_tick_j: U256,
}

/// Parameters describing the current state of a LLAMMA pool,
/// needed to compute a swap.
pub struct SwapState<'a> {
    /// Amplification parameter.
    pub a: U256,
    /// A - 1
    pub a_minus_1: U256,
    /// Static fee (from pool storage).
    pub fee: U256,
    /// Borrowed token amounts per band.
    pub bands_x: &'a HashMap<i64, U256>,
    /// Collateral token amounts per band.
    pub bands_y: &'a HashMap<i64, U256>,
    /// Current active band.
    pub active_band: i64,
    /// Lowest non-empty band.
    pub min_band: i64,
    /// Highest non-empty band.
    pub max_band: i64,
    /// Oracle price (p_o) — already limited by caller.
    pub p_oracle: U256,
    /// Dynamic fee component from oracle price limiting.
    pub oracle_fee: U256,
    /// Upper oracle price for band `active_band`.
    /// Caller computes this via `p_oracle_up(active_band, ...)`.
    pub p_oracle_up_active: U256,
    /// Precomputed: `(A / (A-1))^50` — max oracle dn power.
    pub max_oracle_dn_pow: U256,
    /// Precision of borrowed token (10^(18-decimals)).
    pub in_precision: U256,
    /// Precision of collateral token (10^(18-decimals)).
    pub out_precision: U256,
    /// Use static antifee (crvUSD mint, Vyper 0.3.7) vs per-band (Llamalend, Vyper 0.4.x).
    pub static_antifee: bool,
}

/// Calculate swap output amount via band traversal.
///
/// This is the core LLAMMA swap function.
///
/// `AMM.vy` L787–927: `calc_swap_out`
///
/// # Arguments
///
/// * `pump` — `true` = buying collateral (borrowable in, collateral out, going up);
///   `false` = selling collateral (collateral in, borrowable out, going down).
/// * `in_amount` — amount of input token (in internal precision, i.e. already scaled).
/// * `state` — current pool state.
///
/// # Returns
///
/// A `DetailedTrade` describing the swap result, or `PoolError::MathError`.
pub fn calc_swap_out(
    pump: bool,
    in_amount: U256,
    state: &SwapState,
) -> Result<DetailedTrade, PoolError> {
    // AMM.vy L802-808
    let min_band = state.min_band;
    let max_band = state.max_band;
    let mut out = DetailedTrade {
        n2: state.active_band,
        ..Default::default()
    };
    let mut p_o_up = state.p_oracle_up_active;
    let mut x = *state.bands_x.get(&out.n2).unwrap_or(&U256::ZERO);
    let mut y = *state.bands_y.get(&out.n2).unwrap_or(&U256::ZERO);

    // AMM.vy L810-812
    let mut in_amount_left = in_amount;
    // fee = max(self.fee, p_o[1])
    let fee = if state.fee > state.oracle_fee {
        state.fee
    } else {
        state.oracle_fee
    };
    let mut j: u64 = MAX_TICKS_UINT;

    let p_o = state.p_oracle;
    let a = state.a;
    let a_minus_1 = state.a_minus_1;

    // Vyper 0.3.7 (crvUSD mint): antifee computed once before loop.
    // Vyper 0.4.x (Llamalend): antifee recomputed per band with dynamic fee.
    let static_antifee_val = if state.static_antifee {
        let capped = if fee < WAD - U256::from(1u64) {
            fee
        } else {
            WAD - U256::from(1u64)
        };
        Some(WAD * WAD / (WAD - capped))
    } else {
        None
    };

    // AMM.vy L814: for i in range(MAX_TICKS_UINT + MAX_SKIP_TICKS_UINT)
    let max_iter = MAX_TICKS_UINT + MAX_SKIP_TICKS_UINT;
    for i in 0..max_iter {
        // AMM.vy L815-819
        let y0;
        let mut f = U256::ZERO;
        let mut g = U256::ZERO;
        let mut inv = U256::ZERO;
        let mut dynamic_fee = fee;

        // AMM.vy L821-829
        if x > U256::ZERO || y > U256::ZERO {
            if j == MAX_TICKS_UINT {
                out.n1 = out.n2;
                j = 0;
            }
            y0 = get_y0(x, y, p_o, p_o_up, a, a_minus_1).ok_or(PoolError::MathError)?;
            f = a * y0 * p_o / p_o_up * p_o / WAD;
            g = a_minus_1 * y0 * p_o_up / p_o;
            inv = (f + x) * (g + y);

            if !state.static_antifee {
                let df = get_dynamic_fee(p_o, p_o_up, a, a_minus_1);
                dynamic_fee = if df > fee { df } else { fee };
            }
        }

        let antifee = if let Some(val) = static_antifee_val {
            val
        } else {
            let capped_fee = if dynamic_fee < WAD - U256::from(1u64) {
                dynamic_fee
            } else {
                WAD - U256::from(1u64)
            };
            WAD * WAD / (WAD - capped_fee)
        };

        // AMM.vy L836-841: if j != MAX_TICKS_UINT: append tick
        if j != MAX_TICKS_UINT {
            let tick = if pump { x } else { y };
            out.ticks_in.push(tick);
        }

        // AMM.vy L844: p_ratio = p_o_up * 10**18 / p_o[0]
        let p_ratio = p_o_up * WAD / p_o;

        if pump {
            // AMM.vy L847-868: pump direction (borrowable in, collateral out)
            if y != U256::ZERO && g != U256::ZERO {
                // AMM.vy L849: x_dest = (Inv / g - f) - x
                let x_dest = (inv / g - f) - x;

                // AMM.vy L850: dx = x_dest * antifee / 10**18
                let dx = x_dest * antifee / WAD;

                if dx >= in_amount_left {
                    // AMM.vy L853: last band — partial fill
                    // x_dest = in_amount_left * 10**18 / antifee
                    let x_dest = in_amount_left * WAD / antifee;

                    // AMM.vy L854: out.last_tick_j = min(Inv // (f + (x + x_dest)) - g + 1, y)
                    let rem = inv / (f + (x + x_dest)) - g + U256::from(1u64);
                    out.last_tick_j = if rem < y { rem } else { y };

                    // AMM.vy L855-859
                    x += in_amount_left;
                    out.out_amount += y - out.last_tick_j;
                    out.ticks_in[j as usize] = x;
                    out.in_amount = in_amount;
                    break;
                } else {
                    // AMM.vy L864-868: go into next band
                    let dx = if dx > U256::from(1u64) {
                        dx
                    } else {
                        U256::from(1u64)
                    };
                    in_amount_left -= dx;
                    out.ticks_in[j as usize] = x + dx;
                    out.in_amount += dx;
                    out.out_amount += y;
                }
            }

            // AMM.vy L870-881: advance to next band (pump direction)
            if i != max_iter - 1 {
                if out.n2 == max_band {
                    break;
                }
                if j == MAX_TICKS_UINT - 1 {
                    break;
                }
                // AMM.vy L875: if p_ratio < 10**36 / MAX_ORACLE_DN_POW
                if p_ratio < WAD * WAD / state.max_oracle_dn_pow {
                    break;
                }
                out.n2 += 1;
                // AMM.vy L879: p_o_up = p_o_up * Aminus1 / A
                p_o_up = p_o_up * a_minus_1 / a;
                x = U256::ZERO;
                y = *state.bands_y.get(&out.n2).unwrap_or(&U256::ZERO);
            }
        } else {
            // AMM.vy L883-917: dump direction (collateral in, borrowable out)
            if x != U256::ZERO && f != U256::ZERO {
                // AMM.vy L886: y_dest = (Inv / f - g) - y
                let y_dest = (inv / f - g) - y;

                // AMM.vy L887: dy = y_dest * antifee / 10**18
                let dy = y_dest * antifee / WAD;

                if dy >= in_amount_left {
                    // AMM.vy L890: last band — partial fill
                    let y_dest = in_amount_left * WAD / antifee;

                    // AMM.vy L891: out.last_tick_j = min(Inv // (g + (y + y_dest)) - f + 1, x)
                    let rem = inv / (g + (y + y_dest)) - f + U256::from(1u64);
                    out.last_tick_j = if rem < x { rem } else { x };

                    // AMM.vy L892-896
                    y += in_amount_left;
                    out.out_amount += x - out.last_tick_j;
                    out.ticks_in[j as usize] = y;
                    out.in_amount = in_amount;
                    break;
                } else {
                    // AMM.vy L900-904: go into next band
                    let dy = if dy > U256::from(1u64) {
                        dy
                    } else {
                        U256::from(1u64)
                    };
                    in_amount_left -= dy;
                    out.ticks_in[j as usize] = y + dy;
                    out.in_amount += dy;
                    out.out_amount += x;
                }
            }

            // AMM.vy L906-917: advance to next band (dump direction)
            if i != max_iter - 1 {
                if out.n2 == min_band {
                    break;
                }
                if j == MAX_TICKS_UINT - 1 {
                    break;
                }
                // AMM.vy L911: if p_ratio > MAX_ORACLE_DN_POW
                if p_ratio > state.max_oracle_dn_pow {
                    break;
                }
                out.n2 -= 1;
                // AMM.vy L915: p_o_up = p_o_up * A / Aminus1
                p_o_up = p_o_up * a / a_minus_1;
                x = *state.bands_x.get(&out.n2).unwrap_or(&U256::ZERO);
                y = U256::ZERO;
            }
        }

        // AMM.vy L919-920: if j != MAX_TICKS_UINT: j += 1
        if j != MAX_TICKS_UINT {
            j += 1;
        }
    }

    // AMM.vy L922-925: round up input, round down output
    // out.in_amount = ceil(out.in_amount / in_precision) * in_precision
    let in_prec = state.in_precision;
    let out_prec = state.out_precision;
    out.in_amount = (out.in_amount + in_prec - U256::from(1u64)) / in_prec * in_prec;
    // out.out_amount = floor(out.out_amount / out_precision) * out_precision
    out.out_amount = out.out_amount / out_prec * out_prec;

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state<'a>(
        bands_x: &'a HashMap<i64, U256>,
        bands_y: &'a HashMap<i64, U256>,
        p_oracle: U256,
        p_oracle_up_active: U256,
    ) -> SwapState<'a> {
        let a = U256::from(100u64);
        let a_minus_1 = U256::from(99u64);
        let mut pow = WAD;
        for _ in 0..50 {
            pow = pow * a / a_minus_1;
        }
        SwapState {
            a,
            a_minus_1,
            fee: U256::from(10u64) * WAD / U256::from(10000u64),
            bands_x,
            bands_y,
            active_band: 0,
            min_band: -10,
            max_band: 10,
            p_oracle,
            oracle_fee: U256::ZERO,
            p_oracle_up_active,
            max_oracle_dn_pow: pow,
            in_precision: U256::from(1u64),
            out_precision: U256::from(1u64),
            static_antifee: false,
        }
    }

    #[test]
    fn calc_swap_out_zero_input_returns_zero_output() {
        let mut bx = HashMap::new();
        let mut by = HashMap::new();
        bx.insert(0i64, WAD * U256::from(1000u64));
        by.insert(0i64, WAD);

        let p_o = WAD * U256::from(3000u64);
        let p_o_up = WAD * U256::from(3010u64);

        let state = make_state(&bx, &by, p_o, p_o_up);
        let result = calc_swap_out(true, U256::ZERO, &state).unwrap();
        assert_eq!(result.out_amount, U256::ZERO);
        assert_eq!(result.in_amount, U256::ZERO);
    }

    #[test]
    fn calc_swap_out_pump_produces_output() {
        let mut bx = HashMap::new();
        let mut by = HashMap::new();
        bx.insert(0i64, WAD * U256::from(100u64));
        by.insert(0i64, WAD * U256::from(10u64));

        let p_o = WAD * U256::from(2000u64);
        let p_o_up = WAD * U256::from(2010u64);

        let state = make_state(&bx, &by, p_o, p_o_up);
        let dx = WAD * U256::from(100u64);
        let result = calc_swap_out(true, dx, &state).unwrap();

        assert!(result.out_amount > U256::ZERO, "should get some output");
        assert!(result.in_amount > U256::ZERO, "should consume some input");
        assert!(result.in_amount <= dx, "should not consume more than input");
    }

    #[test]
    fn calc_swap_out_dump_produces_output() {
        let mut bx = HashMap::new();
        let mut by = HashMap::new();
        bx.insert(0i64, WAD * U256::from(10000u64));
        by.insert(0i64, WAD * U256::from(5u64));

        let p_o = WAD * U256::from(2000u64);
        let p_o_up = WAD * U256::from(2010u64);

        let state = make_state(&bx, &by, p_o, p_o_up);
        let dy = WAD;
        let result = calc_swap_out(false, dy, &state).unwrap();

        assert!(result.out_amount > U256::ZERO, "should get some output");
        assert!(result.in_amount > U256::ZERO, "should consume some input");
    }

    #[test]
    fn calc_swap_out_no_liquidity_returns_zero() {
        let bx = HashMap::new();
        let by = HashMap::new();

        let p_o = WAD * U256::from(2000u64);
        let p_o_up = WAD * U256::from(2010u64);

        let state = make_state(&bx, &by, p_o, p_o_up);
        let result = calc_swap_out(true, WAD * U256::from(100u64), &state).unwrap();
        assert_eq!(result.out_amount, U256::ZERO);
    }
}
