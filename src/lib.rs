//! Library surface of the `sni-gate` package.
//!
//! The gateway binary (`src/main.rs`) declares these same modules privately and
//! does not link this library, so nothing here changes how `sni-gate` itself is
//! built. The surface exists for exactly one consumer: `sni-gate-tui`, which
//! has to read a `sni-gate.toml` with the *same* parser the gateway uses rather
//! than a second implementation that drifts from it.
//!
//! Everything under [`tui`] is behind the `tui` feature, so a default build
//! never sees ratatui.

pub mod ca;
pub mod certscope;
pub mod config;
pub mod dns;
pub mod dns_resolvers;
pub mod ech;
pub mod error;
pub mod nat64;
pub mod peek;
pub mod pool;
pub mod probe;
pub mod proxy;
pub mod psl;
pub mod resolver;
pub mod router;
pub mod scoring;
pub mod store;
pub mod suffix;
pub mod trust;
pub mod verify;

#[cfg(feature = "tui")]
pub mod tui;
