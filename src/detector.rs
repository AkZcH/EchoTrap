// src/detector.rs
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

pub struct AttackTracker {
    history: HashMap<IpAddr, Vec<Instant>>,
    threshold: usize,
    window: Duration,
}

impl AttackTracker {
    pub fn new(threshold: usize, window_secs: u64) -> Self {
        Self {
            history: HashMap::new(),
            threshold,
            window: Duration::from_secs(window_secs),
        }
    }

    pub fn record_and_check(&mut self, addr: SocketAddr) -> bool {
        let now = Instant::now();
        let ip = addr.ip();
        let entry = self.history.entry(ip).or_default();

        entry.push(now);

        let cutoff = now - self.window;
        entry.retain(|&t| t >= cutoff);

        let cap = self.threshold.saturating_mul(4);
        if entry.len() > cap {
            entry.drain(0..(entry.len() - cap));
        }

        entry.len() >= self.threshold
    }

    // Used in C-02 (LRU eviction commit).
    #[allow(dead_code)]
    pub fn purge_all_old(&mut self) {
        let now = Instant::now();
        let cutoff = now - self.window;
        self.history.retain(|_, times| {
            times.retain(|&t| t >= cutoff);
            !times.is_empty()
        });
    }

    // Used in C-13 (metrics/dashboard commit).
    #[allow(dead_code)]
    pub fn tracked_ip_count(&self) -> usize {
        self.history.len()
    }
}