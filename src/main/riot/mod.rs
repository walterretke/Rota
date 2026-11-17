//! Riot API access. A single `RiotClient` instance owns the rate limit for the whole app:
//! every Riot call must go through it.

pub mod client;
pub mod limiter;

pub use client::{KeyState, RiotClient, RiotError, Routing};
pub use limiter::Priority;
