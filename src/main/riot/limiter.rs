//! Sliding-window rate limiter for ONE routing value ("bucket").
//!
//! Riot limits each routing value (`br1`, `americas`, ...) with several windows
//! at once (e.g. 20 req/1s and 100 req/120s). Each bucket owns one limiter.
//! User-facing requests (`Priority::High`) go ahead of background ones.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    High,
    Low,
}

/// Only use 90% of the advertised limit.
const SAFETY: f64 = 0.9;
/// Slack added to computed waits so we never fire a few ms early.
const SLACK: Duration = Duration::from_millis(50);

/// Development/personal key limits: 20/1s and 100/120s.
pub const DEFAULT_LIMITS: &[(u32, u64)] = &[(20, 1), (100, 120)];

#[derive(Debug)]
struct State {
    /// (max calls, window)
    windows: Vec<(usize, Duration)>,
    calls: VecDeque<Instant>,
    blocked_until: Option<Instant>,
}

#[derive(Debug)]
pub struct Limiter {
    state: Mutex<State>,
    high_waiting: AtomicUsize,
}

fn with_safety(limits: &[(u32, u64)]) -> Vec<(usize, Duration)> {
    limits
        .iter()
        .map(|&(n, s)| (((n as f64) * SAFETY).floor().max(1.0) as usize, Duration::from_secs(s)))
        .collect()
}

/// Parses a header such as `20:1,100:120` into `[(20,1),(100,120)]`.
pub fn parse_limits_header(value: &str) -> Option<Vec<(u32, u64)>> {
    let mut out = Vec::new();
    for part in value.split(',') {
        let (n, s) = part.trim().split_once(':')?;
        out.push((n.trim().parse().ok()?, s.trim().parse().ok()?));
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

impl Limiter {
    pub fn new(limits: &[(u32, u64)]) -> Self {
        Self {
            state: Mutex::new(State { windows: with_safety(limits), calls: VecDeque::new(), blocked_until: None }),
            high_waiting: AtomicUsize::new(0),
        }
    }

    /// Updates the windows from an `X-App-Rate-Limit` header.
    pub async fn update_limits(&self, limits: &[(u32, u64)]) {
        let windows = with_safety(limits);
        let mut s = self.state.lock().await;
        if s.windows != windows {
            tracing::info!("bucket limits updated: {limits:?}");
            s.windows = windows;
        }
    }

    /// Blocks the bucket (e.g. `Retry-After` from a 429).
    pub async fn block_for(&self, d: Duration) {
        let until = Instant::now() + d;
        let mut s = self.state.lock().await;
        s.blocked_until = Some(s.blocked_until.map_or(until, |b| b.max(until)));
    }

    /// Waits until a call is allowed and records it.
    pub async fn acquire(&self, priority: Priority) {
        let high = priority == Priority::High;
        if high {
            self.high_waiting.fetch_add(1, Ordering::SeqCst);
        }
        loop {
            let wait = {
                let mut s = self.state.lock().await;
                let now = Instant::now();
                let mut wait = Duration::ZERO;

                if let Some(b) = s.blocked_until {
                    if b > now {
                        wait = b - now;
                    } else {
                        s.blocked_until = None;
                    }
                }

                let longest = s.windows.iter().map(|&(_, w)| w).max().unwrap_or_default();
                while s.calls.front().is_some_and(|&t| now - t >= longest) {
                    s.calls.pop_front();
                }
                for &(n, w) in &s.windows {
                    let in_window: Vec<Instant> =
                        s.calls.iter().rev().take_while(|&&t| now - t < w).copied().collect();
                    if in_window.len() >= n {
                        // the n-th most recent call must leave the window first
                        let oldest_relevant = in_window[n - 1];
                        wait = wait.max(w - (now - oldest_relevant) + SLACK);
                    }
                }

                // background work yields while the user is waiting
                if !high && wait.is_zero() && self.high_waiting.load(Ordering::SeqCst) > 0 {
                    wait = Duration::from_millis(100);
                } else if wait.is_zero() {
                    s.calls.push_back(now);
                }
                wait
            };
            if wait.is_zero() {
                break;
            }
            tokio::time::sleep(wait).await;
        }
        if high {
            self.high_waiting.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_header() {
        assert_eq!(parse_limits_header("20:1,100:120"), Some(vec![(20, 1), (100, 120)]));
        assert_eq!(parse_limits_header("garbage"), None);
    }

    #[tokio::test(start_paused = true)]
    async fn respects_short_window() {
        // 10/1s with 90% safety -> 9 per second
        let l = Limiter::new(&[(10, 1)]);
        let start = Instant::now();
        for _ in 0..9 {
            l.acquire(Priority::High).await;
        }
        assert!(Instant::now() - start < Duration::from_millis(10));
        l.acquire(Priority::High).await;
        assert!(Instant::now() - start >= Duration::from_secs(1));
    }

    #[tokio::test(start_paused = true)]
    async fn respects_long_window() {
        // 20/1s and 100/120s -> 18/s and 90/120s
        let l = Limiter::new(DEFAULT_LIMITS);
        let start = Instant::now();
        for _ in 0..90 {
            l.acquire(Priority::High).await;
        }
        assert!(Instant::now() - start < Duration::from_secs(10));
        l.acquire(Priority::High).await; // the 91st waits for the 2-minute window
        assert!(Instant::now() - start >= Duration::from_secs(120));
    }

    #[tokio::test(start_paused = true)]
    async fn honours_block() {
        let l = Limiter::new(DEFAULT_LIMITS);
        l.block_for(Duration::from_secs(7)).await;
        let start = Instant::now();
        l.acquire(Priority::High).await;
        assert!(Instant::now() - start >= Duration::from_secs(7));
    }

    #[tokio::test(start_paused = true)]
    async fn high_priority_goes_first() {
        use std::sync::Arc;
        let l = Arc::new(Limiter::new(&[(10, 1)]));
        for _ in 0..9 {
            l.acquire(Priority::High).await; // fill the window
        }
        let order = Arc::new(std::sync::Mutex::new(Vec::new()));
        let (l1, o1) = (l.clone(), order.clone());
        let low = tokio::spawn(async move {
            l1.acquire(Priority::Low).await;
            o1.lock().unwrap().push("low");
        });
        tokio::task::yield_now().await;
        let (l2, o2) = (l.clone(), order.clone());
        let high = tokio::spawn(async move {
            l2.acquire(Priority::High).await;
            o2.lock().unwrap().push("high");
        });
        low.await.unwrap();
        high.await.unwrap();
        assert_eq!(order.lock().unwrap()[0], "high");
    }
}
