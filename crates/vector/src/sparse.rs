//! Sparse vector and multi-vector (ColBERT-style) support.
//!
//! * [`SparseVector`] — represents an embedding as a sparse map of
//!   non-zero dimensions and their weights. Useful for SPLADE / BM25
//!   embeddings and exact lexical-style scoring.
//! * [`MultiVector`] — holds a list of per-token vectors used by late
//!   interaction models such as ColBERT.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Sparse vector stored as a map of dimension -> weight.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SparseVector {
    /// Non-zero entries.
    pub entries: HashMap<u32, f32>,
}

impl SparseVector {
    /// Creates an empty sparse vector.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Constructs a sparse vector from a dense `f32` slice by dropping zeros.
    #[must_use]
    pub fn from_dense(dense: &[f32]) -> Self {
        let mut entries = HashMap::new();
        for (i, &v) in dense.iter().enumerate() {
            if v != 0.0 {
                entries.insert(i as u32, v);
            }
        }
        Self { entries }
    }

    /// Returns the number of non-zero entries.
    #[must_use]
    pub fn nnz(&self) -> usize {
        self.entries.len()
    }

    /// Returns the L2 norm of the vector.
    #[must_use]
    pub fn l2_norm(&self) -> f32 {
        self.entries
            .values()
            .map(|v| {
                let v = *v as f64;
                (v * v) as f32
            })
            .sum::<f32>()
            .sqrt()
    }

    /// Computes the dot product with another sparse vector. Dimensions
    /// missing on either side are treated as zero.
    #[must_use]
    pub fn dot(&self, other: &Self) -> f32 {
        let (small, large) = if self.entries.len() <= other.entries.len() {
            (self, other)
        } else {
            (other, self)
        };
        let mut total = 0.0f32;
        for (k, v) in &small.entries {
            if let Some(w) = large.entries.get(k) {
                total += v * w;
            }
        }
        total
    }

    /// Cosine similarity, defined as `dot / (||a|| * ||b||)`.
    #[must_use]
    pub fn cosine(&self, other: &Self) -> f32 {
        let denom = self.l2_norm() * other.l2_norm();
        if denom == 0.0 {
            return 0.0;
        }
        self.dot(other) / denom
    }
}

/// Container for multi-vector embeddings (ColBERT / ColPali style).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MultiVector {
    /// One dense vector per token.
    pub vectors: Vec<Vec<f32>>,
}

impl MultiVector {
    /// Creates a new multi-vector.
    #[must_use]
    pub fn new(vectors: Vec<Vec<f32>>) -> Self {
        Self { vectors }
    }

    /// Number of token vectors.
    #[must_use]
    pub fn len(&self) -> usize {
        self.vectors.len()
    }

    /// True if there are no token vectors.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }

    /// Computes the late-interaction score against a query multi-vector.
    /// For each query vector we take the maximum similarity to any document
    /// vector and sum the result. This is the ColBERT MaxSim operator.
    #[must_use]
    pub fn max_sim(&self, query: &Self) -> f32 {
        let mut total = 0.0f32;
        for q in &query.vectors {
            let mut best = f32::NEG_INFINITY;
            for d in &self.vectors {
                if q.len() != d.len() {
                    continue;
                }
                let s: f32 = q.iter().zip(d.iter()).map(|(a, b)| a * b).sum();
                if s > best {
                    best = s;
                }
            }
            if best.is_finite() {
                total += best;
            }
        }
        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparse_vector_from_dense_drops_zeros() {
        let s = SparseVector::from_dense(&[0.0, 1.0, 0.0, 2.0]);
        assert_eq!(s.nnz(), 2);
        assert_eq!(s.entries.get(&1), Some(&1.0));
        assert_eq!(s.entries.get(&3), Some(&2.0));
    }

    #[test]
    fn sparse_dot_and_cosine() {
        let a = SparseVector::from_dense(&[1.0, 0.0, 2.0, 0.0]);
        let b = SparseVector::from_dense(&[2.0, 0.0, 3.0, 0.0]);
        assert!((a.dot(&b) - 8.0).abs() < 1e-6);
        assert!((a.cosine(&b) - 0.992_277).abs() < 1e-3);
    }

    #[test]
    fn multi_vector_max_sim() {
        let q = MultiVector::new(vec![vec![1.0, 0.0], vec![0.0, 1.0]]);
        let d = MultiVector::new(vec![vec![0.5, 0.0], vec![0.0, 0.7], vec![1.0, 0.0]]);
        let s = d.max_sim(&q);
        // q[0] -> max(0.5, 1.0) = 1.0
        // q[1] -> max(0.7)     = 0.7
        assert!((s - 1.7).abs() < 1e-6);
    }

    #[test]
    fn multi_vector_empty_query_is_zero() {
        let d = MultiVector::new(vec![vec![1.0, 0.0]]);
        let q = MultiVector::new(vec![]);
        assert_eq!(d.max_sim(&q), 0.0);
    }
}
