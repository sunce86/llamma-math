//! Extract LLAMMA immutables from deployed contract bytecode.
//!
//! Vyper appends immutable values as 32-byte words at the end of the
//! deployed bytecode. The layout varies by compiler version but the
//! relative offsets from `A` are consistent across crvUSD mint (Vyper
//! 0.3.10) and Llamalend (Vyper 0.4.x) AMMs:
//!
//! | Offset from A | Field              |
//! |:-------------:|:-------------------|
//! |     -2        | COLLATERAL_PRECISION |
//! |      0        | A                  |
//! |     +1        | A - 1              |
//! |     +2        | A²                 |
//! |     +3        | (A - 1)²           |
//! |     +5        | LOG_A_RATIO        |
//! |     +6        | MAX_ORACLE_DN_POW  |

use alloy_primitives::{I256, U256};

/// Immutable parameters extracted from a LLAMMA AMM contract's bytecode.
///
/// These are set at deployment time and never change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlammaImmutables {
    /// Amplification parameter.
    pub a: U256,
    /// `ln(A / (A - 1)) * 1e18`, computed by Vyper's integer log at deploy time.
    pub log_a_ratio: I256,
    /// `(A / (A - 1))^50 * 1e18`.
    pub max_oracle_dn_pow: U256,
    /// `10^(18 - collateral_decimals)`.
    pub collateral_precision: U256,
    /// Whether the contract uses static antifee (Vyper 0.3.x) or per-band (0.4.x).
    /// Detected from bytecode prefix: `0x60` = Vyper 0.3.x (static), `0x5f` = 0.4.x (dynamic).
    pub static_antifee: bool,
}

/// Extract immutables from a LLAMMA AMM contract's deployed bytecode.
///
/// # Algorithm
///
/// 1. Parse the last 20 32-byte words from the bytecode.
/// 2. Search backwards for a word matching the known `A` value.
/// 3. Validate by checking the next word equals `A - 1`.
/// 4. Read LOG_A_RATIO, MAX_ORACLE_DN_POW, and COLLATERAL_PRECISION
///    at known offsets from A.
///
/// # Arguments
///
/// * `bytecode` — full deployed bytecode (hex-decoded bytes)
/// * `a` — the amplification parameter A, read from `A()` view function
///
/// # Returns
///
/// `Some(LlammaImmutables)` if extraction succeeds, `None` on parse error.
pub fn extract_immutables(bytecode: &[u8], a: U256) -> Option<LlammaImmutables> {
    // Detect Vyper version from bytecode prefix:
    // 0x60 (PUSH1) = Vyper 0.3.x → static antifee
    // 0x5f (PUSH0) = Vyper 0.4.x → per-band dynamic antifee
    let static_antifee = bytecode.first().copied() != Some(0x5f);

    // Read last 20 words (640 bytes) — enough for all known layouts.
    let word_count = 20usize;
    let tail_len = word_count * 32;
    if bytecode.len() < tail_len {
        return None;
    }

    let tail = &bytecode[bytecode.len() - tail_len..];
    let words: Vec<U256> = (0..word_count)
        .map(|i| U256::from_be_slice(&tail[i * 32..(i + 1) * 32]))
        .collect();

    // Search backwards for a word == A, validated by next word == A-1
    let a_minus_1 = a - U256::from(1u64);
    let a_index = words
        .windows(2)
        .enumerate()
        .rev()
        .find(|(_, w)| w[0] == a && w[1] == a_minus_1)
        .map(|(i, _)| i)?;

    // Validate A² and (A-1)² at offsets +2, +3
    if a_index + 6 >= word_count {
        return None;
    }
    let a_sq = words[a_index + 2];
    let a_m1_sq = words[a_index + 3];
    if a_sq != a * a || a_m1_sq != a_minus_1 * a_minus_1 {
        return None;
    }

    // Extract values at known offsets
    let log_a_ratio_raw = words[a_index + 5];
    let max_oracle_dn_pow = words[a_index + 6];

    // COLLATERAL_PRECISION is at offset -2 from A
    let collateral_precision = if a_index >= 2 {
        words[a_index - 2]
    } else {
        return None;
    };

    // Validate: collateral_precision must be a power of 10
    if !is_power_of_10(collateral_precision) {
        return None;
    }

    // LOG_A_RATIO must be positive and in reasonable range
    // For A=4 (max ratio): ln(4/3)*1e18 ≈ 2.88e17
    // For A=200 (min ratio): ln(200/199)*1e18 ≈ 5.01e15
    let log_a_ratio = I256::from_raw(log_a_ratio_raw);
    if log_a_ratio <= I256::ZERO || log_a_ratio_raw > U256::from(300_000_000_000_000_000u128) {
        return None;
    }

    Some(LlammaImmutables {
        a,
        log_a_ratio,
        max_oracle_dn_pow,
        collateral_precision,
        static_antifee,
    })
}

fn is_power_of_10(v: U256) -> bool {
    let mut p = U256::from(1u64);
    for _ in 0..19 {
        if v == p {
            return true;
        }
        p *= U256::from(10u64);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bytecode(words: &[U256]) -> Vec<u8> {
        // Prepend some fake code, then append immutables
        let mut bytes = vec![0u8; 1000]; // fake runtime code
        for w in words {
            bytes.extend_from_slice(&w.to_be_bytes::<32>());
        }
        bytes
    }

    #[test]
    fn extract_weth_crvusd_v1() {
        // Simulated crvUSD mint layout (last 10 words)
        let words = vec![
            U256::from_str_radix("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2", 16).unwrap(), // COLLATERAL_TOKEN
            U256::from(1u64),                       // COLLATERAL_PRECISION
            U256::from(1891729517225194179952u128), // BASE_PRICE_0
            U256::from(100u64),                     // A
            U256::from(99u64),                      // A-1
            U256::from(10000u64),                   // A^2
            U256::from(9801u64),                    // (A-1)^2
            U256::from(1005037815259212075u128),    // SQRT_BAND_RATIO
            U256::from(10050335853501431u128),      // LOG_A_RATIO
            U256::from(1652875986403404071u128),    // MAX_ORACLE_DN_POW
        ];
        let bytecode = make_bytecode(&words);

        let result = extract_immutables(&bytecode, U256::from(100u64)).unwrap();
        assert_eq!(result.a, U256::from(100u64));
        assert_eq!(
            result.log_a_ratio,
            I256::try_from(10050335853501431i128).unwrap()
        );
        assert_eq!(
            result.max_oracle_dn_pow,
            U256::from(1652875986403404071u128)
        );
        assert_eq!(result.collateral_precision, U256::from(1u64));
    }

    #[test]
    fn extract_wbtc_crvusd_v1() {
        let words = vec![
            U256::from_str_radix("2260fac5e5542a773aa44fbcfedf7c193bc2c599", 16).unwrap(),
            U256::from(10_000_000_000u64), // WBTC 8 dec → 10^10
            U256::from(30494600536592806208414u128),
            U256::from(100u64),
            U256::from(99u64),
            U256::from(10000u64),
            U256::from(9801u64),
            U256::from(1005037815259212075u128),
            U256::from(10050335853501431u128),
            U256::from(1652875986403404071u128),
        ];
        let bytecode = make_bytecode(&words);

        let result = extract_immutables(&bytecode, U256::from(100u64)).unwrap();
        assert_eq!(result.collateral_precision, U256::from(10_000_000_000u64));
    }

    #[test]
    fn extract_crv_llamalend() {
        // Llamalend layout (A=30)
        let words = vec![
            U256::from(1u64),                    // COLLATERAL_PRECISION
            U256::from(832536689573559988u128),  // SQRT_BAND_RATIO
            U256::from(30u64),                   // A
            U256::from(29u64),                   // A-1
            U256::from(900u64),                  // A^2
            U256::from(841u64),                  // (A-1)^2
            U256::from(1017095255431215574u128), // ?
            U256::from(33901551675681339u128),   // LOG_A_RATIO
            U256::from(5447068553010022855u128), // MAX_ORACLE_DN_POW
            U256::from_str_radix("e1ccf80a66f9d04c2d73b3f23fc5e33c", 16).unwrap(), // ?
        ];
        let bytecode = make_bytecode(&words);

        let result = extract_immutables(&bytecode, U256::from(30u64)).unwrap();
        assert_eq!(result.a, U256::from(30u64));
        assert_eq!(
            result.log_a_ratio,
            I256::try_from(33901551675681339i128).unwrap()
        );
        assert_eq!(
            result.max_oracle_dn_pow,
            U256::from(5447068553010022855u128)
        );
        assert_eq!(result.collateral_precision, U256::from(1u64));
    }

    #[test]
    fn extract_rejects_bad_bytecode() {
        let words = vec![U256::ZERO; 10];
        let bytecode = make_bytecode(&words);
        assert!(extract_immutables(&bytecode, U256::from(100u64)).is_none());
    }

    #[test]
    fn extract_rejects_too_short() {
        let bytecode = vec![0u8; 100];
        assert!(extract_immutables(&bytecode, U256::from(100u64)).is_none());
    }
}
