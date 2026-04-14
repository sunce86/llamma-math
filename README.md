# llamma-math

[![CI](https://github.com/sunce86/llamma-math/actions/workflows/unit-tests.yml/badge.svg)](https://github.com/sunce86/llamma-math/actions/workflows/unit-tests.yml)
[![crates.io](https://img.shields.io/crates/v/llamma-math.svg)](https://crates.io/crates/llamma-math)
[![docs.rs](https://docs.rs/llamma-math/badge.svg)](https://docs.rs/llamma-math)

Pure Rust implementation of Curve Finance **LLAMMA** (Lending-Liquidating AMM Algorithm) math.
Wei-level precision, differentially fuzz-tested against on-chain contracts.

Used in crvUSD mint markets and Llamalend (Curve Lending).

## Verified Pools

Every registered pool is **differentially fuzz-tested** against its on-chain
`get_dy`: our `get_amount_out` is called with random swap amounts and the
result is compared with an on-chain call at the same block. The test requires
**exact wei-level match** — no tolerances, no approximations.

[![Fuzz](https://github.com/sunce86/llamma-math/actions/workflows/fuzz.yml/badge.svg)](https://github.com/sunce86/llamma-math/actions/workflows/fuzz.yml)

| Chain | Verified pools | Last indexed |
|-------|---------------|-------------|
| Ethereum | 53 / 53 ![](https://geps.dev/progress/100?successColor=6366f1) | 2026-04-14 |
| Arbitrum | 21 / 21 ![](https://geps.dev/progress/100?successColor=6366f1) | 2026-04-14 |

## Performance

Compared against [revm](https://github.com/bluealloy/revm) executing the
same pool's on-chain `get_dy` bytecode. Both produce identical results (wei-exact).

**Realistic simulation** — revm with full EVM setup (DB, accounts, storage, bytecode):

| Pool type | llamma-math | revm | Speedup |
|-----------|-------------|------|---------|
| crvUSD mint WETH, 1-band (A=100) | 3.4 µs | 192 µs | **56x** |
| Llamalend CRV, 1-band (A=30) | 4.0 µs | 166 µs | **42x** |
| crvUSD mint WBTC, 10-band (A=100) | 4.2 µs | 192 µs | **46x** |

Measured with [criterion](https://github.com/bheisler/criterion.rs) on Apple M1.

## Contract Versions

There are two deployed versions of LLAMMA AMM contracts with different
swap behavior. The adapter auto-detects the version from the bytecode.

| | crvUSD mint | Llamalend |
|---|---|---|
| Compiler | Vyper 0.3.7 | Vyper 0.4.x |
| Pools | 6 (Ethereum) | 47+ (Ethereum, Arbitrum) |
| `A` | 100 (all) | 30–200 |
| Antifee | **Static** (computed once before loop) | **Dynamic** (recomputed per band via `get_dynamic_fee`) |
| Admin fee | Yes (`admin_fee = 1`) | No |
| `static_antifee` flag | `true` | `false` |

**Note for integrators:** This library currently provides read-only swap
quotes (`get_amount_out`). A future `apply_swap` function that returns
`(output_amount, new_pool_state)` — correctly handling `admin_fee`
differences between versions — would enable sequential swap simulation
(e.g., split routing in solvers) without exposing version details to
the caller.

Verified sources: [Etherscan](https://etherscan.io/address/0x1681195c176239ac5e72d9aebacf5b2492e0c4ee#code)
(Vyper 0.3.7). Note: the GitHub V1 tag contains newer code (with dynamic
per-band fee) that was never deployed to the mint markets.

## Crates

| Crate | Description |
|-------|-------------|
| [`llamma-math`](crates/llamma-math) | Core math library. Stateless functions + `LlammaPool` for swap quotes. Only depends on `alloy-primitives`. |
| [`llamma-adapter`](crates/llamma-adapter) | Reads on-chain state and constructs `LlammaPool` instances. Extracts immutables from bytecode, batch-reads bands via Multicall3. |

## Usage

### With llamma-adapter (reads pool from chain)

```rust
use llamma_adapter::{extract_immutables, build_pool};

// 1. Extract deploy-time constants from bytecode
let bytecode = provider.get_code_at(amm_addr).await?;
let a = /* read A() from contract */;
let immutables = extract_immutables(&bytecode, a).unwrap();

// 2. Build pool snapshot at a specific block
let pool = build_pool(&provider, amm_addr, block, &immutables, borrowed_precision).await?;

// 3. Quote a swap: 1000 crvUSD -> WETH
let dy = pool.get_amount_out(0, 1, U256::from(1000) * WAD)?;
```

### With llamma-math only (bring your own state)

```rust
use llamma_math::pool::LlammaPool;

let pool = LlammaPool {
    a, a_minus_1, base_price, log_a_ratio,
    // ... see LlammaPool fields
};

// Swap quote
let dy = pool.get_amount_out(0, 1, dx)?;

// Spot price
let price = pool.spot_price()?;
```

## Structure

```
llamma-math/
├── crates/
│   ├── llamma-math/           Pure math library
│   │   ├── src/
│   │   │   ├── constants.rs   Protocol constants (WAD, MAX_TICKS, ...)
│   │   │   ├── core.rs        Stateless math (wad_exp, get_y0, get_p, ...)
│   │   │   ├── swap.rs        Band traversal (calc_swap_out)
│   │   │   └── pool.rs        LlammaPool struct + get_amount_out
│   │   └── tests/
│   │       └── wad_exp_crosscheck.rs  Independent exp verification
│   └── llamma-adapter/        On-chain state reading
│       ├── src/
│       │   ├── immutables.rs  Extract constants from bytecode
│       │   └── build.rs       Build LlammaPool via Multicall3
│       └── tests/
│           ├── extract_onchain.rs    Immutables extraction test
│           ├── fuzz_registry.rs      Registry-driven differential fuzz
│           └── registry/1.toml       Pool registry (Ethereum)
├── Cargo.toml                 Workspace root
└── README.md
```

## License

MIT
