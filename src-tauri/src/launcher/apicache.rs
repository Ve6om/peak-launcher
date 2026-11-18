//! A small in-memory cache for API responses, so browsing doesn't repeat the
//! same requests (each one counts against the CurseForge proxy's quota).
//! Only successful responses are stored, keyed by the full URL, each with its
//! own expiry. The size is capped so a long session can't grow it forever.

use std::collections::HashMap;
use std::time::{Duration, Instant};

pub const CAPACITY: usize = 300;

struct Entry {
    body: String,
    stored: Instant,
    expires: Instant,
}

pub struct ResponseCache {
    entries: HashMap<String, Entry>,
    capacity: usize,
}

impl ResponseCache {
    pub fn new(capacity: usize) -> Self {
        ResponseCache { entries: HashMap::new(), capacity }
    }

    /// The stored body for `url`, if it hasn't expired.
    pub fn get(&mut self, url: &str, now: Instant) -> Option<String> {
        match self.entries.get(url) {
            Some(e) if e.expires > now => Some(e.body.clone()),
            Some(_) => {
                self.entries.remove(url);
                None
            }
            None => None,
        }
    }

    pub fn put(&mut self, url: String, body: String, ttl: Duration, now: Instant) {
        if self.capacity == 0 {
            return;
        }
        if !self.entries.contains_key(&url) && self.entries.len() >= self.capacity {
            // Make room: drop anything expired, then the oldest entries.
            self.entries.retain(|_, e| e.expires > now);
            while self.entries.len() >= self.capacity {
                let oldest = self.entries.iter().min_by_key(|(_, e)| e.stored).map(|(k, _)| k.clone());
                match oldest {
                    Some(k) => self.entries.remove(&k),
                    None => break,
                };
            }
        }
        self.entries.insert(url, Entry { body, stored: now, expires: now + ttl });
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: Duration = Duration::from_secs(60);

    #[test]
    fn hit_within_expiry_and_miss_after() {
        let mut c = ResponseCache::new(10);
        let t0 = Instant::now();
        c.put("u".into(), "body".into(), 5 * MIN, t0);
        assert_eq!(c.get("u", t0 + 4 * MIN).as_deref(), Some("body"));
        assert_eq!(c.get("u", t0 + 5 * MIN), None);
        assert_eq!(c.len(), 0, "expired entry is dropped");
        assert_eq!(c.get("other", t0), None);
    }

    #[test]
    fn stays_within_capacity_dropping_expired_then_oldest() {
        let mut c = ResponseCache::new(3);
        let t0 = Instant::now();
        c.put("short".into(), "1".into(), MIN, t0);
        c.put("a".into(), "2".into(), 60 * MIN, t0 + Duration::from_secs(1));
        c.put("b".into(), "3".into(), 60 * MIN, t0 + Duration::from_secs(2));
        // "short" has expired by now, so it goes first.
        c.put("c".into(), "4".into(), 60 * MIN, t0 + 2 * MIN);
        assert_eq!(c.len(), 3);
        assert!(c.get("short", t0 + 2 * MIN).is_none());
        // Nothing expired: the oldest ("a") makes room.
        c.put("d".into(), "5".into(), 60 * MIN, t0 + 3 * MIN);
        assert_eq!(c.len(), 3);
        assert!(c.get("a", t0 + 3 * MIN).is_none());
        assert!(c.get("b", t0 + 3 * MIN).is_some());
        assert!(c.get("d", t0 + 3 * MIN).is_some());
        // Replacing an existing key doesn't evict anything.
        c.put("d".into(), "6".into(), 60 * MIN, t0 + 4 * MIN);
        assert_eq!(c.len(), 3);
        assert_eq!(c.get("d", t0 + 4 * MIN).as_deref(), Some("6"));
    }

    #[test]
    fn clear_empties_it() {
        let mut c = ResponseCache::new(10);
        let t0 = Instant::now();
        c.put("u".into(), "body".into(), MIN, t0);
        c.clear();
        assert_eq!(c.get("u", t0), None);
    }
}
