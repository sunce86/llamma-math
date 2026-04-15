//! Pure Rust implementation of Curve Finance
//! [LLAMMA](https://docs.curve.finance/crvUSD/amm/) (Lending-Liquidating AMM Algorithm) math.
//!
//! Exact on-chain match — no tolerances, no approximations, wei-level precision.
//! Differentially fuzz-tested against on-chain `get_dy` for 74 pools across
//! Ethereum and Arbitrum.
//!
//! Supports both contract versions:
//! - crvUSD mint markets (Vyper 0.3.7, static antifee)
//! - Llamalend markets (Vyper 0.4.x, per-band dynamic antifee)
//!
//! # Architecture
//!
//! - **`core`** — stateless math (`wad_exp`, `get_y0`, `get_p`, band pricing,
//!   dynamic fees). Always available, zero deps beyond `alloy-primitives`.
//! - **`swap`** + **`pool`** — `LlammaPool` with band traversal swap simulation.
//!   Requires the `swap` feature.
//!
//! # Quick start
//!
//! ```rust,ignore
//! use llamma_math::pool::LlammaPool;
//! use alloy_primitives::U256;
//!
//! // Use llamma-adapter::build_pool for real pools, or construct directly:
//! let pool = LlammaPool::new(
//!     a, a_minus_1, base_price, log_a_ratio,
//!     max_oracle_dn_pow, sqrt_band_ratio,
//!     borrowed_precision, collateral_precision,
//!     fee, active_band, min_band, max_band,
//!     bands_x, bands_y, p_oracle, oracle_fee,
//!     static_antifee,
//! )?;
//!
//! let dy = pool.get_amount_out(0, 1, dx)?; // crvUSD → collateral
//! let price = pool.spot_price()?;
//! ```
//!
//! Ported line-by-line from
//! [`AMM.vy`](https://github.com/curvefi/curve-stablecoin/blob/master/curve_stablecoin/AMM.vy).

#![allow(clippy::too_many_arguments)]

pub mod constants;
pub mod core;

#[cfg(feature = "swap")]
pub mod swap;

#[cfg(feature = "swap")]
pub mod pool;
