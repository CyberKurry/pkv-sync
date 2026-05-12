use dashmap::DashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
struct Entry {
    failures: u32,
    first_failure: Instant,
    locked_until: Option<Instant>,
}

#[derive(Clone)]
pub struct LoginRateLimiter {
    inner: Arc<DashMap<IpAddr, Entry>>,
    config: Arc<RwLock<Config>>,
}

#[derive(Debug, Clone, Copy)]
struct Config {
    threshold: u32,
    window: Duration,
    lock_duration: Duration,
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

    pub fn prune_stale(&self) -> usize {
        let now = Instant::now();
        let config = *self.config.read().expect("login limiter lock poisoned");
        let stale: Vec<IpAddr> = self
            .inner
            .iter()
            .filter_map(|entry| entry_is_stale(entry.value(), now, config).then_some(*entry.key()))
            .collect();
        let removed = stale.len();
        for ip in stale {
            self.inner.remove(&ip);
        }
        removed
    }
}

fn entry_is_stale(entry: &Entry, now: Instant, config: Config) -> bool {
    if let Some(until) = entry.locked_until {
        return until <= now;
    }
    now.duration_since(entry.first_failure) > config.window
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
}
