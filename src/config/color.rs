//! Re-export of the canonical [`Color`] type from `deadsync-core`.
//!
//! Config options that carry colors (such as the gameplay background color) use
//! the project-wide `Color`. The ARGB hex string forms read from and written to
//! disk are handled by [`Color::from_argb_hex`] and [`Color::to_argb_hex`].

pub use deadsync_core::Color;
