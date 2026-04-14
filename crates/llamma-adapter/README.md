# llamma-adapter

Adapts raw on-chain LLAMMA data into [`llamma-math::LlammaPool`](https://crates.io/crates/llamma-math) instances ready for swap computation.

## What it does

- **Immutables extraction** — reads `A`, `LOG_A_RATIO`, `MAX_ORACLE_DN_POW`, and `COLLATERAL_PRECISION` from deployed bytecode
- **Version detection** — auto-detects Vyper 0.3.x (crvUSD mint, static antifee) vs 0.4.x (Llamalend, per-band antifee) from bytecode prefix
- **Pool construction** — `build_pool(RawLlammaState) -> LlammaPool` with precomputation and validation

## Usage

```rust
use llamma_adapter::{RawLlammaState, build_pool};
use alloy_primitives::U256;

// Fill in RawLlammaState from your data source (RPC, indexer, database)
let state = RawLlammaState {
    a: U256::from(100u64),
    log_a_ratio, collateral_precision, borrowed_precision,
    static_antifee: true,
    fee, active_band, min_band, max_band,
    p_oracle, base_price, oracle_fee,
    bands_x, bands_y,
};

// Build pool (pure, no I/O)
let pool = build_pool(&state);
let dy = pool.get_amount_out(0, 1, dx).unwrap();
```

## Data-source agnostic

`RawLlammaState` is a flat struct with no nested types — populate it from any source:

- **RPC** — call `A()`, `fee()`, `active_band()`, `bands_x(n)`, etc. directly; use `extract_immutables()` for bytecode-derived values
- **Substreams / indexer** — decode storage changes into the struct fields
- **Database** — load a snapshot and fill in the struct
