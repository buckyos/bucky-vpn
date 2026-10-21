#![allow(dead_code)]
#![allow(unexpected_cfgs)]
#![allow(hidden_glob_reexports)]
pub use vpn_frame::*;

#[path = "support/idle_client.rs"]
mod client;
