// src/detector.rs
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

/// AttackTracker: simple sliding-window tracker per IpAddr.
/// NOTE: This is intentionally simple. For production/high-load, switch
/// to a fixed-size ring buffer or probabilistic sketch to save memory.
pub struct AttackTracker {
    /// Map from IP -> timestamps (Instant) of recent connection attempts.
    history: HashMap<IpAddr, Vec<Instant>>,
    threshold: usize,
    window: Duration,
}

impl AttackTracker {
    /// Create a new tracker with a threshold and a sliding window in seconds.
    pub fn new(threshold: usize, window_secs: u64) -> Self {
        Self {
            history: HashMap::new(),
            threshold,
            window: Duration::from_secs(window_secs),
        }
    }

    /// Record a connection from `addr`. Returns true if this addr is now suspicious
    /// (i.e., within the sliding window there are >= threshold events).
    ///
    /// This method *prunes* stale timestamps on every call for that IP.
    pub fn record_and_check(&mut self, addr: SocketAddr) -> bool {
        let now = Instant::now();
        let ip = addr.ip();
        let entry = self.history.entry(ip).or_default();

        // Append current timestamp
        entry.push(now);

        // Remove timestamps older than window
        let cutoff = now - self.window;
        entry.retain(|&t| t >= cutoff);

        // If a client is repeatedly connecting, the Vec could grow. Cap to (threshold * 4)
        // to limit memory in pathological cases.
        let cap = self.threshold.saturating_mul(4);
        if entry.len() > cap {
            entry.drain(0..(entry.len() - cap));
        }

        // Now evaluate suspiciousness
        entry.len() >= self.threshold
    }

    /// Optional: remove old entries for all IPs (less aggressive housekeeping).
    /// Call periodically if you want to shrink the map size over time.
    pub fn purge_all_old(&mut self) {
        let now = Instant::now();
        let cutoff = now - self.window;
        self.history.retain(|_, times| {
            times.retain(|&t| t >= cutoff);
            !times.is_empty()
        });
    }

    /// Get number of tracked IP entries (for metrics/debug).
    pub fn tracked_ip_count(&self) -> usize {
        self.history.len()
    }
}