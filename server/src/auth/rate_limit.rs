use dashmap::DashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
struct Entry {
    failures: u32,
    /// In-flight reservations from try_acquire that have not yet resolved
    /// to success or failure. Counts toward the threshold so a burst of
    /// concurrent requests cannot all pass check() before any of them
    /// record_failure(), closing the window between check() and
    /// record_failure().
    in_flight: u32,
    first_failure: Instant,
    locked_until: Option<Instant>,
}

#[derive(Clone)]
pub struct LoginRateLimiter {
    inner: Arc<DashMap<IpAddr, Entry>>,
    config: Arc<RwLock<Config>>,
}

#[derive(Clone)]
pub struct McpAuthRateLimiter {
    inner: Arc<DashMap<String, Entry>>,
    config: Arc<RwLock<Config>>,
}

pub type AuthFailureRateLimiter = McpAuthRateLimiter;

#[derive(Debug, Clone)]
struct McpWriteEntry {
    count: u32,
    window_start: Instant,
}

#[derive(Clone)]
pub struct McpWriteRateLimiter {
    inner: Arc<DashMap<(String, String), McpWriteEntry>>,
    config: Arc<RwLock<McpWriteConfig>>,
}

#[derive(Debug, Clone, Copy)]
struct Config {
    threshold: u32,
    window: Duration,
    lock_duration: Duration,
}

#[derive(Debug, Clone, Copy)]
struct McpWriteConfig {
    limit: u32,
    window: Duration,
}

impl LoginRateLimiter {
    pub fn new(threshold: u32, window: Duration, lock_duration: Duration) -> Self {
        Self {
            inner: Arc::new(DashMap::new()),
            config: Arc::new(RwLock::new(Config {
                threshold: threshold.max(1),
                window,
                lock_duration,
            })),
        }
    }

    pub fn update_config(&self, threshold: u32, window: Duration, lock_duration: Duration) {
        *self.config.write().expect("login limiter lock poisoned") = Config {
            threshold: threshold.max(1),
            window,
            lock_duration,
        };
    }

    pub fn check(&self, ip: IpAddr) -> Result<(), Duration> {
        let now = Instant::now();
        let config = *self.config.read().expect("login limiter lock poisoned");
        let mut stale = false;
        if let Some(e) = self.inner.get(&ip) {
            if entry_is_stale(&e, now, config) {
                stale = true;
            } else if let Some(until) = e.locked_until {
                return Err(until - now);
            }
        }
        if stale {
            self.inner.remove(&ip);
        }
        Ok(())
    }

    pub fn record_failure(&self, ip: IpAddr) {
        let now = Instant::now();
        let config = *self.config.read().expect("login limiter lock poisoned");
        let mut e = self.inner.entry(ip).or_insert(Entry {
            failures: 0,
            in_flight: 0,
            first_failure: now,
            locked_until: None,
        });
        if now.duration_since(e.first_failure) > config.window {
            e.failures = 0;
            e.first_failure = now;
            e.locked_until = None;
        }
        e.failures += 1;
        if e.failures >= config.threshold {
            e.locked_until = Some(now + config.lock_duration);
        }
    }

    pub fn record_success(&self, ip: IpAddr) {
        self.inner.remove(&ip);
    }

    /// Atomic reservation: in a single dashmap entry lock, check that the IP
    /// is not currently locked and that allowing one more in-flight attempt
    /// won't cross the threshold; if both conditions hold, increment
    /// `in_flight` and return a reservation handle. Otherwise return the
    /// lock duration (caller maps to 429).
    ///
    /// Counting `in_flight` toward threshold closes the concurrent-attempt window:
    /// even if N concurrent requests pass the lock check, after N=`threshold`
    /// reservations are in flight the (N+1)th request is rejected before
    /// argon2 runs, instead of waiting for one of them to call
    /// `record_failure` (which historically left a wide CPU-burn window).
    pub fn try_acquire(&self, ip: IpAddr) -> Result<AttemptReservation, Duration> {
        let now = Instant::now();
        let config = *self.config.read().expect("login limiter lock poisoned");
        let mut entry = self.inner.entry(ip).or_insert(Entry {
            failures: 0,
            in_flight: 0,
            first_failure: now,
            locked_until: None,
        });

        // Honour expired lock state first so a returning attacker after the
        // lockout window gets a fresh budget.
        if entry_is_stale(&entry, now, config) {
            entry.failures = 0;
            entry.in_flight = 0;
            entry.first_failure = now;
            entry.locked_until = None;
        }

        if let Some(until) = entry.locked_until {
            if until > now {
                return Err(until - now);
            }
        }

        // Failures + in-flight reservations together must stay strictly below
        // the threshold. The (N=threshold)th request trips a fresh lock.
        if entry.failures + entry.in_flight >= config.threshold {
            entry.locked_until = Some(now + config.lock_duration);
            return Err(config.lock_duration);
        }

        entry.in_flight += 1;
        Ok(AttemptReservation {
            limiter: self.clone(),
            ip,
            resolved: false,
        })
    }

    /// Internal: called by AttemptReservation::success to release the slot.
    fn release_success(&self, ip: IpAddr) {
        // Successful auth resets the entry entirely. record_success already
        // does this; we go through the same path to keep semantics centralised.
        self.inner.remove(&ip);
    }

    /// Internal: called by AttemptReservation::failure to release the slot
    /// and atomically charge a failure.
    fn release_failure(&self, ip: IpAddr) {
        let now = Instant::now();
        let config = *self.config.read().expect("login limiter lock poisoned");
        let mut entry = self.inner.entry(ip).or_insert(Entry {
            failures: 0,
            in_flight: 0,
            first_failure: now,
            locked_until: None,
        });
        if now.duration_since(entry.first_failure) > config.window {
            entry.failures = 0;
            entry.first_failure = now;
            entry.locked_until = None;
        }
        if entry.in_flight > 0 {
            entry.in_flight -= 1;
        }
        entry.failures += 1;
        if entry.failures >= config.threshold {
            entry.locked_until = Some(now + config.lock_duration);
        }
    }

    /// Internal: called by AttemptReservation::release for non-attributable
    /// outcomes (e.g. internal 500). Releases the slot without changing the
    /// failure counter.
    fn release_neutral(&self, ip: IpAddr) {
        let mut entry = match self.inner.get_mut(&ip) {
            Some(e) => e,
            None => return,
        };
        if entry.in_flight > 0 {
            entry.in_flight -= 1;
        }
    }

    pub fn prune_stale(&self) -> usize {
        let now = Instant::now();
        let config = *self.config.read().expect("login limiter lock poisoned");
        let before = self.inner.len();
        self.inner
            .retain(|_, entry| !entry_is_stale(entry, now, config));
        before.saturating_sub(self.inner.len())
    }
}

impl McpAuthRateLimiter {
    pub fn new(threshold: u32, window: Duration, lock_duration: Duration) -> Self {
        Self {
            inner: Arc::new(DashMap::new()),
            config: Arc::new(RwLock::new(Config {
                threshold: threshold.max(1),
                window,
                lock_duration,
            })),
        }
    }

    pub fn update_config(&self, threshold: u32, window: Duration, lock_duration: Duration) {
        *self.config.write().expect("mcp auth limiter lock poisoned") = Config {
            threshold: threshold.max(1),
            window,
            lock_duration,
        };
    }

    pub fn check(&self, key: &str) -> Result<(), Duration> {
        let now = Instant::now();
        let config = *self.config.read().expect("mcp auth limiter lock poisoned");
        let mut stale = false;
        if let Some(entry) = self.inner.get(key) {
            if entry_is_stale(&entry, now, config) {
                stale = true;
            } else if let Some(until) = entry.locked_until {
                return Err(until - now);
            }
        }
        if stale {
            self.inner.remove(key);
        }
        Ok(())
    }

    pub fn try_acquire(&self, key: &str) -> Result<McpAuthAttemptReservation, Duration> {
        let now = Instant::now();
        let config = *self.config.read().expect("mcp auth limiter lock poisoned");
        let key = key.to_string();
        let mut entry = self.inner.entry(key.clone()).or_insert(Entry {
            failures: 0,
            in_flight: 0,
            first_failure: now,
            locked_until: None,
        });

        if entry_is_stale(&entry, now, config) {
            entry.failures = 0;
            entry.in_flight = 0;
            entry.first_failure = now;
            entry.locked_until = None;
        }

        if let Some(until) = entry.locked_until {
            if until > now {
                return Err(until - now);
            }
        }

        if entry.failures + entry.in_flight >= config.threshold {
            entry.locked_until = Some(now + config.lock_duration);
            return Err(config.lock_duration);
        }

        entry.in_flight += 1;
        Ok(McpAuthAttemptReservation {
            limiter: self.clone(),
            key,
            resolved: false,
        })
    }

    pub fn record_failure(&self, key: &str) {
        let now = Instant::now();
        let config = *self.config.read().expect("mcp auth limiter lock poisoned");
        let mut entry = self.inner.entry(key.to_string()).or_insert(Entry {
            failures: 0,
            in_flight: 0,
            first_failure: now,
            locked_until: None,
        });
        if now.duration_since(entry.first_failure) > config.window {
            entry.failures = 0;
            entry.in_flight = 0;
            entry.first_failure = now;
            entry.locked_until = None;
        }
        entry.failures += 1;
        if entry.failures >= config.threshold {
            entry.locked_until = Some(now + config.lock_duration);
        }
    }

    pub fn record_success(&self, key: &str) {
        self.inner.remove(key);
    }

    fn release_success(&self, key: &str) {
        self.inner.remove(key);
    }

    fn release_failure(&self, key: &str) {
        let now = Instant::now();
        let config = *self.config.read().expect("mcp auth limiter lock poisoned");
        let mut entry = self.inner.entry(key.to_string()).or_insert(Entry {
            failures: 0,
            in_flight: 0,
            first_failure: now,
            locked_until: None,
        });
        if now.duration_since(entry.first_failure) > config.window {
            entry.failures = 0;
            entry.first_failure = now;
            entry.locked_until = None;
        }
        if entry.in_flight > 0 {
            entry.in_flight -= 1;
        }
        entry.failures += 1;
        if entry.failures >= config.threshold {
            entry.locked_until = Some(now + config.lock_duration);
        }
    }

    fn release_neutral(&self, key: &str) {
        let mut entry = match self.inner.get_mut(key) {
            Some(entry) => entry,
            None => return,
        };
        if entry.in_flight > 0 {
            entry.in_flight -= 1;
        }
    }

    pub fn prune_stale(&self) -> usize {
        let now = Instant::now();
        let config = *self.config.read().expect("mcp auth limiter lock poisoned");
        let before = self.inner.len();
        self.inner
            .retain(|_, entry| !entry_is_stale(entry, now, config));
        before.saturating_sub(self.inner.len())
    }
}

impl McpWriteRateLimiter {
    pub fn new(limit: u32, window: Duration) -> Self {
        Self {
            inner: Arc::new(DashMap::new()),
            config: Arc::new(RwLock::new(McpWriteConfig {
                limit: limit.max(1),
                window,
            })),
        }
    }

    pub fn update_config(&self, limit: u32, window: Duration) {
        *self
            .config
            .write()
            .expect("mcp write limiter lock poisoned") = McpWriteConfig {
            limit: limit.max(1),
            window,
        };
    }

    pub fn try_record(&self, token_id: &str, vault_id: &str) -> Result<(), Duration> {
        let now = Instant::now();
        let config = *self.config.read().expect("mcp write limiter lock poisoned");
        let key = (token_id.to_string(), vault_id.to_string());
        let mut entry = self.inner.entry(key).or_insert(McpWriteEntry {
            count: 0,
            window_start: now,
        });
        let elapsed = now.duration_since(entry.window_start);
        if elapsed >= config.window {
            entry.count = 0;
            entry.window_start = now;
        }
        if entry.count >= config.limit {
            return Err(config.window.saturating_sub(elapsed));
        }
        entry.count += 1;
        Ok(())
    }

    pub fn prune_stale(&self) -> usize {
        let now = Instant::now();
        let config = *self.config.read().expect("mcp write limiter lock poisoned");
        let before = self.inner.len();
        self.inner
            .retain(|_, entry| now.duration_since(entry.window_start) < config.window);
        before.saturating_sub(self.inner.len())
    }
}

fn entry_is_stale(entry: &Entry, now: Instant, config: Config) -> bool {
    if let Some(until) = entry.locked_until {
        return until <= now;
    }
    now.duration_since(entry.first_failure) > config.window
}

/// Reservation handle returned by `try_acquire`. Holding this object means a
/// slot has been reserved against the limiter; the holder MUST resolve it
/// with `success()`, `failure()`, or `release()` before drop. If dropped
/// without explicit resolution (e.g. due to a panic), the Drop impl treats
/// the attempt as a failure — pessimistic by design so a panicking handler
/// cannot silently leak a free attempt.
pub struct AttemptReservation {
    limiter: LoginRateLimiter,
    ip: IpAddr,
    resolved: bool,
}

impl AttemptReservation {
    /// Resolve as a successful authentication. Resets the IP's failure
    /// counter (the entry is removed entirely).
    pub fn success(mut self) {
        self.resolved = true;
        self.limiter.release_success(self.ip);
    }

    /// Resolve as a failed authentication. Atomically decrements in-flight,
    /// increments failures, and may set the lockout.
    pub fn failure(mut self) {
        self.resolved = true;
        self.limiter.release_failure(self.ip);
    }

    /// Resolve as a non-attributable outcome (e.g. internal server error,
    /// validation error pre-auth). Releases the slot without changing the
    /// failure counter. Use sparingly — most non-success outcomes should be
    /// treated as failures.
    pub fn release(mut self) {
        self.resolved = true;
        self.limiter.release_neutral(self.ip);
    }
}

impl Drop for AttemptReservation {
    fn drop(&mut self) {
        if !self.resolved {
            self.limiter.release_failure(self.ip);
        }
    }
}

pub struct McpAuthAttemptReservation {
    limiter: McpAuthRateLimiter,
    key: String,
    resolved: bool,
}

impl McpAuthAttemptReservation {
    pub fn success(mut self) {
        self.resolved = true;
        self.limiter.release_success(&self.key);
    }

    pub fn failure(mut self) {
        self.resolved = true;
        self.limiter.release_failure(&self.key);
    }

    pub fn release(mut self) {
        self.resolved = true;
        self.limiter.release_neutral(&self.key);
    }
}

impl Drop for McpAuthAttemptReservation {
    fn drop(&mut self) {
        if !self.resolved {
            self.limiter.release_failure(&self.key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};
    use std::time::Duration;

    fn ip() -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4))
    }

    #[test]
    fn allows_below_threshold() {
        let l = LoginRateLimiter::new(3, Duration::from_secs(60), Duration::from_secs(60));
        l.record_failure(ip());
        l.record_failure(ip());
        assert!(l.check(ip()).is_ok());
    }

    #[test]
    fn locks_at_threshold() {
        let l = LoginRateLimiter::new(3, Duration::from_secs(60), Duration::from_secs(60));
        l.record_failure(ip());
        l.record_failure(ip());
        l.record_failure(ip());
        assert!(l.check(ip()).is_err());
    }

    #[test]
    fn success_resets() {
        let l = LoginRateLimiter::new(3, Duration::from_secs(60), Duration::from_secs(60));
        l.record_failure(ip());
        l.record_failure(ip());
        l.record_success(ip());
        l.record_failure(ip());
        l.record_failure(ip());
        assert!(l.check(ip()).is_ok());
    }

    #[test]
    fn lock_expires() {
        let l = LoginRateLimiter::new(2, Duration::from_secs(60), Duration::from_millis(50));
        l.record_failure(ip());
        l.record_failure(ip());
        assert!(l.check(ip()).is_err());
        std::thread::sleep(Duration::from_millis(80));
        assert!(l.check(ip()).is_ok());
    }

    #[test]
    fn check_prunes_expired_lock_entry() {
        let l = LoginRateLimiter::new(2, Duration::from_secs(60), Duration::from_millis(50));
        l.record_failure(ip());
        l.record_failure(ip());
        std::thread::sleep(Duration::from_millis(80));

        assert!(l.check(ip()).is_ok());
        assert_eq!(l.inner.len(), 0);
    }

    #[test]
    fn updated_threshold_applies_to_future_failures() {
        let l = LoginRateLimiter::new(3, Duration::from_secs(60), Duration::from_secs(60));
        l.update_config(1, Duration::from_secs(60), Duration::from_secs(60));
        l.record_failure(ip());
        assert!(l.check(ip()).is_err());
    }

    /// Regression: in-flight reservations count toward
    /// the threshold so concurrent attempts cannot all sneak through before
    /// any of them records a failure. With threshold=3, after 3 in-flight
    /// try_acquire calls, the 4th must be rejected even though no failures
    /// have been recorded yet.
    #[test]
    fn try_acquire_blocks_concurrent_burst_before_failures_record() {
        let l = LoginRateLimiter::new(3, Duration::from_secs(60), Duration::from_secs(60));
        // Acquire 3 slots; do NOT resolve them. They represent in-flight
        // argon2 verifications.
        let r1 = l.try_acquire(ip()).expect("first reservation");
        let r2 = l.try_acquire(ip()).expect("second reservation");
        let r3 = l.try_acquire(ip()).expect("third reservation");
        // 4th must be rejected — without H-1 fix, it would proceed because
        // failures is still 0.
        assert!(l.try_acquire(ip()).is_err());
        // Drop holds without resolution → Drop impl charges them as failures.
        drop(r1);
        drop(r2);
        drop(r3);
        // After all three resolve to failure, the lock is still active.
        assert!(l.check(ip()).is_err());
    }

    /// Reservation success resets the IP entirely so a legitimate login does
    /// not leave residual failure counter behind.
    #[test]
    fn reservation_success_resets_state() {
        let l = LoginRateLimiter::new(3, Duration::from_secs(60), Duration::from_secs(60));
        l.record_failure(ip());
        l.record_failure(ip());
        let r = l.try_acquire(ip()).unwrap();
        r.success();
        // Fresh: full budget available again.
        assert!(l.try_acquire(ip()).is_ok());
    }

    /// Reservation::release decrements in_flight without changing failures,
    /// for non-attributable outcomes like internal errors.
    #[test]
    fn reservation_release_does_not_charge_failure() {
        let l = LoginRateLimiter::new(2, Duration::from_secs(60), Duration::from_secs(60));
        let r = l.try_acquire(ip()).unwrap();
        r.release();
        // No failure recorded → still have full budget.
        let r2 = l.try_acquire(ip()).unwrap();
        let r3 = l.try_acquire(ip()).unwrap();
        // Two slots taken, threshold=2 → third rejected.
        assert!(l.try_acquire(ip()).is_err());
        r2.release();
        r3.release();
    }

    #[test]
    fn mcp_try_acquire_blocks_concurrent_burst_before_failures_record() {
        let l = McpAuthRateLimiter::new(3, Duration::from_secs(60), Duration::from_secs(60));
        let r1 = l.try_acquire("api-auth").expect("first reservation");
        let r2 = l.try_acquire("api-auth").expect("second reservation");
        let r3 = l.try_acquire("api-auth").expect("third reservation");

        assert!(l.try_acquire("api-auth").is_err());
        assert!(l.try_acquire("other-auth").is_ok());

        drop(r1);
        drop(r2);
        drop(r3);
        assert!(l.check("api-auth").is_err());
    }

    #[test]
    fn prune_stale_uses_retain_without_key_collection() {
        let source = include_str!("rate_limit.rs");
        let impl_start = source
            .find("impl LoginRateLimiter")
            .expect("login limiter impl exists");
        let test_start = source.find("#[cfg(test)]").expect("test module exists");
        let implementation = &source[impl_start..test_start];

        assert!(implementation.contains(".retain("));
        assert!(!implementation.contains(".collect::<Vec"));
    }
}
