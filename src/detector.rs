// src/detector.rs
use lru::LruCache;
use std::net::{IpAddr, SocketAddr};
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};

/// Maximum number of distinct IPs tracked simultaneously.
/// When full, the least-recently-seen IP is evicted.
/// At ~72 bytes per entry (IpAddr + Vec<Instant> with a few timestamps),
/// 10k entries costs ~720KB worst-case — acceptable on any target hardware.
const MAX_TRACKED_IPS: usize = 10_000;

pub struct AttackTracker {
    history: LruCache<IpAddr, Vec<Instant>>,
    threshold: usize,
    window: Duration,
}

impl AttackTracker {
    pub fn new(threshold: usize, window_secs: u64) -> Self {
        Self {
            history: LruCache::new(
                NonZeroUsize::new(MAX_TRACKED_IPS).expect("MAX_TRACKED_IPS must be non-zero"),
            ),
            threshold,
            window: Duration::from_secs(window_secs),
        }
    }

    /// Record a connection from `addr`. Returns true if this IP is now
    /// suspicious (>= threshold hits within the sliding window).
    ///
    /// On every call for a given IP:
    ///   1. Append the current timestamp.
    ///   2. Drain timestamps older than the window (sliding window pruning).
    ///   3. Cap the Vec at threshold*4 to bound per-IP memory.
    ///
    /// LruCache handles the cross-IP memory bound: when MAX_TRACKED_IPS is
    /// reached, the least-recently-used IP is evicted automatically.
    pub fn record_and_check(&mut self, addr: SocketAddr) -> bool {
        let now = Instant::now();
        let ip = addr.ip();

        // get_or_insert via get_mut + push, using LruCache's built-in LRU
        // promotion on access.
        let entry = self.history.get_or_insert_mut(ip, Vec::new);

        entry.push(now);

        // Prune timestamps outside the sliding window.
        let cutoff = now - self.window;
        entry.retain(|&t| t >= cutoff);

        // Cap per-IP vec to prevent a single IP from using unbounded memory
        // if retain somehow leaves too many entries (e.g. very high threshold).
        let cap = self.threshold.saturating_mul(4);
        if entry.len() > cap {
            entry.drain(0..(entry.len() - cap));
        }

        entry.len() >= self.threshold
    }

    /// Evict all IPs whose most-recent connection is older than the window.
    /// Call this periodically (e.g. every 60s) to shrink the cache after
    /// a scan burst subsides. Not required for correctness — LRU handles
    /// the hard cap — but keeps memory lean during quiet periods.
    #[allow(dead_code)]
    pub fn purge_all_old(&mut self) {
        let window = self.window;
        let now = Instant::now();
        let cutoff = now - window;
        // LruCache doesn't support retain directly; collect keys to drop first.
        let stale: Vec<IpAddr> = self
            .history
            .iter()
            .filter(|(_, times)| times.iter().all(|&t| t < cutoff))
            .map(|(&ip, _)| ip)
            .collect();
        for ip in stale {
            self.history.pop(&ip);
        }
    }

    /// Number of IPs currently in the tracker. Used by C-13 metrics endpoint.
    #[allow(dead_code)]
    pub fn tracked_ip_count(&self) -> usize {
        self.history.len()
    }
}
