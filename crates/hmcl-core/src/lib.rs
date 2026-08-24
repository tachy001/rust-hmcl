//! Core logic of the HMCL launcher rewritten in Rust.
//!
//! This crate contains no GUI dependencies and mirrors the structure of
//! HMCL's `HMCLCore` Java module: auth, game, launch, download, modpack,
//! java, task, event and util.

pub mod auth;
pub mod download;
pub mod event;
pub mod game;
pub mod task;
pub mod util;
