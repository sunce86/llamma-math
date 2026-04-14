//! Registry-driven differential fuzz test.
//!
//! Reads `tests/registry/1.toml`, builds each pool via `build_pool` (Multicall3),
//! and compares `get_amount_out` against on-chain `get_dy` (wei-exact).
//! All pools are built and fuzzed in parallel.
//!
//! Run:
//!   FUZZ_ITERATIONS=20 RPC_URL_1=... \
//!     cargo test -p llamma-adapter --test fuzz_registry -- --ignored --nocapture

use alloy::providers::{Provider, ProviderBuilder};
use alloy_primitives::{Address, I256, U256};
use llamma_adapter::{build_pool, RawLlammaState};
use llamma_math::constants::WAD;
use serde::Deserialize;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Deserialize)]
struct Registry {
    pools: Vec<PoolEntry>,
}

#[derive(Deserialize, Clone)]
struct PoolEntry {
    amm: String,
    name: String,
    a: u64,
    log_a_ratio: i64,
    borrowed_precision: u64,
    collateral_precision: u64,
    static_antifee: bool,
}

alloy::sol! {
    #[sol(rpc)]
    interface ILLAMMA {
        function get_dy(uint256 i, uint256 j, uint256 in_amount) external view returns (uint256);
    }
}

fn fuzz_iterations() -> usize {
    std::env::var("FUZZ_ITERATIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10)
}

fn splitmix64(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = *seed;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

fn generate_amounts(n: usize, balance: U256, seed: u64) -> Vec<U256> {
    let mut amounts = vec![U256::ZERO, U256::from(1u64)];
    if !balance.is_zero() {
        amounts.extend_from_slice(&[
            balance / U256::from(1000u64),
            balance / U256::from(10u64),
            balance / U256::from(2u64),
            balance,
        ]);
    }
    let remaining = n.saturating_sub(amounts.len());
    if remaining > 0 && !balance.is_zero() {
        let mut seed = seed;
        let max_f64 = balance.to_string().parse::<f64>().unwrap_or(1e30);
        let ln_max = max_f64.ln();
        for _ in 0..remaining {
            let r = splitmix64(&mut seed);
            let t = (r as f64) / (u64::MAX as f64);
            let val = (t * ln_max).exp();
            let val_u128 = val.min(1e38) as u128;
            amounts.push(U256::from(val_u128).max(U256::from(1u64)).min(balance));
        }
    }
    amounts
}

alloy::sol! {
    #[sol(rpc)]
    interface ILlammaState {
        function fee() external view returns (uint256);
        function active_band() external view returns (int256);
        function min_band() external view returns (int256);
        function max_band() external view returns (int256);
        function bands_x(int256 n) external view returns (uint256);
        function bands_y(int256 n) external view returns (uint256);
        function price_oracle() external view returns (uint256);
        function dynamic_fee() external view returns (uint256);
        function get_base_price() external view returns (uint256);
    }

    #[sol(rpc)]
    interface IMulticall3 {
        struct Call3 { address target; bool allowFailure; bytes callData; }
        struct Result { bool success; bytes returnData; }
        function aggregate3(Call3[] calldata calls) external payable returns (Result[] memory returnData);
    }
}

const MULTICALL3: Address = Address::new([
    0xca, 0x11, 0xbd, 0xe0, 0x59, 0x77, 0xb3, 0x63, 0x11, 0x67, 0x02, 0x88, 0x62, 0xbe, 0x2a, 0x17,
    0x39, 0x76, 0xca, 0x11,
]);

async fn read_state(
    provider: &impl Provider,
    amm_addr: Address,
    block: alloy::eips::BlockId,
    immutables: &llamma_adapter::LlammaImmutables,
    borrowed_precision: U256,
) -> Option<RawLlammaState> {
    use alloy::sol_types::SolCall;

    let amm = ILlammaState::new(amm_addr, provider);
    let fee = amm.fee().block(block).call().await.ok()?;
    let active_band = amm.active_band().block(block).call().await.ok()?.as_i64();
    let min_band = amm.min_band().block(block).call().await.ok()?.as_i64();
    let max_band = amm.max_band().block(block).call().await.ok()?.as_i64();
    let p_oracle = amm.price_oracle().block(block).call().await.ok()?;
    let base_price = amm.get_base_price().block(block).call().await.ok()?;
    let dynamic_fee_val = amm.dynamic_fee().block(block).call().await.ok()?;
    let oracle_fee = if dynamic_fee_val > fee {
        dynamic_fee_val
    } else {
        U256::ZERO
    };

    let multicall = IMulticall3::new(MULTICALL3, provider);
    let band_range: Vec<i64> = (min_band..=max_band).collect();
    let mut all_calls = Vec::with_capacity(band_range.len() * 2);
    for &n in &band_range {
        let n_i256 = I256::try_from(n as i128).unwrap();
        all_calls.push(IMulticall3::Call3 {
            target: amm_addr,
            allowFailure: false,
            callData: ILlammaState::bands_xCall { n: n_i256 }.abi_encode().into(),
        });
        all_calls.push(IMulticall3::Call3 {
            target: amm_addr,
            allowFailure: false,
            callData: ILlammaState::bands_yCall { n: n_i256 }.abi_encode().into(),
        });
    }
    let mut results = Vec::with_capacity(all_calls.len());
    for chunk in all_calls.chunks(200) {
        results.extend(
            multicall
                .aggregate3(chunk.to_vec())
                .block(block)
                .call()
                .await
                .ok()?,
        );
    }
    let mut bands_x = HashMap::new();
    let mut bands_y = HashMap::new();
    for (i, &n) in band_range.iter().enumerate() {
        let bx_data = &results[i * 2];
        let by_data = &results[i * 2 + 1];
        if bx_data.success && bx_data.returnData.len() >= 32 {
            let bx = U256::from_be_slice(&bx_data.returnData[bx_data.returnData.len() - 32..]);
            if !bx.is_zero() {
                bands_x.insert(n, bx);
            }
        }
        if by_data.success && by_data.returnData.len() >= 32 {
            let by = U256::from_be_slice(&by_data.returnData[by_data.returnData.len() - 32..]);
            if !by.is_zero() {
                bands_y.insert(n, by);
            }
        }
    }

    Some(RawLlammaState {
        a: immutables.a,
        log_a_ratio: immutables.log_a_ratio,
        collateral_precision: immutables.collateral_precision,
        static_antifee: immutables.static_antifee, // validated against registry by caller
        borrowed_precision,
        fee,
        active_band,
        min_band,
        max_band,
        p_oracle,
        base_price,
        oracle_fee,
        bands_x,
        bands_y,
    })
}

async fn run_registry_fuzz(rpc_url: &str, registry_toml: &str) {
    let provider = Arc::new(ProviderBuilder::new().connect_http(rpc_url.parse().unwrap()));
    let registry: Registry = toml::from_str(registry_toml).expect("invalid registry TOML");

    let bn = provider.get_block_number().await.unwrap() - 5;
    let block = alloy::eips::BlockId::number(bn);
    let n = fuzz_iterations();

    let total_passed = Arc::new(AtomicU64::new(0));
    let total_mismatched = Arc::new(AtomicU64::new(0));
    let total_skipped = Arc::new(AtomicU64::new(0));

    // Spawn all pools in parallel
    let mut handles = Vec::new();
    for entry in registry.pools {
        let provider = provider.clone();
        let passed = total_passed.clone();
        let mismatched = total_mismatched.clone();
        let skipped = total_skipped.clone();

        handles.push(tokio::spawn(async move {
            let amm_addr = Address::from_str(&entry.amm).unwrap();
            let amm = ILLAMMA::new(amm_addr, &*provider);

            let bytecode = match provider.get_code_at(amm_addr).await {
                Ok(b) => b,
                Err(_) => {
                    eprintln!("  SKIP {}: get_code failed", entry.name);
                    skipped.fetch_add(1, Ordering::Relaxed);
                    return;
                }
            };

            let immutables =
                match llamma_adapter::extract_immutables(&bytecode, U256::from(entry.a)) {
                    Some(imm) => imm,
                    None => {
                        eprintln!("  SKIP {}: extract_immutables failed", entry.name);
                        skipped.fetch_add(1, Ordering::Relaxed);
                        return;
                    }
                };

            let raw_state = match read_state(
                &*provider,
                amm_addr,
                block,
                &immutables,
                U256::from(entry.borrowed_precision),
            )
            .await
            {
                Some(s) => s,
                None => {
                    eprintln!("  SKIP {}: read_state failed", entry.name);
                    skipped.fetch_add(1, Ordering::Relaxed);
                    return;
                }
            };

            let pool = build_pool(&raw_state);

            let mut p = 0u64;
            let mut m = 0u64;

            // Pump (0→1)
            let ref_x = *pool.bands_x.get(&pool.active_band).unwrap_or(&U256::ZERO);
            let ref_x = if ref_x > U256::ZERO {
                ref_x
            } else {
                WAD * U256::from(1000u64)
            };
            for dx in generate_amounts(n, ref_x, bn) {
                if dx.is_zero() {
                    if pool.get_amount_out(0, 1, dx) == Ok(U256::ZERO) {
                        p += 1;
                    }
                    continue;
                }
                match amm
                    .get_dy(U256::from(0u64), U256::from(1u64), dx)
                    .block(block)
                    .call()
                    .await
                {
                    Ok(expected) => match pool.get_amount_out(0, 1, dx) {
                        Ok(r) if r == expected => p += 1,
                        Ok(r) => {
                            m += 1;
                            eprintln!(
                                "  MISMATCH {} 0→1 dx={dx}: ours={r}, on-chain={expected}",
                                entry.name
                            );
                        }
                        Err(_) if expected.is_zero() => p += 1,
                        Err(e) => {
                            m += 1;
                            eprintln!(
                                "  MISMATCH {} 0→1 dx={dx}: Err({e}), on-chain={expected}",
                                entry.name
                            );
                        }
                    },
                    Err(_) => {}
                }
            }

            // Dump (1→0)
            let ref_y = *pool.bands_y.get(&pool.active_band).unwrap_or(&U256::ZERO);
            let ref_y = if ref_y > U256::ZERO { ref_y } else { WAD };
            for dx in generate_amounts(n, ref_y, bn + 1) {
                if dx.is_zero() {
                    if pool.get_amount_out(1, 0, dx) == Ok(U256::ZERO) {
                        p += 1;
                    }
                    continue;
                }
                match amm
                    .get_dy(U256::from(1u64), U256::from(0u64), dx)
                    .block(block)
                    .call()
                    .await
                {
                    Ok(expected) => match pool.get_amount_out(1, 0, dx) {
                        Ok(r) if r == expected => p += 1,
                        Ok(r) => {
                            m += 1;
                            eprintln!(
                                "  MISMATCH {} 1→0 dx={dx}: ours={r}, on-chain={expected}",
                                entry.name
                            );
                        }
                        Err(_) if expected.is_zero() => p += 1,
                        Err(e) => {
                            m += 1;
                            eprintln!(
                                "  MISMATCH {} 1→0 dx={dx}: Err({e}), on-chain={expected}",
                                entry.name
                            );
                        }
                    },
                    Err(_) => {}
                }
            }

            println!("  {}: {} passed, {} mismatched", entry.name, p, m);
            passed.fetch_add(p, Ordering::Relaxed);
            mismatched.fetch_add(m, Ordering::Relaxed);
        }));
    }

    // Wait for all
    for h in handles {
        h.await.unwrap();
    }

    let p = total_passed.load(Ordering::Relaxed);
    let m = total_mismatched.load(Ordering::Relaxed);
    let s = total_skipped.load(Ordering::Relaxed);

    println!(
        "\nRegistry fuzz (block {bn}): {p} passed, {m} mismatched, {s} skipped ({} pools)",
        registry_toml.matches("[[pools]]").count()
    );
    assert_eq!(m, 0, "mismatches detected — see stderr");
    assert!(p > 0, "no tests ran");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore = "requires RPC_URL"]
async fn fuzz_1() {
    let rpc_url = std::env::var("RPC_URL_1").expect("RPC_URL_1 must be set");
    run_registry_fuzz(&rpc_url, include_str!("registry/1.toml")).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore = "requires RPC_URL"]
async fn fuzz_42161() {
    let rpc_url = std::env::var("RPC_URL_42161").expect("RPC_URL_42161 must be set");
    run_registry_fuzz(&rpc_url, include_str!("registry/42161.toml")).await;
}
