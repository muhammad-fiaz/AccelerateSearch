//! BM25 ranking implementation.

use std::collections::HashMap;

use indexing::{FieldLengths, InvertedIndex, Posting, TermInfo};

/// Default BM25 parameters. `k1` controls term-frequency saturation, `b`
/// controls length normalisation.
pub const DEFAULT_K1: f64 = 1.2;
pub const DEFAULT_B: f64 = 0.75;

/// Computes the BM25 score of a single term posting for a document.
#[must_use]
pub fn bm25_score(
    term_info: &TermInfo,
    posting: &Posting,
    field_length: usize,
    avg_field_length: f64,
    total_docs: u64,
    k1: f64,
    b: f64,
) -> f64 {
    if total_docs == 0 {
        return 0.0;
    }
    let n = term_info.doc_freq as f64;
    let idf = ((total_docs as f64 - n + 0.5) / (n + 0.5) + 1.0).ln();
    let tf = posting.tf as f64;
    let norm = 1.0 - b + b * (field_length as f64 / avg_field_length.max(1.0));
    idf * (tf * (k1 + 1.0)) / (tf + k1 * norm)
}

/// Computes the BM25 score of a document for a multi-term query.
///
/// `term_scores` maps a term to `(term_info, posting, field_length)`. `avg_field_length`
/// is the collection average field length, and `total_docs` is the number of
/// documents in the collection.
#[must_use]
pub fn bm25_document_score(
    term_scores: &[(TermInfo, Posting, usize)],
    avg_field_length: f64,
    total_docs: u64,
    k1: f64,
    b: f64,
) -> f64 {
    term_scores
        .iter()
        .map(|(info, posting, len)| {
            bm25_score(info, posting, *len, avg_field_length, total_docs, k1, b)
        })
        .sum()
}

/// Ranks the candidate documents using BM25. Returns a vector of
/// `(doc_id, score)` sorted by score descending.
#[must_use]
pub fn rank(index: &InvertedIndex, terms: &[String], k1: f64, b: f64) -> Vec<(String, f64)> {
    let total = index.stats.doc_count;
    if total == 0 {
        return Vec::new();
    }
    let avg = index.stats.avg_field_length.max(1.0);
    // HashMap is significantly faster than BTreeMap for the insert-heavy
    // workload we have here: postings are looked up by doc-id (an opaque
    // key) and we only need to sort by score at the end.
    let mut candidates: HashMap<&str, Vec<(TermInfo, Posting, usize)>> = HashMap::new();
    for term in terms {
        let Some(info) = index.terms.get(term) else {
            continue;
        };
        let Some(postings) = index.postings.get(term) else {
            continue;
        };
        for (doc_id, posting) in postings {
            let field_length = index
                .field_lengths
                .get(doc_id)
                .map(FieldLengths::total)
                .unwrap_or(0);
            candidates.entry(doc_id.as_str()).or_default().push((
                info.clone(),
                posting.clone(),
                field_length,
            ));
        }
    }
    let mut scored: Vec<(String, f64)> = candidates
        .into_iter()
        .map(|(doc_id, infos)| {
            let score = bm25_document_score(&infos, avg, total, k1, b);
            (doc_id.to_string(), score)
        })
        .filter(|(_, s)| *s > 0.0)
        .collect();
    scored.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexing::{FieldLengths, InvertedIndex, TermInfo};
    use models::DocumentId;

    fn build_index() -> InvertedIndex {
        let mut idx = InvertedIndex::new();
        // Doc 1: "the quick brown fox"
        for (term, freq) in [("the", 1), ("quick", 1), ("brown", 1), ("fox", 1)] {
            for _ in 0..freq {
                idx.add_occurrence(term, "body", &DocumentId::new("1"));
            }
        }
        idx.add_field_length(&DocumentId::new("1"), "body", 4);
        // Doc 2: "the lazy dog"
        for term in ["the", "lazy", "dog"] {
            idx.add_occurrence(term, "body", &DocumentId::new("2"));
        }
        idx.add_field_length(&DocumentId::new("2"), "body", 3);
        // Doc 3: "the fox"
        for term in ["the", "fox"] {
            idx.add_occurrence(term, "body", &DocumentId::new("3"));
        }
        idx.add_field_length(&DocumentId::new("3"), "body", 2);
        idx.recompute_stats();
        idx
    }

    #[test]
    fn rank_relevance_works() {
        let idx = build_index();
        let hits = rank(&idx, &["fox".into()], 1.2, 0.75);
        assert!(!hits.is_empty());
        // Both 1 and 3 contain "fox"; 1 has the longer body, so 3 may rank
        // higher due to length normalisation. Either way, both appear.
        let doc_ids: Vec<&str> = hits.iter().map(|(d, _)| d.as_str()).collect();
        assert!(doc_ids.contains(&"1"));
        assert!(doc_ids.contains(&"3"));
    }

    #[test]
    fn bm25_with_zero_total_docs_is_zero() {
        let info = TermInfo::default();
        let posting = Posting::default();
        assert_eq!(bm25_score(&info, &posting, 10, 10.0, 0, 1.2, 0.75), 0.0);
    }

    #[test]
    fn bm25_is_higher_for_relevant_term() {
        let idx = build_index();
        let hits = rank(&idx, &["fox".into()], 1.2, 0.75);
        let best = &hits[0];
        assert!(best.1 > 0.0);
    }

    #[test]
    fn field_lengths_collected_correctly() {
        let mut fl = FieldLengths::default();
        fl.add("body", 5);
        fl.add("title", 2);
        assert_eq!(fl.get("body"), 5);
        assert_eq!(fl.total(), 7);
    }
}
