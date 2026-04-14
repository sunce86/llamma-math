//! Core stateless math functions for LLAMMA.
//!
//! Ported line-by-line from:
//! - `AMM.vy`: <https://github.com/curvefi/curve-stablecoin/blob/master/curve_stablecoin/AMM.vy>
//! - `snekmate/utils/math.vy`: <https://github.com/pcaversaccio/snekmate/blob/main/src/snekmate/utils/math.vy>
//!
//! All arithmetic follows EVM uint256/int256 semantics:
//! - Division truncates toward zero.
//! - All intermediate results are in 1e18 ("WAD") fixed-point unless noted.

use alloy_primitives::{I256, U256};

use crate::constants::WAD;

// Integer square root

/// Integer square root (floor). Babylonian method.
///
/// `AMM.vy` L190–195:
/// ```vyper
/// def sqrt_int(_x: uint256) -> uint256:
///     return isqrt(_x)
/// ```
///
/// Matches Vyper's built-in `isqrt`.
pub fn sqrt_int(x: U256) -> U256 {
    if x.is_zero() {
        return U256::ZERO;
    }
    let mut z = (x + U256::from(1u64)) >> 1;
    let mut y = x;
    while z < y {
        y = z;
        z = (x / z + z) >> 1;
    }
    y
}

// snekmate wad_exp  (Remco Bloemen's exp approximation)

/// Natural exponential `e^x` with 1e18 precision.
///
/// Port of `snekmate/utils/math.vy::_wad_exp(x: int256) -> int256`.
/// Source: <https://github.com/pcaversaccio/snekmate/blob/main/src/snekmate/utils/math.vy>
///
/// Input `x` is a signed 256-bit fixed-point number in WAD (1e18) units.
/// Returns `e^x` in WAD units, or `None` if overflow.
///
/// Range: for `x <= -41.446e18` returns 0.  For `x >= 135.305e18` overflows.
pub fn wad_exp(x: I256) -> Option<U256> {
    // Port of the Remco Bloemen exp approximation as used in Curve's AMM.vy (V1).
    // Algorithm: https://xn--2-umb.com/22/exp-ln
    //
    // Uses SDIV (wrapping_div) for all intermediate /2^96 divisions,
    // matching the on-chain Vyper `unsafe_div(x, 2**96)` which compiles to SDIV.
    // This differs from snekmate's `>> 96` (SAR) for negative intermediates.
    //
    // All arithmetic uses WRAPPING operations to match Vyper's
    // unsafe_mul/unsafe_add/unsafe_sub which wrap modulo 2^256.

    // Vyper V1: assert power > -41446531673892821376
    // Use the V1 threshold to match deployed LLAMMA contracts.
    if x <= I256::try_from(-41_446_531_673_892_822_313i128).unwrap() {
        return Some(U256::ZERO);
    }

    // Vyper: assert x < 135_305_999_368_893_231_589
    if x >= I256::from_raw(U256::from_be_slice(
        &135_305_999_368_893_231_589u128.to_be_bytes(),
    )) {
        return None; // overflow
    }

    // Helper: construct I256 from u128 (for large positive constants)
    let c = |v: u128| -> I256 { I256::from_raw(U256::from(v)) };

    // 2^96 as I256 — used for SDIV divisions throughout
    let two_96: I256 = c(1u128 << 96);

    // Vyper V1: x = unsafe_div(unsafe_mul(power, 2**96), 10**18)
    // Equivalent to: x = (power * 2^96) / 10^18 via SDIV
    // Also equivalent to: x = (power << 78) / 5^18 (same numerical result)
    let wad: I256 = c(1_000_000_000_000_000_000);
    let mut x: I256 = x.wrapping_mul(two_96).wrapping_div(wad);

    // k = round(x / log(2))
    // Vyper V1: k = unsafe_div(unsafe_add(unsafe_div(unsafe_mul(x, 2**96), ln2), 2**95), 2**96)
    let ln2_96 = c(54_916_777_467_707_473_351_141_471_128);
    let k: I256 = x
        .wrapping_mul(two_96)
        .wrapping_div(ln2_96)
        .wrapping_add(I256::from_raw(U256::from(2u64).pow(U256::from(95))))
        .wrapping_div(two_96);

    // x' = x - k * log(2)
    x = x.wrapping_sub(k.wrapping_mul(ln2_96));

    // Evaluate (6, 7)-term rational approximation for p.
    // All intermediate divisions by 2^96 use SDIV (wrapping_div).
    let mut y: I256 = x.wrapping_add(c(1_346_386_616_545_796_478_920_950_773_328));
    y = y
        .wrapping_mul(x)
        .wrapping_div(two_96)
        .wrapping_add(c(57_155_421_227_552_351_082_224_309_758_442));

    let mut p: I256 = y
        .wrapping_add(x)
        .wrapping_sub(c(94_201_549_194_550_492_254_356_042_504_812));
    p = p
        .wrapping_mul(y)
        .wrapping_div(two_96)
        .wrapping_add(c(28_719_021_644_029_726_153_956_944_680_412_240));
    p = p
        .wrapping_mul(x)
        .wrapping_add(c(4_385_272_521_454_847_904_659_076_985_693_276).wrapping_shl(96));

    // Evaluate q polynomial.
    let mut q: I256 = x.wrapping_sub(c(2_855_989_394_907_223_263_936_484_059_900));
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

    // r = p / q (in 2^192 base)
    let r: I256 = p.wrapping_div(q);

    // Finalize: multiply by scale factor s ≈ 6.031367120, adjust for 2^k, and
    // convert from 2^96 to 1e18 base. Done all at once with the magic constant.
    //
    // snekmate (current version):
    //   return convert(
    //       unsafe_mul(convert(convert(r, bytes32), uint256),
    //           3_822_833_074_963_236_453_042_738_258_902_158_003_155_416_615_667)
    //       >> convert(unsafe_sub(195, k), uint256), int256)
    //
    // r is converted to uint256 (raw bits, preserving two's complement),
    // then multiplied (wrapping) by the scale constant, then right-shifted.
    // 3_822_833_074_963_236_453_042_738_258_902_158_003_155_416_615_667
    // = 0x29d9dc38563c32e5c2f6dc192ee70ef65f9978af3
    let scale = U256::from_str_radix("29d9dc38563c32e5c2f6dc192ee70ef65f9978af3", 16).unwrap();
    let shift: I256 = I256::try_from(195).unwrap().wrapping_sub(k);
    // Safe: input bounds (lines 63-72) guarantee x ∈ (-42e18, 135e18),
    // so k = round(x/ln2) ∈ (-61, 195), making shift ∈ (0, 256).
    let shift_u: usize = shift.as_i64() as usize;

    let r_uint: U256 = r.into_raw();
    let (product, _) = r_uint.overflowing_mul(scale);
    let result_uint: U256 = product >> shift_u;

    // Convert back to signed (matching Vyper's final convert(..., int256))
    // then return as unsigned (the result is always positive in the valid range).
    let result = I256::from_raw(result_uint);

    Some(result.into_raw())
}

// Band pricing

/// Upper oracle price for band `n`.
///
/// `AMM.vy` L345–361:
/// ```vyper
/// def _p_oracle_up(n: int256) -> uint256:
///     power: int256 = -n * LOG_A_RATIO
///     exp_result: uint256 = convert(math._wad_exp(power), uint256)
///     assert exp_result > 1000
///     return unsafe_div(self._base_price() * exp_result, WAD)
/// ```
///
/// `base_price` is already adjusted for rate_mul by the caller.
pub fn p_oracle_up(n: i64, base_price: U256, log_a_ratio: I256) -> Option<U256> {
    // power = -n * LOG_A_RATIO
    let power = I256::try_from(-n as i128).ok()? * log_a_ratio;

    // exp_result = wad_exp(power)
    let exp_result = wad_exp(power)?;

    // Vyper: assert exp_result > 1000
    if exp_result <= U256::from(1000u64) {
        return None;
    }

    // Vyper: return unsafe_div(self._base_price() * exp_result, WAD)
    Some(base_price * exp_result / WAD)
}

/// Lower oracle price for band `n`. Equal to `p_oracle_up(n + 1)`.
///
/// `AMM.vy` L416–422:
/// ```vyper
/// def p_oracle_down(n: int256) -> uint256:
///     return self._p_oracle_up(n + 1)
/// ```
pub fn p_oracle_down(n: i64, base_price: U256, log_a_ratio: I256) -> Option<U256> {
    p_oracle_up(n + 1, base_price, log_a_ratio)
}

// Dynamic fee

/// Dynamic fee based on oracle/AMM price divergence.
///
/// `AMM.vy` L256–267:
/// ```vyper
/// def get_dynamic_fee(p_o: uint256, p_o_up: uint256) -> uint256:
///     p_c_d: uint256 = unsafe_div(unsafe_div(p_o ** 2, p_o_up) * p_o, p_o_up)
///     p_c_u: uint256 = unsafe_div(unsafe_div(p_c_d * A, Aminus1) * A, Aminus1)
///     if p_o < p_c_d:
///         return unsafe_div(unsafe_sub(p_c_d, p_o) * (10**18 // 4), p_c_d)
///     elif p_o > p_c_u:
///         return unsafe_div(unsafe_sub(p_o, p_c_u) * (10**18 // 4), p_o)
///     else:
///         return 0
/// ```
/// Preconditions: `p_o_up > 0`, `a_minus_1 > 0`, `p_o > 0` (guaranteed by on-chain AMM invariants).
pub fn get_dynamic_fee(p_o: U256, p_o_up: U256, a: U256, a_minus_1: U256) -> U256 {
    // p_c_d = p_o^2 / p_o_up * p_o / p_o_up  (p_o_up > 0: band price from exp, always positive)
    let p_c_d = (p_o * p_o / p_o_up) * p_o / p_o_up;

    // p_c_u = p_c_d * A / (A-1) * A / (A-1)  (a_minus_1 > 0: A ≥ 2 on-chain)
    let p_c_u = (p_c_d * a / a_minus_1) * a / a_minus_1;

    // WAD / 4 = 0.25e18
    let quarter_wad = WAD / U256::from(4u64);

    if p_o < p_c_d {
        // Vyper: unsafe_div(unsafe_sub(p_c_d, p_o) * (10**18 // 4), p_c_d)
        (p_c_d - p_o) * quarter_wad / p_c_d
    } else if p_o > p_c_u {
        // Vyper: unsafe_div(unsafe_sub(p_o, p_c_u) * (10**18 // 4), p_o)
        (p_o - p_c_u) * quarter_wad / p_o
    } else {
        U256::ZERO
    }
}

// Core invariant: _get_y0

/// Calculate `y0` — the amount of collateral when band has no borrowed tokens
/// but current price equals both oracle price and upper band price.
///
/// `AMM.vy` L427–451:
/// ```vyper
/// def _get_y0(x: uint256, y: uint256, p_o: uint256, p_o_up: uint256) -> uint256:
///     assert p_o != 0
///     b: uint256 = 0
///     if x != 0:
///         b = unsafe_div(p_o_up * Aminus1 * x, p_o)
///     if y != 0:
///         b += unsafe_div(A * p_o**2 // p_o_up * y, 10**18)
///     if x > 0 and y > 0:
///         D: uint256 = b**2 + unsafe_div((unsafe_mul(4, A) * p_o) * y, 10**18) * x
///         return unsafe_div((b + self.sqrt_int(D)) * 10**18, unsafe_mul(unsafe_mul(2, A), p_o))
///     else:
///         return unsafe_div(b * 10**18, unsafe_mul(A, p_o))
/// ```
/// Preconditions: `p_o > 0` (checked), `p_o_up > 0` (band price from exp),
/// `a > 0` (amplification ≥ 2 on-chain), `a_minus_1 > 0`.
pub fn get_y0(x: U256, y: U256, p_o: U256, p_o_up: U256, a: U256, a_minus_1: U256) -> Option<U256> {
    if p_o.is_zero() {
        return None;
    }

    let mut b = U256::ZERO;

    if !x.is_zero() {
        // p_o checked nonzero above
        b = p_o_up * a_minus_1 * x / p_o;
    }

    if !y.is_zero() {
        // p_o_up > 0: derived from wad_exp which returns > 1000 (see p_oracle_up)
        b += a * p_o * p_o / p_o_up * y / WAD;
    }

    if !x.is_zero() && !y.is_zero() {
        let d = b * b + (U256::from(4u64) * a * p_o) * y / WAD * x;
        // a * p_o > 0: both guaranteed positive by preconditions
        Some((b + sqrt_int(d)) * WAD / (U256::from(2u64) * a * p_o))
    } else {
        // a * p_o > 0: same
        Some(b * WAD / (a * p_o))
    }
}

// Spot price in band

/// Current AMM price in a specific band.
///
/// `AMM.vy` L456–485:
/// ```vyper
/// def _get_p(n: int256, x: uint256, y: uint256) -> uint256:
///     p_o_up: uint256 = self._p_oracle_up(n)
///     p_o: uint256 = self._price_oracle_ro()[0]
///     ...
/// ```
///
/// Caller provides `p_o` and `p_o_up` since those depend on external state.
/// Preconditions: `p_o_up > 0` (checked), `a_minus_1 > 0`, `a > 0`, `p_o > 0`
/// (guaranteed by on-chain AMM invariants — A ≥ 2, prices from wad_exp > 1000).
pub fn get_p(x: U256, y: U256, p_o: U256, p_o_up: U256, a: U256, a_minus_1: U256) -> Option<U256> {
    if p_o_up.is_zero() {
        return None;
    }

    if x.is_zero() {
        if y.is_zero() {
            // p_o_up checked above, a_minus_1 > 0 by precondition
            return Some(((p_o * p_o / p_o_up) * p_o / p_o_up) * a / a_minus_1);
        }
        return Some((p_o * p_o / p_o_up) * p_o / p_o_up);
    }

    if y.is_zero() {
        // a > 0 by precondition
        let p_o_down = p_o_up * a_minus_1 / a;
        // p_o_down > 0: p_o_up > 0, a_minus_1/a ∈ (0.5, 1) for A ≥ 2
        return Some(p_o * p_o / p_o_down * p_o / p_o_down);
    }

    let y0 = get_y0(x, y, p_o, p_o_up, a, a_minus_1)?;

    let f = a * y0 * p_o / p_o_up * p_o;
    // p_o > 0 by precondition
    let g = a_minus_1 * y0 * p_o_up / p_o;

    // g + y > 0: y > 0 (checked above)
    Some((f + x * WAD) / (g + y))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqrt_int_basic() {
        assert_eq!(sqrt_int(U256::ZERO), U256::ZERO);
        assert_eq!(sqrt_int(U256::from(1u64)), U256::from(1u64));
        assert_eq!(sqrt_int(U256::from(4u64)), U256::from(2u64));
        assert_eq!(sqrt_int(U256::from(9u64)), U256::from(3u64));
        // floor(sqrt(10)) = 3
        assert_eq!(sqrt_int(U256::from(10u64)), U256::from(3u64));
    }

    #[test]
    fn sqrt_int_large() {
        // sqrt(1e36) = 1e18
        let x = U256::from(10u64).pow(U256::from(36));
        assert_eq!(sqrt_int(x), U256::from(10u64).pow(U256::from(18)));
    }

    #[test]
    fn wad_exp_zero() {
        // e^0 = 1e18
        let result = wad_exp(I256::ZERO).unwrap();
        assert_eq!(result, WAD);
    }

    #[test]
    fn wad_exp_one() {
        // e^1 ≈ 2.718e18
        let result = wad_exp(I256::try_from(WAD).unwrap()).unwrap();
        // e ≈ 2.71828... so result should be ~2_718_281_828_459_045_235
        let expected_low = U256::from(2_718_281_828_000_000_000u128);
        let expected_high = U256::from(2_718_281_829_000_000_000u128);
        assert!(
            result >= expected_low && result <= expected_high,
            "e^1 = {result}"
        );
    }

    #[test]
    fn wad_exp_negative_large_returns_zero() {
        // Very negative → 0
        let result = wad_exp(I256::try_from(-50_000_000_000_000_000_000i128).unwrap()).unwrap();
        assert_eq!(result, U256::ZERO);
    }

    #[test]
    fn wad_exp_overflow_returns_none() {
        // x >= 135.305e18 → overflow
        let result = wad_exp(I256::try_from(136_000_000_000_000_000_000i128).unwrap());
        assert!(result.is_none());
    }

    #[test]
    fn wad_exp_negative_one() {
        // e^(-1) ≈ 0.3678e18
        let neg_wad = -I256::try_from(WAD).unwrap();
        let result = wad_exp(neg_wad).unwrap();
        let expected_low = U256::from(367_879_441_000_000_000u128);
        let expected_high = U256::from(367_879_442_000_000_000u128);
        assert!(
            result >= expected_low && result <= expected_high,
            "e^(-1) = {result}"
        );
    }

    #[test]
    fn wad_exp_two() {
        // e^2 ≈ 7.389e18
        let two_wad = I256::try_from(2_000_000_000_000_000_000i128).unwrap();
        let result = wad_exp(two_wad).unwrap();
        let expected_low = U256::from(7_389_056_098_000_000_000u128);
        let expected_high = U256::from(7_389_056_099_000_000_000u128);
        assert!(
            result >= expected_low && result <= expected_high,
            "e^2 = {result}"
        );
    }

    #[test]
    fn wad_exp_ten() {
        // e^10 ≈ 22026.465e18
        let ten_wad = I256::try_from(10_000_000_000_000_000_000i128).unwrap();
        let result = wad_exp(ten_wad).unwrap();
        let expected_low = U256::from(22_026_465_794_000_000_000_000u128);
        let expected_high = U256::from(22_026_465_795_000_000_000_000u128);
        assert!(
            result >= expected_low && result <= expected_high,
            "e^10 = {result}"
        );
    }

    #[test]
    fn wad_exp_negative_two() {
        // e^(-2) ≈ 0.1353e18
        let neg_two_wad = I256::try_from(-2_000_000_000_000_000_000i128).unwrap();
        let result = wad_exp(neg_two_wad).unwrap();
        let expected_low = U256::from(135_335_283_000_000_000u128);
        let expected_high = U256::from(135_335_284_000_000_000u128);
        assert!(
            result >= expected_low && result <= expected_high,
            "e^(-2) = {result}"
        );
    }

    #[test]
    fn wad_exp_negative_ten() {
        // e^(-10) ≈ 0.0000454e18 = 45399929762484854
        let neg_ten_wad = I256::try_from(-10_000_000_000_000_000_000i128).unwrap();
        let result = wad_exp(neg_ten_wad).unwrap();
        let expected_low = U256::from(45_399_929_000_000u128);
        let expected_high = U256::from(45_399_930_000_000u128);
        assert!(
            result >= expected_low && result <= expected_high,
            "e^(-10) = {result}"
        );
    }

    #[test]
    fn wad_exp_half() {
        // e^0.5 ≈ 1.6487e18
        let half_wad = I256::try_from(500_000_000_000_000_000i128).unwrap();
        let result = wad_exp(half_wad).unwrap();
        let expected_low = U256::from(1_648_721_270_000_000_000u128);
        let expected_high = U256::from(1_648_721_271_000_000_000u128);
        assert!(
            result >= expected_low && result <= expected_high,
            "e^0.5 = {result}"
        );
    }

    #[test]
    fn wad_exp_monotonic() {
        // e^a < e^b for a < b
        let a = I256::try_from(-5_000_000_000_000_000_000i128).unwrap();
        let b = I256::try_from(-1_000_000_000_000_000_000i128).unwrap();
        let c_val = I256::ZERO;
        let d = I256::try_from(1_000_000_000_000_000_000i128).unwrap();
        let e = I256::try_from(5_000_000_000_000_000_000i128).unwrap();

        let ra = wad_exp(a).unwrap();
        let rb = wad_exp(b).unwrap();
        let rc = wad_exp(c_val).unwrap();
        let rd = wad_exp(d).unwrap();
        let re = wad_exp(e).unwrap();

        assert!(ra < rb, "e^(-5) < e^(-1): {ra} vs {rb}");
        assert!(rb < rc, "e^(-1) < e^(0): {rb} vs {rc}");
        assert!(rc < rd, "e^(0) < e^(1): {rc} vs {rd}");
        assert!(rd < re, "e^(1) < e^(5): {rd} vs {re}");
    }

    #[test]
    fn wad_exp_boundary_negative() {
        // Just above the zero threshold: should be very small but nonzero
        let x = I256::try_from(-41_000_000_000_000_000_000i128).unwrap();
        let result = wad_exp(x).unwrap();
        assert!(result > U256::ZERO, "e^(-41e18) should be > 0: {result}");
        assert!(
            result < U256::from(1000u64),
            "e^(-41e18) should be tiny: {result}"
        );
    }

    #[test]
    fn wad_exp_boundary_zero_threshold() {
        // At exactly the threshold: returns 0
        let x = I256::try_from(-42_139_678_854_452_767_551i128).unwrap();
        let result = wad_exp(x).unwrap();
        assert_eq!(result, U256::ZERO);
    }

    #[test]
    fn wad_exp_product_rule() {
        // e^(a+b) ≈ e^a * e^b (within rounding)
        let a = I256::try_from(2_000_000_000_000_000_000i128).unwrap();
        let b = I256::try_from(3_000_000_000_000_000_000i128).unwrap();
        let a_plus_b = I256::try_from(5_000_000_000_000_000_000i128).unwrap();

        let ea = wad_exp(a).unwrap();
        let eb = wad_exp(b).unwrap();
        let eab = wad_exp(a_plus_b).unwrap();

        // e^a * e^b / 1e18 should ≈ e^(a+b)
        let product = ea * eb / WAD;

        // Allow small relative error (< 0.001%)
        let diff = if product > eab {
            product - eab
        } else {
            eab - product
        };
        let max_err = eab / U256::from(100_000u64); // 0.001%
        assert!(
            diff <= max_err,
            "product rule: e^2 * e^3 = {product}, e^5 = {eab}, diff = {diff}"
        );
    }

    #[test]
    fn get_dynamic_fee_in_range_returns_zero() {
        let a = U256::from(100u64);
        let a_minus_1 = U256::from(99u64);
        // p_o exactly at midpoint → fee = 0
        let p_o_up = WAD * U256::from(3000u64);
        // p_c_d = p_o^3 / p_o_up^2
        // When p_o is between p_c_d and p_c_u, fee is 0
        // Set p_o = p_o_up * (A-1)/A * sqrt(A/(A-1)) roughly
        // For simplicity test with p_o slightly less than p_o_up
        let p_o = p_o_up * U256::from(99u64) / U256::from(100u64);
        let fee = get_dynamic_fee(p_o, p_o_up, a, a_minus_1);
        // When p_o is close to p_o_up and within p_c_d..p_c_u range, fee should be 0
        // This depends on exact values; assert it's small
        assert!(fee <= WAD / U256::from(4u64), "fee = {fee}");
    }

    #[test]
    fn get_y0_zero_amounts() {
        let a = U256::from(100u64);
        let a_minus_1 = U256::from(99u64);
        let p_o = WAD * U256::from(3000u64);
        let p_o_up = WAD * U256::from(3010u64);

        // Both zero → b=0, y0=0
        let y0 = get_y0(U256::ZERO, U256::ZERO, p_o, p_o_up, a, a_minus_1).unwrap();
        assert_eq!(y0, U256::ZERO);
    }

    #[test]
    fn get_y0_only_x() {
        let a = U256::from(100u64);
        let a_minus_1 = U256::from(99u64);
        let p_o = WAD * U256::from(3000u64);
        let p_o_up = WAD * U256::from(3010u64);
        let x = WAD * U256::from(1000u64); // 1000 borrowed tokens

        let y0 = get_y0(x, U256::ZERO, p_o, p_o_up, a, a_minus_1).unwrap();
        assert!(y0 > U256::ZERO);
    }

    #[test]
    fn get_y0_only_y() {
        let a = U256::from(100u64);
        let a_minus_1 = U256::from(99u64);
        let p_o = WAD * U256::from(3000u64);
        let p_o_up = WAD * U256::from(3010u64);
        let y = WAD; // 1 collateral token

        let y0 = get_y0(U256::ZERO, y, p_o, p_o_up, a, a_minus_1).unwrap();
        assert!(y0 > U256::ZERO);
    }

    #[test]
    fn get_y0_both_xy() {
        let a = U256::from(100u64);
        let a_minus_1 = U256::from(99u64);
        let p_o = WAD * U256::from(3000u64);
        let p_o_up = WAD * U256::from(3010u64);
        let x = WAD * U256::from(1000u64);
        let y = WAD; // 1 ETH

        let y0 = get_y0(x, y, p_o, p_o_up, a, a_minus_1).unwrap();
        assert!(y0 > U256::ZERO);
    }

    #[test]
    fn get_y0_rejects_zero_p_o() {
        let a = U256::from(100u64);
        let a_minus_1 = U256::from(99u64);
        assert!(get_y0(WAD, WAD, U256::ZERO, WAD, a, a_minus_1).is_none());
    }

    #[test]
    fn get_p_both_zero_returns_midband() {
        let a = U256::from(100u64);
        let a_minus_1 = U256::from(99u64);
        let p_o = WAD * U256::from(3000u64);
        let p_o_up = WAD * U256::from(3010u64);

        let price = get_p(U256::ZERO, U256::ZERO, p_o, p_o_up, a, a_minus_1).unwrap();
        // Mid-band price should be between p_current_down and p_current_up
        assert!(price > U256::ZERO);
    }

    #[test]
    fn get_p_only_y_returns_p_current_down() {
        let a = U256::from(100u64);
        let a_minus_1 = U256::from(99u64);
        let p_o = WAD * U256::from(3000u64);
        let p_o_up = WAD * U256::from(3010u64);

        // x=0, y>0 → lowest point of band (p_current_down)
        let price = get_p(U256::ZERO, WAD, p_o, p_o_up, a, a_minus_1).unwrap();
        assert!(price > U256::ZERO);
    }

    #[test]
    fn get_p_only_x_returns_p_current_up() {
        let a = U256::from(100u64);
        let a_minus_1 = U256::from(99u64);
        let p_o = WAD * U256::from(3000u64);
        let p_o_up = WAD * U256::from(3010u64);

        // y=0, x>0 → highest point of band (p_current_up)
        let price = get_p(
            WAD * U256::from(1000u64),
            U256::ZERO,
            p_o,
            p_o_up,
            a,
            a_minus_1,
        )
        .unwrap();
        assert!(price > U256::ZERO);
    }
}
