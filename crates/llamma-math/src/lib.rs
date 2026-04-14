//! Pure Rust implementation of Curve Finance LLAMMA
//! (Lending-Liquidating AMM Algorithm) math.
//!
//! Exact on-chain match — no tolerances, no approximations, wei-level precision.
//! Differentially fuzz-tested against on-chain LLAMMA contracts.
//!
//! # Architecture
//!
//! - **`constants`** — protocol constants (`WAD`, `MAX_TICKS`, etc.).
//! - **`core`** — stateless math functions (`wad_exp`, `get_y0`, `get_p`,
//!   band pricing, dynamic fees).
//!
//! # Source
//!
//! Ported line-by-line from:
//! - [`AMM.vy`](https://github.com/curvefi/curve-stablecoin/blob/master/curve_stablecoin/AMM.vy)

#![allow(clippy::too_many_arguments)]

pub mod constants;
pub mod core;
