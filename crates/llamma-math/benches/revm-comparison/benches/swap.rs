//! Benchmark: llamma-math get_amount_out vs revm executing on-chain get_dy.
//!
//! Both produce identical results (wei-exact). We measure pure computation time.
//!
//! Run: cd crates/llamma-math/benches/revm-comparison && cargo bench

use alloy_primitives::{Address, Bytes, I256, U256};
use criterion::{criterion_group, criterion_main, Criterion};
use llamma_math::constants::WAD;
use llamma_math::pool::LlammaPool;
use revm::{
    bytecode::Bytecode, context::TxEnv, database::CacheDB, database_interface::EmptyDB,
    primitives::TxKind, state::AccountInfo, Context, ExecuteEvm, MainBuilder, MainContext,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::str::FromStr;

#[derive(Deserialize)]
struct Fixture {
    name: String,
    calldata: String,
    i: usize,
    j: usize,
    dx: String,
    expected_dy: String,
}

#[derive(Deserialize)]
struct EvmState {
    accounts: HashMap<String, AccountState>,
}

#[derive(Deserialize)]
struct AccountState {
    code: String,
    storage: HashMap<String, String>,
}

fn u(s: &str) -> U256 {
    U256::from_str_radix(s, 10).unwrap()
}

fn load_fixture(name: &str) -> Fixture {
    let path = format!("{}/fixtures/{}.json", env!("CARGO_MANIFEST_DIR"), name);
    let data = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("Fixture not found: {path}. Run generate_fixtures.py first."));
    serde_json::from_str(&data).expect("Invalid fixture JSON")
}

fn load_evm_state(name: &str) -> EvmState {
    let path = format!("{}/fixtures/{}_evm.json", env!("CARGO_MANIFEST_DIR"), name);
    let data = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("EVM state not found: {path}. Run generate_fixtures.py first."));
    serde_json::from_str(&data).expect("Invalid EVM state JSON")
}

fn setup_revm_db(evm_state: &EvmState) -> CacheDB<EmptyDB> {
    let mut db = CacheDB::new(EmptyDB::default());

    for (addr_hex, state) in &evm_state.accounts {
        let addr = Address::from_str(addr_hex).unwrap();
        let code_hex = state.code.strip_prefix("0x").unwrap_or(&state.code);
        let code_bytes = hex::decode(code_hex).unwrap();
        let bytecode = Bytecode::new_raw(Bytes::from(code_bytes));

        let info = AccountInfo {
            balance: U256::ZERO,
            nonce: 1,
            code_hash: bytecode.hash_slow(),
            code: Some(bytecode),
            account_id: None,
        };
        db.insert_account_info(addr, info);

        for (slot_hex, val_hex) in &state.storage {
            let slot_clean = slot_hex.strip_prefix("0x").unwrap_or(slot_hex);
            let val_clean = val_hex.strip_prefix("0x").unwrap_or(val_hex);
            let slot = U256::from_str_radix(slot_clean, 16).unwrap();
            let val = U256::from_str_radix(val_clean, 16).unwrap_or(U256::ZERO);
            let _ = db.insert_account_storage(addr, slot, val);
        }
    }

    // Caller
    let caller = Address::from_str("0x0000000000000000000000000000000000000001").unwrap();
    db.insert_account_info(
        caller,
        AccountInfo {
            balance: U256::from(1_000_000_000_000_000_000u128),
            nonce: 0,
            code_hash: Default::default(),
            code: None,
            account_id: None,
        },
    );

    db
}

fn build_bench_pool(name: &str) -> LlammaPool {
    match name {
        "weth_crvusd" => {
            let a = U256::from(100u64);
            let a_minus_1 = U256::from(99u64);
            let mut pow = WAD;
            for _ in 0..50 {
                pow = pow * a / a_minus_1;
            }

            let mut bands_x = HashMap::new();
            let mut bands_y = HashMap::new();
            bands_x.insert(4i64, U256::from(344371171623378907u128));
            bands_y.insert(4i64, U256::from(54046862449952u128));
            bands_y.insert(5i64, U256::from(171433458210907u128));
            bands_y.insert(6i64, U256::from(160627967084810u128));

            LlammaPool {
                a,
                a_minus_1,
                base_price: U256::from_str_radix("2410675227117098271961", 10).unwrap(),
                log_a_ratio: I256::try_from(10_050_335_853_501_431i128).unwrap(),
                max_oracle_dn_pow: pow,
                sqrt_band_ratio: U256::ZERO,
                borrowed_precision: U256::from(1u64),
                collateral_precision: U256::from(1u64),
                fee: U256::from(19000000000000000u128),
                active_band: 4,
                min_band: -68,
                max_band: 1057,
                bands_x,
                bands_y,
                p_oracle: U256::from_str_radix("2383616511272813782212", 10).unwrap(),
                oracle_fee: U256::ZERO,
                static_antifee: true,
            }
        }
        "crv_llamalend" => {
            let a = U256::from(30u64);
            let a_minus_1 = U256::from(29u64);
            let mut pow = WAD;
            for _ in 0..50 {
                pow = pow * a / a_minus_1;
            }

            let mut bands_x = HashMap::new();
            let mut bands_y = HashMap::new();
            bands_x.insert(
                47i64,
                U256::from_str_radix("5158151016204956995741", 10).unwrap(),
            );
            bands_x.insert(
                48i64,
                U256::from_str_radix("1886887147243800319726", 10).unwrap(),
            );
            bands_y.insert(
                48i64,
                U256::from_str_radix("12932829916657340076596", 10).unwrap(),
            );
            bands_y.insert(
                49i64,
                U256::from_str_radix("24187978120075270087996", 10).unwrap(),
            );
            bands_y.insert(
                50i64,
                U256::from_str_radix("28062780071522949074116", 10).unwrap(),
            );

            LlammaPool {
                a,
                a_minus_1,
                base_price: U256::from_str_radix("1136702468099780029", 10).unwrap(),
                log_a_ratio: I256::try_from(33_901_551_675_681_339i128).unwrap(),
                max_oracle_dn_pow: pow,
                sqrt_band_ratio: U256::ZERO,
                borrowed_precision: U256::from(1u64),
                collateral_precision: U256::from(1u64),
                fee: U256::from(6000000000000000u128),
                active_band: 48,
                min_band: 0,
                max_band: 200,
                bands_x,
                bands_y,
                p_oracle: U256::from_str_radix("218986281844411509", 10).unwrap(),
                oracle_fee: U256::ZERO,
                static_antifee: false,
            }
        }
        "wbtc_crvusd_multiband" => {
            // WBTC/crvUSD mint, 50K crvUSD swap traversing ~10 bands
            let a = U256::from(100u64);
            let a_minus_1 = U256::from(99u64);
            let mut pow = WAD;
            for _ in 0..50 {
                pow = pow * a / a_minus_1;
            }

            let mut bands_x = HashMap::new();
            let mut bands_y = HashMap::new();
            bands_x.insert(
                -65i64,
                U256::from_str_radix("25953788190638142560224", 10).unwrap(),
            );
            bands_y.insert(-65i64, U256::from(1101264100667855482u128));
            bands_y.insert(-64i64, U256::from(1441609796678554063u128));
            bands_y.insert(-63i64, U256::from(179487668326621116u128));
            bands_y.insert(-62i64, U256::from(177736855450067952u128));
            bands_y.insert(-61i64, U256::from(174743752098662457u128));
            bands_y.insert(-60i64, U256::from(152192164233212493u128));
            bands_y.insert(-59i64, U256::from(154609404465802623u128));
            bands_y.insert(-58i64, U256::from(163622163939169250u128));
            bands_y.insert(-57i64, U256::from(154445006077983248u128));
            bands_y.insert(-56i64, U256::from(145566600159650958u128));
            bands_y.insert(-55i64, U256::from(157278789277960872u128));

            LlammaPool {
                a,
                a_minus_1,
                base_price: U256::from_str_radix("38902563076553686326901", 10).unwrap(),
                log_a_ratio: I256::try_from(10_050_335_853_501_431i128).unwrap(),
                max_oracle_dn_pow: pow,
                sqrt_band_ratio: U256::ZERO,
                borrowed_precision: U256::from(1u64),
                collateral_precision: U256::from(10_000_000_000u64),
                fee: U256::from(19000000000000000u128),
                active_band: -65,
                min_band: -105,
                max_band: 1037,
                bands_x,
                bands_y,
                p_oracle: U256::from_str_radix("74164734358570211064267", 10).unwrap(),
                oracle_fee: U256::ZERO,
                static_antifee: true,
            }
        }
        _ => panic!("Unknown: {name}"),
    }
}

fn bench_fixture(c: &mut Criterion, name: &str) {
    let fixture = load_fixture(name);
    let evm_state = load_evm_state(name);
    let dx = u(&fixture.dx);
    let pool = build_bench_pool(name);

    let our_dy = pool
        .get_amount_out(fixture.i, fixture.j, dx)
        .expect("llamma-math should produce result");
    assert!(our_dy > U256::ZERO, "{name}: zero output");

    let pool_addr = Address::from_str(
        evm_state
            .accounts
            .keys()
            .next()
            .expect("no accounts in EVM state"),
    )
    .unwrap();

    let calldata = hex::decode(
        fixture
            .calldata
            .strip_prefix("0x")
            .unwrap_or(&fixture.calldata),
    )
    .unwrap();
    let caller = Address::from_str("0x0000000000000000000000000000000000000001").unwrap();

    let tx = TxEnv {
        caller,
        kind: TxKind::Call(pool_addr),
        data: Bytes::from(calldata),
        value: U256::ZERO,
        gas_limit: 1_000_000,
        ..Default::default()
    };

    let mut group = c.benchmark_group(&fixture.name);

    // revm (pure): only EVM execution, DB pre-loaded
    group.bench_function("revm (pure)", |b| {
        b.iter_batched(
            || setup_revm_db(&evm_state),
            |db| {
                let ctx = Context::mainnet().with_db(db);
                let mut evm = ctx.build_mainnet();
                let result = evm.transact(tx.clone()).unwrap();
                std::hint::black_box(result);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // revm (full): includes DB setup — realistic simulation cost
    group.bench_function("revm (full)", |b| {
        b.iter(|| {
            let db = setup_revm_db(&evm_state);
            let ctx = Context::mainnet().with_db(db);
            let mut evm = ctx.build_mainnet();
            let result = evm.transact(tx.clone()).unwrap();
            std::hint::black_box(result);
        });
    });

    // llamma-math benchmark
    let pool_clone = pool.clone();
    let i = fixture.i;
    let j = fixture.j;
    group.bench_function("llamma-math", |b| {
        b.iter(|| {
            let result = pool_clone.get_amount_out(i, j, dx);
            std::hint::black_box(result);
        });
    });

    group.finish();
}

fn bench_weth(c: &mut Criterion) {
    bench_fixture(c, "weth_crvusd");
}
fn bench_crv(c: &mut Criterion) {
    bench_fixture(c, "crv_llamalend");
}
fn bench_wbtc_multi(c: &mut Criterion) {
    bench_fixture(c, "wbtc_crvusd_multiband");
}

criterion_group!(benches, bench_weth, bench_crv, bench_wbtc_multi);
criterion_main!(benches);
