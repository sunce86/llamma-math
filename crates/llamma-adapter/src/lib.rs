//! Data-source agnostic pool construction for LLAMMA.
//!
//! - [`extract_immutables`] — reads `A`, `LOG_A_RATIO`, `COLLATERAL_PRECISION`
//!   from deployed bytecode. Auto-detects Vyper 0.3.x vs 0.4.x.
//! - [`build_pool`] — converts flat [`RawLlammaState`] into a `LlammaPool`.
//!   Pure, no I/O. Requires the `build` feature (default).
//!
//! ```rust,ignore
//! use llamma_adapter::{RawLlammaState, build_pool, extract_immutables};
//!
//! let imm = extract_immutables(&bytecode, a).unwrap();
//! let state = RawLlammaState {
//!     a: imm.a, log_a_ratio: imm.log_a_ratio,
//!     collateral_precision: imm.collateral_precision,
//!     static_antifee: imm.static_antifee,
//!     borrowed_precision, fee, active_band, min_band, max_band,
//!     p_oracle, base_price, oracle_fee, bands_x, bands_y,
//! };
//! let pool = build_pool(&state)?;
//! let dy = pool.get_amount_out(0, 1, dx)?;
//! ```

pub mod immutables;

#[cfg(feature = "build")]
pub mod build;

#[cfg(feature = "build")]
pub use build::{build_pool, RawLlammaState};
pub use immutables::{extract_immutables, LlammaImmutables};
