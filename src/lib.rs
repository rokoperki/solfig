//! SOLFIG library surface.
//!
//! The modules are exposed so the `solfig` binary (`main.rs`) and the
//! integration tests under `tests/` share one compiled crate.

pub mod app;
pub mod config;
pub mod endpoints;
pub mod faucets;
pub mod profiles;
pub mod rpc;
pub mod theme;
pub mod ui;
