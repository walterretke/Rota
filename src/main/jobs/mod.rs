//! Background jobs. Every job must pause while the user is in a match (`InProgress`)
//! so the app never competes with the game for CPU or network.

pub mod import;

use crate::lcu::LcuState;
use std::time::Duration;
use tokio::sync::watch;

/// Waits while the user is in a match. Returns `true` if it had to wait.
pub async fn wait_while_in_game(lcu: &mut watch::Receiver<LcuState>) -> bool {
    let mut waited = false;
    while lcu.borrow_and_update().in_game() {
        waited = true;
        let _ = tokio::time::timeout(Duration::from_secs(30), lcu.changed()).await;
    }
    waited
}
