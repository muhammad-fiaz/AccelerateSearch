//! FST-backed term dictionary for O(log n) prefix and exact lookup.
//!
//! Wraps the `fst` crate with a builder API that ingests `(term, value)`
//! pairs in sorted order and produces an immutable `Map`. The map is then
//! stored alongside the in-memory `InvertedIndex` so that search can resolve
//! term corrections and prefix queries in microseconds regardless of the
//! collection size.

use fst::{IntoStreamer, Map, MapBuilder, Streamer};

/// A read-only term dictionary mapping each term to a `u64` payload (for
/// example, the term's `total_term_freq`).
#[derive(Clone, Default)]
pub struct TermDict {
    inner: Option<Map<Vec<u8>>>,
}

impl TermDict {
    /// Creates an empty dictionary.
    #[must_use]
    pub fn new() -> Self {
        Self { inner: None }
    }

    /// Builds a `TermDict` from an iterator of `(term, payload)` pairs.
    ///
    /// The input **must be sorted** by `term` (byte-wise) and contain no
    /// duplicate keys. Use [`crate::TermDictBuilder`] for a streaming
    /// builder that does not require a pre-sorted slice.
    ///
    /// # Errors
    /// Returns an error if the FST cannot be built (e.g. the inputs are not
    /// sorted).
    pub fn from_sorted<'a, I>(pairs: I) -> Result<Self, fst::Error>
    where
        I: IntoIterator<Item = (&'a str, u64)>,
    {
        let mut buf: Vec<u8> = Vec::new();
        {
            let mut builder = MapBuilder::new(&mut buf)?;
            for (k, v) in pairs {
                builder.insert(k, v)?;
            }
            builder.finish()?;
        }
        let map = Map::new(buf)?;
        Ok(Self { inner: Some(map) })
    }

    /// Returns true if the dictionary is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_none() || self.as_ref().is_empty()
    }

    /// Returns the number of distinct terms in the dictionary.
    #[must_use]
    pub fn len(&self) -> usize {
        self.as_ref().len()
    }

    /// Looks up an exact term, returning the payload if present.
    #[must_use]
    pub fn get(&self, term: &str) -> Option<u64> {
        self.as_ref().get(term)
    }

    /// Returns all terms that start with `prefix`, up to `limit` entries.
    ///
    /// The returned vector is sorted lexicographically. The lookups are
    /// O(log n + k) where `k` is the result size.
    #[must_use]
    pub fn prefix(&self, prefix: &str, limit: usize) -> Vec<(String, u64)> {
        if limit == 0 {
            return Vec::new();
        }
        let map = self.as_ref();
        let upper = next_lexicographic(prefix);
        let mut out = Vec::new();
        let mut stream = map.range().ge(prefix).lt(&upper).into_stream();
        while let Some((k, v)) = stream.next() {
            if out.len() >= limit {
                break;
            }
            let s = String::from_utf8_lossy(k).into_owned();
            out.push((s, v));
        }
        out
    }

    /// Streams the entire dictionary, calling `visit` for each entry.
    pub fn for_each<F: FnMut(&str, u64)>(&self, mut visit: F) {
        let map = self.as_ref();
        let mut stream = map.into_stream();
        while let Some((k, v)) = stream.next() {
            visit(std::str::from_utf8(k).unwrap_or(""), v);
        }
    }

    fn as_ref(&self) -> &Map<Vec<u8>> {
        static EMPTY: std::sync::OnceLock<Map<Vec<u8>>> = std::sync::OnceLock::new();
        EMPTY.get_or_init(Map::default);
        self.inner.as_ref().unwrap_or_else(|| EMPTY.get().unwrap())
    }
}

impl std::fmt::Debug for TermDict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TermDict")
            .field("len", &self.len())
            .finish_non_exhaustive()
    }
}

/// Streaming builder for [`TermDict`] that maintains a sorted buffer and
/// can append new entries without requiring a pre-sorted slice.
pub struct TermDictBuilder {
    entries: std::collections::BTreeMap<String, u64>,
}

impl TermDictBuilder {
    /// Creates a new, empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: std::collections::BTreeMap::new(),
        }
    }

    /// Creates a builder pre-populated with `entries`.
    #[must_use]
    pub fn with_capacity(cap: usize) -> Self {
        // BTreeMap has no `with_capacity`; the cap is a hint only.
        let _ = cap;
        Self {
            entries: std::collections::BTreeMap::new(),
        }
    }

    /// Inserts or replaces a term's payload. If a payload already exists for
    /// `term`, the values are summed.
    pub fn add(&mut self, term: impl Into<String>, value: u64) {
        self.entries
            .entry(term.into())
            .and_modify(|v| *v = v.saturating_add(value))
            .or_insert(value);
    }

    /// Merges the contents of another builder.
    pub fn extend(&mut self, other: TermDictBuilder) {
        for (k, v) in other.entries {
            self.entries
                .entry(k)
                .and_modify(|cur| *cur = cur.saturating_add(v))
                .or_insert(v);
        }
    }

    /// Consumes the builder and produces an immutable [`TermDict`].
    ///
    /// # Errors
    /// Returns an error if the FST cannot be built.
    pub fn build(self) -> Result<TermDict, fst::Error> {
        TermDict::from_sorted(self.entries.iter().map(|(k, v)| (k.as_str(), *v)))
    }

    /// Returns the number of unique terms buffered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the builder is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for TermDictBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns the smallest string that is strictly greater than `prefix` in
/// lexicographic byte order. Used to build half-open ranges for prefix
/// queries.
fn next_lexicographic(prefix: &str) -> String {
    let mut bytes = prefix.as_bytes().to_vec();
    for b in bytes.iter_mut().rev() {
        if *b < 0xFF {
            *b += 1;
            bytes.truncate(bytes.len());
            return String::from_utf8_lossy(&bytes).into_owned();
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_and_lookup() {
        let mut b = TermDictBuilder::new();
        b.add("apple", 10);
        b.add("banana", 20);
        b.add("cherry", 30);
        let d = b.build().unwrap();
        assert_eq!(d.len(), 3);
        assert_eq!(d.get("apple"), Some(10));
        assert_eq!(d.get("banana"), Some(20));
        assert_eq!(d.get("missing"), None);
    }

    #[test]
    fn prefix_lookup() {
        let mut b = TermDictBuilder::new();
        for (i, w) in ["apple", "apricot", "avocado", "banana", "blueberry"]
            .iter()
            .enumerate()
        {
            b.add(*w, i as u64);
        }
        let d = b.build().unwrap();
        let p = d.prefix("ap", 100);
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].0, "apple");
        assert_eq!(p[1].0, "apricot");

        let b2 = d.prefix("b", 1);
        assert_eq!(b2.len(), 1);
        assert_eq!(b2[0].0, "banana");

        assert!(d.prefix("zzz", 10).is_empty());
    }

    #[test]
    fn extend_sums_values() {
        let mut b1 = TermDictBuilder::new();
        b1.add("hello", 5);
        b1.add("world", 1);
        let mut b2 = TermDictBuilder::new();
        b2.add("hello", 7);
        b2.add("foo", 2);
        b1.extend(b2);
        let d = b1.build().unwrap();
        assert_eq!(d.get("hello"), Some(12));
        assert_eq!(d.get("world"), Some(1));
        assert_eq!(d.get("foo"), Some(2));
    }

    #[test]
    fn empty_dict_works() {
        let d = TermDict::new();
        assert!(d.is_empty());
        assert_eq!(d.get("anything"), None);
        assert!(d.prefix("any", 10).is_empty());
        assert_eq!(d.len(), 0);
    }
}
