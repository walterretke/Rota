//! LCU module: talks to the League client's local API (unofficial, may change any patch).
//!
//! Rules (guide section 1.3.1): in phase 0 we only READ the gameflow phase and the
//! logged-in user. No auto-accept, auto-pick, auto-ban, and no ally identities.
//! Any failure here just disconnects and retries later; it never crashes the app.

pub mod discovery;
pub mod watcher;

pub use discovery::Credentials;
pub use watcher::{LcuState, LcuUser, Watcher};
