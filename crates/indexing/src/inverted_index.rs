//! In-memory inverted index data structures used by the indexing pipeline.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use models::DocumentId;

use crate::term_dict::TermDict;

/// Per-document field length statistics used for BM25 normalisation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FieldLengths(pub BTreeMap<String, usize>);

impl FieldLengths {
    /// Returns the length of a single field, defaulting to 0.
    #[must_use]
    pub fn get(&self, field: &str) -> usize {
        self.0.get(field).copied().unwrap_or(0)
    }

    /// Returns the sum of all field lengths.
    #[must_use]
    pub fn total(&self) -> usize {
        self.0.values().sum()
    }

    /// Increments the length of a field by `n`.
    pub fn add(&mut self, field: &str, n: usize) {
        *self.0.entry(field.to_string()).or_insert(0) += n;
    }
}

/// Per-term statistics in a collection.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TermInfo {
    /// Number of documents that contain the term at least once.
    pub doc_freq: u32,
    /// Total number of occurrences across all documents.
    pub total_term_freq: u64,
    /// Per-field term frequencies, summed across all documents.
    #[serde(default)]
    pub field_term_freq: BTreeMap<String, u64>,
}

/// Per-document, per-term posting.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Posting {
    /// Term frequency for this posting.
    pub tf: u32,
    /// Per-field positions (optional).
    #[serde(default)]
    pub field_tf: BTreeMap<String, u32>,
}

/// Collection-level statistics used for BM25 scoring.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CollectionStats {
    /// Number of documents in the collection.
    pub doc_count: u64,
    /// Sum of all field lengths across the collection.
    pub total_field_length: u64,
    /// Average field length per document.
    pub avg_field_length: f64,
}

/// Mutable in-memory representation of a single collection's inverted
/// index. The storage layer persists a serialised snapshot of this
/// structure.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InvertedIndex {
    /// Term -> TermInfo.
    pub terms: BTreeMap<String, TermInfo>,
    /// Term -> doc_id -> Posting.
    pub postings: BTreeMap<String, BTreeMap<DocumentId, Posting>>,
    /// Document -> FieldLengths.
    pub field_lengths: BTreeMap<DocumentId, FieldLengths>,
    /// Collection-level stats.
    pub stats: CollectionStats,
    /// FST-backed term dictionary, rebuilt on every commit. Provides
    /// O(log n) prefix and exact lookups. `None` for an empty index.
    /// Not persisted to storage — rebuilt on load.
    #[serde(default, skip)]
    pub term_dict: Option<TermDict>,
}

impl InvertedIndex {
    /// Creates a new empty inverted index.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if the index is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.stats.doc_count == 0
    }

    /// Recomputes collection-level statistics from scratch.
    pub fn recompute_stats(&mut self) {
        let mut total_field_length: u64 = 0;
        for lengths in self.field_lengths.values() {
            total_field_length += lengths.total() as u64;
        }
        let doc_count = self.field_lengths.len() as u64;
        let avg = if doc_count == 0 {
            0.0
        } else {
            total_field_length as f64 / doc_count as f64
        };
        self.stats = CollectionStats {
            doc_count,
            total_field_length,
            avg_field_length: avg,
        };
    }

    /// Removes a document from the index, returning the list of terms that
    /// were affected.
    pub fn remove_document(&mut self, doc_id: &DocumentId) -> Vec<String> {
        let mut affected = Vec::new();
        let lengths = self.field_lengths.remove(doc_id);
        for (term, map) in self.postings.iter_mut() {
            if let Some(posting) = map.remove(doc_id) {
                if let Some(info) = self.terms.get_mut(term) {
                    info.doc_freq = info.doc_freq.saturating_sub(1);
                    info.total_term_freq -= posting.tf as u64;
                    for (field, count) in &posting.field_tf {
                        if let Some(field_freq) = info.field_term_freq.get_mut(field) {
                            *field_freq = field_freq.saturating_sub(*count as u64);
                            if *field_freq == 0 {
                                info.field_term_freq.remove(field);
                            }
                        }
                    }
                }
                if map.is_empty() {
                    affected.push(term.clone());
                }
            }
        }
        for term in &affected {
            if let Some(map) = self.postings.get(term)
                && map.is_empty()
            {
                self.postings.remove(term);
                self.terms.remove(term);
            }
        }
        if lengths.is_some() {
            self.recompute_stats();
        }
        affected
    }

    /// Inserts a single (term, field) occurrence for a document, updating
    /// the term info and posting.
    pub fn add_occurrence(&mut self, term: &str, field: &str, doc_id: &DocumentId) {
        let posting_entry = self
            .postings
            .entry(term.to_string())
            .or_default()
            .entry(doc_id.clone())
            .or_default();
        posting_entry.tf += 1;
        *posting_entry.field_tf.entry(field.to_string()).or_insert(0) += 1;

        let info = self.terms.entry(term.to_string()).or_default();
        if posting_entry.tf == 1 {
            info.doc_freq += 1;
        }
        info.total_term_freq += 1;
        *info.field_term_freq.entry(field.to_string()).or_insert(0) += 1;
    }

    /// Increments the stored field length for a document.
    pub fn add_field_length(&mut self, doc_id: &DocumentId, field: &str, n: usize) {
        let entry = self.field_lengths.entry(doc_id.clone()).or_default();
        entry.add(field, n);
    }

    /// Rebuilds the FST-backed term dictionary from `terms`. Call after a
    /// batch of mutations to make prefix queries fast.
    pub fn rebuild_term_dict(&mut self) {
        let mut builder = crate::term_dict::TermDictBuilder::with_capacity(self.terms.len());
        for (term, info) in &self.terms {
            builder.add(term.as_str(), info.total_term_freq);
        }
        self.term_dict = Some(
            builder
                .build()
                .unwrap_or_else(|_| crate::term_dict::TermDict::new()),
        );
    }

    /// Returns all terms that start with `prefix`, up to `limit` entries.
    #[must_use]
    pub fn term_prefix(&self, prefix: &str, limit: usize) -> Vec<(String, u64)> {
        match &self.term_dict {
            Some(d) => d.prefix(prefix, limit),
            None => {
                if limit == 0 {
                    return Vec::new();
                }
                self.terms
                    .range(prefix.to_string()..)
                    .take_while(|(k, _)| k.starts_with(prefix))
                    .take(limit)
                    .map(|(k, info)| (k.clone(), info.total_term_freq))
                    .collect()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(s: &str) -> DocumentId {
        DocumentId::new(s)
    }

    #[test]
    fn add_occurrence_updates_posting_and_term() {
        let mut idx = InvertedIndex::new();
        idx.add_occurrence("hello", "title", &doc("1"));
        idx.add_occurrence("hello", "title", &doc("1"));
        idx.add_occurrence("hello", "body", &doc("1"));
        idx.add_occurrence("hello", "title", &doc("2"));
        let info = idx.terms.get("hello").unwrap();
        assert_eq!(info.doc_freq, 2);
        assert_eq!(info.total_term_freq, 4);
        let posting = idx.postings.get("hello").unwrap().get(&doc("1")).unwrap();
        assert_eq!(posting.tf, 3);
    }

    #[test]
    fn remove_document_drops_terms() {
        let mut idx = InvertedIndex::new();
        idx.add_occurrence("hello", "title", &doc("1"));
        idx.add_occurrence("world", "title", &doc("1"));
        idx.add_field_length(&doc("1"), "title", 2);
        idx.recompute_stats();
        let affected = idx.remove_document(&doc("1"));
        assert!(affected.contains(&"hello".to_string()));
        assert!(affected.contains(&"world".to_string()));
        assert_eq!(idx.stats.doc_count, 0);
    }

    #[test]
    fn field_lengths_default_zero() {
        let f = FieldLengths::default();
        assert_eq!(f.get("missing"), 0);
        assert_eq!(f.total(), 0);
    }

    #[test]
    fn recompute_stats_works() {
        let mut idx = InvertedIndex::new();
        idx.add_field_length(&doc("1"), "title", 5);
        idx.add_field_length(&doc("2"), "title", 10);
        idx.recompute_stats();
        assert_eq!(idx.stats.doc_count, 2);
        assert_eq!(idx.stats.total_field_length, 15);
        assert!((idx.stats.avg_field_length - 7.5).abs() < 1e-9);
    }

    #[test]
    fn term_dict_prefix_lookup() {
        let mut idx = InvertedIndex::new();
        for w in ["apple", "apricot", "avocado", "banana"] {
            idx.add_occurrence(w, "body", &doc("1"));
        }
        idx.rebuild_term_dict();
        let p = idx.term_prefix("ap", 100);
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].0, "apple");
        assert_eq!(p[1].0, "apricot");
        let p2 = idx.term_prefix("z", 100);
        assert!(p2.is_empty());
    }

    #[test]
    fn term_dict_falls_back_when_not_built() {
        let mut idx = InvertedIndex::new();
        idx.add_occurrence("apple", "body", &doc("1"));
        idx.add_occurrence("apricot", "body", &doc("1"));
        // No rebuild_term_dict call -> fallback to BTreeMap range scan.
        let p = idx.term_prefix("ap", 10);
        assert_eq!(p.len(), 2);
    }
}
