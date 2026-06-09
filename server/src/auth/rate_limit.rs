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
    /// concurrent requests cannot all reserve work before any failure is
    /// resolved.
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

    /// Atomic reservation: in a single dashmap entry lock, check that the IP
    /// is not currently locked and that allowing one more in-flight attempt
    /// won't cross the threshold; if both conditions hold, increment
    /// `in_flight` and return a reservation handle. Otherwise return the
    /// lock duration (caller maps to 429).
    ///
    /// Counting `in_flight` toward threshold closes the concurrent-attempt window:
    /// even if N concurrent requests pass the lock check, after N=`threshold`
    /// reservations are in flight the (N+1)th request is rejected before
    /// argon2 runs, instead of waiting for one of them to resolve as a
    /// failure (which historically left a wide CPU-burn window).
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
    /// outcomes (e.g. internal 500, cancellation, or unresolved Drop). Releases
    /// the slot without changing the failure counter. If the entry has no
    /// recorded failures and no more in-flight slots remain, any transient burst
    /// lock set by try_acquire is also cleared, since there is nothing real to
    /// lock for.
    fn release_neutral(&self, ip: IpAddr) {
        let mut entry = match self.inner.get_mut(&ip) {
            Some(e) => e,
            None => return,
        };
        if entry.in_flight > 0 {
            entry.in_flight -= 1;
        }
        // If there are no real failures and no more in-flight attempts, remove
        // any transient burst-protection lock that was set by try_acquire. That
        // lock was defensive (in-flight counting), not a failure-based lockout.
        if entry.failures == 0 && entry.in_flight == 0 {
            entry.locked_until = None;
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

    pub fn try_acquire(&self, key: &str) -> Result<McpAuthAttemptReservation, Duration> {
        let now = Instant::now();
        let config = *self.config.read().expect("mcp auth limiter lock poisoned");
        let mut entry = self.inner.entry(key.to_string()).or_insert(Entry {
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
        let key = entry.key().clone();
        Ok(McpAuthAttemptReservation {
            limiter: self.clone(),
            key,
            resolved: false,
        })
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
        // If there are no real failures and no more in-flight attempts, remove
        // any transient burst-protection lock set by try_acquire.
        if entry.failures == 0 && entry.in_flight == 0 {
            entry.locked_until = None;
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
/// slot has been reserved against the limiter; the caller SHOULD resolve it
/// explicitly with `success()`, `failure()`, or `release()`. If dropped
/// without explicit resolution (e.g. due to cancellation, connection abort, or
/// a panic), the Drop impl performs a *neutral release*: the in-flight counter
/// is decremented without charging a failure. This means cancelled or aborted
/// requests do not consume the authentication failure budget.
///
/// Callers that know a real authentication failure occurred MUST call
/// `.failure()` explicitly; never rely on Drop for failure attribution.
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
    /// Unresolved drop (cancellation / abort / panic) is treated as a neutral
    /// release: the in-flight slot is freed without incrementing the failure
    /// counter. Real authentication failures MUST be attributed via `.failure()`.
    fn drop(&mut self) {
        if !self.resolved {
            self.limiter.release_neutral(self.ip);
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
    /// Unresolved drop (cancellation / abort / panic) is treated as a neutral
    /// release: the in-flight slot is freed without incrementing the failure
    /// counter. Real authentication failures MUST be attributed via `.failure()`.
    fn drop(&mut self) {
        if !self.resolved {
            self.limiter.release_neutral(&self.key);
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

    fn charge_login_failure(limiter: &LoginRateLimiter) {
        limiter
            .try_acquire(ip())
            .expect("login reservation")
            .failure();
    }

    fn assert_login_open(limiter: &LoginRateLimiter) {
        limiter
            .try_acquire(ip())
            .expect("login limiter should allow reservation")
            .release();
    }

    fn assert_login_locked(limiter: &LoginRateLimiter) {
        assert!(
            limiter.try_acquire(ip()).is_err(),
            "login limiter should reject reservation"
        );
    }

    fn charge_mcp_failure(limiter: &McpAuthRateLimiter, key: &str) {
        limiter
            .try_acquire(key)
            .expect("mcp auth reservation")
            .failure();
    }

    fn assert_mcp_open(limiter: &McpAuthRateLimiter, key: &str) {
        limiter
            .try_acquire(key)
            .expect("mcp auth limiter should allow reservation")
            .release();
    }

    fn assert_mcp_locked(limiter: &McpAuthRateLimiter, key: &str) {
        assert!(
            limiter.try_acquire(key).is_err(),
            "mcp auth limiter should reject reservation"
        );
    }

    #[test]
    fn allows_below_threshold() {
        let l = LoginRateLimiter::new(3, Duration::from_secs(60), Duration::from_secs(60));
        charge_login_failure(&l);
        charge_login_failure(&l);
        assert_login_open(&l);
    }

    #[test]
    fn locks_at_threshold() {
        let l = LoginRateLimiter::new(3, Duration::from_secs(60), Duration::from_secs(60));
        charge_login_failure(&l);
        charge_login_failure(&l);
        charge_login_failure(&l);
        assert_login_locked(&l);
    }

    #[test]
    fn success_resets() {
        let l = LoginRateLimiter::new(3, Duration::from_secs(60), Duration::from_secs(60));
        charge_login_failure(&l);
        charge_login_failure(&l);
        l.try_acquire(ip()).expect("success reservation").success();
        charge_login_failure(&l);
        charge_login_failure(&l);
        assert_login_open(&l);
    }

    #[test]
    fn lock_expires() {
        let l = LoginRateLimiter::new(2, Duration::from_secs(60), Duration::from_millis(50));
        charge_login_failure(&l);
        charge_login_failure(&l);
        assert_login_locked(&l);
        std::thread::sleep(Duration::from_millis(80));
        assert_login_open(&l);
    }

    #[test]
    fn prune_stale_removes_expired_lock_entry() {
        let l = LoginRateLimiter::new(2, Duration::from_secs(60), Duration::from_millis(50));
        charge_login_failure(&l);
        charge_login_failure(&l);
        std::thread::sleep(Duration::from_millis(80));

        assert_eq!(l.prune_stale(), 1);
        assert_eq!(l.inner.len(), 0);
    }

    #[test]
    fn updated_threshold_applies_to_future_failures() {
        let l = LoginRateLimiter::new(3, Duration::from_secs(60), Duration::from_secs(60));
        l.update_config(1, Duration::from_secs(60), Duration::from_secs(60));
        charge_login_failure(&l);
        assert_login_locked(&l);
    }

    /// Regression: in-flight reservations count toward
    /// the threshold so concurrent attempts cannot all sneak through before
    /// any of them records a failure. With threshold=3, after 3 in-flight
    /// try_acquire calls, the 4th must be rejected even though no failures
    /// have been recorded yet. After the burst is dropped (neutral), the limiter
    /// is unlocked because no failures were charged — only the concurrent slot
    /// occupancy blocked the 4th attempt while in flight.
    #[test]
    fn try_acquire_blocks_concurrent_burst_before_failures_record() {
        let l = LoginRateLimiter::new(3, Duration::from_secs(60), Duration::from_secs(60));
        // Acquire 3 slots; do NOT resolve them. They represent in-flight
        // argon2 verifications.
        let r1 = l.try_acquire(ip()).expect("first reservation");
        let r2 = l.try_acquire(ip()).expect("second reservation");
        let r3 = l.try_acquire(ip()).expect("third reservation");
        // 4th must be rejected while the 3 slots are in flight — without the
        // in-flight fix, it would proceed because failures is still 0.
        assert!(l.try_acquire(ip()).is_err());
        // Drop without resolution → neutral release (no failure charged).
        drop(r1);
        drop(r2);
        drop(r3);
        // All in-flight slots released neutrally: no failures recorded,
        // limiter should now be open again.
        assert_login_open(&l);
    }

    /// Dropping a reservation without resolving it must not increment the
    /// failure counter. A single unresolved drop (threshold=1) must leave the
    /// limiter open so the next try_acquire succeeds.
    #[test]
    fn drop_without_resolve_is_neutral_login() {
        let l = LoginRateLimiter::new(1, Duration::from_secs(60), Duration::from_secs(60));
        let r = l.try_acquire(ip()).expect("first reservation");
        drop(r); // neutral: no failure charged
                 // If Drop had charged a failure, threshold=1 would now be locked and
                 // the next try_acquire would return Err.
        assert!(
            l.try_acquire(ip()).is_ok(),
            "unresolved drop must not consume failure budget"
        );
    }

    /// Explicit .failure() must still increment the counter and lock at
    /// threshold — the neutral-drop change must not weaken the explicit path.
    #[test]
    fn explicit_failure_still_locks_login() {
        let l = LoginRateLimiter::new(1, Duration::from_secs(60), Duration::from_secs(60));
        let r = l.try_acquire(ip()).expect("reservation");
        r.failure(); // explicit failure
        assert_login_locked(&l);
    }

    /// Reservation success resets the IP entirely so a legitimate login does
    /// not leave residual failure counter behind.
    #[test]
    fn reservation_success_resets_state() {
        let l = LoginRateLimiter::new(3, Duration::from_secs(60), Duration::from_secs(60));
        charge_login_failure(&l);
        charge_login_failure(&l);
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

        // 4th for the same key must be rejected while 3 slots are in flight.
        assert!(l.try_acquire("api-auth").is_err());
        // Different key is unaffected.
        assert!(l.try_acquire("other-auth").is_ok());

        // Drop without resolution → neutral release (no failure charged).
        drop(r1);
        drop(r2);
        drop(r3);
        // All slots released neutrally: limiter must now be open.
        assert_mcp_open(&l, "api-auth");
    }

    /// Symmetric to drop_without_resolve_is_neutral_login for the MCP limiter.
    #[test]
    fn drop_without_resolve_is_neutral_mcp() {
        let l = McpAuthRateLimiter::new(1, Duration::from_secs(60), Duration::from_secs(60));
        let r = l.try_acquire("mcp-key").expect("first reservation");
        drop(r); // neutral: no failure charged
        assert!(
            l.try_acquire("mcp-key").is_ok(),
            "unresolved drop must not consume mcp failure budget"
        );
    }

    /// Explicit .failure() on MCP reservation must still increment the counter
    /// and lock at threshold.
    #[test]
    fn explicit_failure_still_locks_mcp() {
        let l = McpAuthRateLimiter::new(1, Duration::from_secs(60), Duration::from_secs(60));
        charge_mcp_failure(&l, "mcp-key");
        assert_mcp_locked(&l, "mcp-key");
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

    #[test]
    fn mcp_try_acquire_does_not_clone_key_before_entry_lookup() {
        let source = include_str!("rate_limit.rs");
        let fn_start = source
            .find("pub fn try_acquire(&self, key: &str) -> Result<McpAuthAttemptReservation")
            .expect("MCP try_acquire exists");
        let release_success_start = source[fn_start..]
            .find("fn release_success")
            .map(|idx| fn_start + idx)
            .expect("release_success follows try_acquire");
        let implementation = &source[fn_start..release_success_start];

        assert!(!implementation.contains("let key = key.to_string();"));
        assert!(implementation.contains("let key = entry.key().clone();"));
    }

    #[test]
    fn legacy_auth_limiter_public_apis_are_not_kept() {
        let source = include_str!("rate_limit.rs");
        let login_impl_start = source
            .find("impl LoginRateLimiter")
            .expect("login limiter impl exists");
        let mcp_impl_start = source
            .find("impl McpAuthRateLimiter")
            .expect("mcp auth limiter impl exists");
        let write_impl_start = source
            .find("impl McpWriteRateLimiter")
            .expect("mcp write limiter impl exists");
        let login_impl = &source[login_impl_start..mcp_impl_start];
        let mcp_impl = &source[mcp_impl_start..write_impl_start];

        for legacy_api in [
            "pub fn check",
            "pub fn record_failure",
            "pub fn record_success",
        ] {
            assert!(
                !login_impl.contains(legacy_api),
                "LoginRateLimiter still exposes legacy API {legacy_api}"
            );
            assert!(
                !mcp_impl.contains(legacy_api),
                "McpAuthRateLimiter still exposes legacy API {legacy_api}"
            );
        }
    }
}
