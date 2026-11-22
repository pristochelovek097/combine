//! Shared building blocks for COMBINE's package tooling.
//!
//! Deliberately dependency-free: everything here must build with a bare
//! `cargo build` on a fresh distro image, with no crates.io access.

pub mod fsutil;
pub mod md5;
pub mod packages;

/// Ergonomic error type: these tools are small, and a printable message is
/// enough for every failure path.
pub type Result<T> = std::result::Result<T, String>;
