//! Pure Rust implementation of Curve Finance
//! [LLAMMA](https://docs.curve.finance/crvUSD/amm/) (Lending-Liquidating AMM Algorithm) math.
//!
//! Exact on-chain match — no tolerances, no approximations, wei-level precision.
//! Differentially fuzz-tested against on-chain `get_dy` for 74 pools across 2 chains.
//!
//! # Architecture
//!
//! - **`constants`** — protocol constants (`WAD`, `MAX_TICKS`, etc.).
//! - **`core`** — stateless math functions (`wad_exp`, `get_y0`, `get_p`,
//!   band pricing, dynamic fees). Always available, zero dependencies beyond
//!   `alloy-primitives`.
//! - **`swap`** + **`LlammaPool`** — pool simulation with band traversal,
//!   fee computation, and precision scaling. Requires the `swap` feature.
//!
//! # Quick start
//!
//! ```ignore
//! use llamma_math::pool::LlammaPool;
//!
//! // Build a pool (or use llamma-adapter to read from chain)
//! let pool = LlammaPool { /* ... */ };
//!
//! // Quote a swap: crvUSD → collateral
//! let dy = pool.get_amount_out(0, 1, dx)?;
//!
//! // Spot price in active band
//! let price = pool.spot_price()?;
//! ```
//!
//! # Source
//!
//! Ported line-by-line from:
//! - [`AMM.vy`](https://github.com/curvefi/curve-stablecoin/blob/master/curve_stablecoin/AMM.vy)

#![allow(clippy::too_many_arguments)]

pub mod constants;
pub mod core;

#[cfg(feature = "swap")]
pub mod swap;

#[cfg(feature = "swap")]
pub mod pool;
