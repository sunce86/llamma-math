//! Adapts raw LLAMMA data into [`llamma_math::pool::LlammaPool`] instances.
//!
//! # Architecture
//!
//! - [`RawLlammaState`] — plain struct holding all pool parameters. Can be
//!   populated from any data source (RPC, substream indexer, database).
//! - [`build_pool`] — pure function that converts `RawLlammaState` into a
//!   `LlammaPool` ready for swap computation. No I/O.
//!
//! ```ignore
//! let state = RawLlammaState { /* populate from your data source */ };
//! let pool = build_pool(&state);
//! let dy = pool.get_amount_out(0, 1, dx)?;
//! ```

pub mod immutables;

#[cfg(feature = "build")]
pub mod build;

#[cfg(feature = "build")]
pub use build::{build_pool, RawLlammaState};
pub use immutables::{extract_immutables, LlammaImmutables};
