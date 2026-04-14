//! Integration test: extract immutables from real on-chain bytecode.
//!
//! Run:
//!   RPC_URL=... cargo test -p llamma-adapter --test extract_onchain -- --ignored --nocapture

use alloy::providers::ProviderBuilder;
use alloy_primitives::{Address, I256, U256};
use llamma_adapter::extract_immutables;
use std::str::FromStr;

macro_rules! make_provider {
    () => {{
        let rpc_url = std::env::var("RPC_URL_1").expect("RPC_URL must be set");
        ProviderBuilder::new().connect_http(rpc_url.parse().expect("invalid RPC_URL"))
    }};
}

alloy::sol! {
    #[sol(rpc)]
    interface ILLAMMA {
        function A() external view returns (uint256);
    }
}

async fn test_market(addr_str: &str, expected_a: u64, expected_lar: i128, expected_coll_prec: u64) {
    let provider = make_provider!();
    let addr = Address::from_str(addr_str).unwrap();
    let amm = ILLAMMA::new(addr, &provider);

    let a: U256 = amm.A().call().await.unwrap();
    assert_eq!(a, U256::from(expected_a), "A mismatch for {addr_str}");

    let bytecode = provider.get_code_at(addr).await.unwrap();

    let imm = extract_immutables(&bytecode, a)
        .expect(&format!("failed to extract immutables for {addr_str}"));

    assert_eq!(imm.a, U256::from(expected_a));
    assert_eq!(
        imm.log_a_ratio,
        I256::try_from(expected_lar).unwrap(),
        "LOG_A_RATIO mismatch for {addr_str}"
    );
    assert_eq!(
        imm.collateral_precision,
        U256::from(expected_coll_prec),
        "COLLATERAL_PRECISION mismatch for {addr_str}"
    );

    println!(
        "{addr_str}: A={}, LAR={}, coll_prec={}, max_dn_pow={}",
        imm.a, imm.log_a_ratio, imm.collateral_precision, imm.max_oracle_dn_pow
    );
}

use alloy::providers::Provider;

#[tokio::test]
#[ignore = "requires RPC_URL"]
async fn extract_weth_crvusd() {
    test_market(
        "0x1681195C176239ac5E72d9aeBaCf5b2492E0C4ee",
        100,
        10_050_335_853_501_431,
        1,
    )
    .await;
}

#[tokio::test]
#[ignore = "requires RPC_URL"]
async fn extract_wbtc_crvusd() {
    test_market(
        "0xE0438Eb3703bF871E31Ce639BD351109c88666eA",
        100,
        10_050_335_853_501_431,
        10_000_000_000,
    )
    .await;
}

#[tokio::test]
#[ignore = "requires RPC_URL"]
async fn extract_wsteth_crvusd() {
    test_market(
        "0x37417B2238AA52D0DD2d6252d989E728e8f706e4",
        100,
        10_050_335_853_501_431,
        1,
    )
    .await;
}

#[tokio::test]
#[ignore = "requires RPC_URL"]
async fn extract_crv_llamalend() {
    test_market(
        "0xafca625321Df8D6A068bDD8F1585d489D2acF11b",
        30,
        33_901_551_675_681_339,
        1,
    )
    .await;
}
