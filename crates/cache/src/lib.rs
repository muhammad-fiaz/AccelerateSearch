//! LRU + TTL cache used by AccelerateSearch for search results, per-key
//! lookups, and any other expensive read.

use std::hash::Hash;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

use lru::LruCache;
use parking_lot::Mutex;

use config_crate::CacheConfig;

/// Thread-safe LRU + TTL cache.
pub struct TtlCache<K, V> {
    inner: Arc<Mutex<LruInner<K, V>>>,
    ttl: Duration,
    enabled: bool,
}

struct LruInner<K, V> {
    entries: LruCache<K, Entry<V>>,
}

struct Entry<V> {
    value: V,
    expires_at: chrono::DateTime<chrono::Utc>,
}

impl<K, V> TtlCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Creates a new cache with the given configuration.
    #[must_use]
    pub fn new(cfg: &CacheConfig) -> Self {
        let cap = NonZeroUsize::new(cfg.max_entries.max(1)).unwrap();
        Self {
            inner: Arc::new(Mutex::new(LruInner {
                entries: LruCache::new(cap),
            })),
            ttl: Duration::from_secs(cfg.ttl_seconds.max(1)),
            enabled: cfg.enabled,
        }
    }

    /// Returns true if the cache is enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Inserts a value into the cache.
    pub fn put(&self, key: K, value: V) {
        if !self.enabled {
            return;
        }
        let mut guard = self.inner.lock();
        let expires_at = chrono::Utc::now()
            + chrono::Duration::from_std(self.ttl).unwrap_or(chrono::Duration::seconds(60));
        guard.entries.put(key, Entry { value, expires_at });
    }

    /// Looks up a value, returning `None` if missing or expired.
    pub fn get(&self, key: &K) -> Option<V> {
        if !self.enabled {
            return None;
        }
        let mut guard = self.inner.lock();
        let entry = guard.entries.get(key)?;
        if entry.expires_at < chrono::Utc::now() {
            guard.entries.pop(key);
            return None;
        }
        Some(entry.value.clone())
    }

    /// Removes a value from the cache.
    pub fn invalidate(&self, key: &K) {
        let mut guard = self.inner.lock();
        guard.entries.pop(key);
    }

    /// Clears the cache.
    pub fn clear(&self) {
        let mut guard = self.inner.lock();
        guard.entries.clear();
    }

    /// Returns the number of live (non-expired) entries.
    #[must_use]
    pub fn len(&self) -> usize {
        let guard = self.inner.lock();
        let now = chrono::Utc::now();
        guard
            .entries
            .iter()
            .filter(|(_, e)| e.expires_at > now)
            .count()
    }

    /// Returns true if the cache has no live entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use config_crate::CacheConfig;
    use std::thread::sleep;

    fn cfg(ttl_seconds: u64, max_entries: usize) -> CacheConfig {
        CacheConfig {
            enabled: true,
            max_entries,
            ttl_seconds,
        }
    }

    #[test]
    fn put_get_works() {
        let c: TtlCache<String, String> = TtlCache::new(&cfg(60, 16));
        c.put("k".into(), "v".into());
        assert_eq!(c.get(&"k".into()).as_deref(), Some("v"));
    }

    #[test]
    fn ttl_expires_entries() {
        let c: TtlCache<String, String> = TtlCache::new(&cfg(1, 16));
        c.put("k".into(), "v".into());
        assert_eq!(c.get(&"k".into()).as_deref(), Some("v"));
        sleep(Duration::from_millis(1100));
        assert!(c.get(&"k".into()).is_none());
    }

    #[test]
    fn lru_evicts_old_entries() {
        let c: TtlCache<i32, i32> = TtlCache::new(&cfg(60, 2));
        c.put(1, 10);
        c.put(2, 20);
        c.put(3, 30);
        assert!(c.get(&1).is_none());
        assert_eq!(c.get(&2), Some(20));
        assert_eq!(c.get(&3), Some(30));
    }

    #[test]
    fn disabled_cache_returns_none() {
        let mut cfg = cfg(60, 16);
        cfg.enabled = false;
        let c: TtlCache<String, String> = TtlCache::new(&cfg);
        c.put("k".into(), "v".into());
        assert!(c.get(&"k".into()).is_none());
    }
}
