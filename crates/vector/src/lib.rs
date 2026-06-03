//! Vector search and embedding management.
//!
//! Provides a [`VectorIndex`] trait and an in-memory fallback implementation
//! using brute-force cosine / dot / Euclidean distance. The
//! [`hnsw_rs`](https://docs.rs/hnsw_rs) backend can be plugged in via the
//! feature flag in the future.

pub mod quantization;
pub mod sparse;

pub use quantization::{
    BinaryQuantizer, ProductQuantizer, Quantizer, QuantizerDispatch, ScalarQuantizer,
};
pub use sparse::{MultiVector, SparseVector};

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use models::Similarity;
use serde::{Deserialize, Serialize};

use errors::AppResult;
use models::DocumentId;

/// A point in vector space.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VectorPoint {
    /// Identifier of the document this vector represents.
    pub id: DocumentId,
    /// The vector itself.
    pub vector: Vec<f32>,
}

impl VectorPoint {
    /// Creates a new vector point.
    #[must_use]
    pub fn new(id: DocumentId, vector: Vec<f32>) -> Self {
        Self { id, vector }
    }
}

/// Embedding type carried by a document. The default representation is a
/// dense `f32` vector; sparse and multi-vector payloads are supported for
/// models such as SPLADE and ColBERT.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Embedding {
    /// Dense `f32` vector.
    Dense(Vec<f32>),
    /// Sparse map of dimension -> weight.
    Sparse(sparse::SparseVector),
    /// Multi-vector (ColBERT-style) embedding.
    Multi(sparse::MultiVector),
}

impl Embedding {
    /// Wraps a dense `f32` slice as a dense embedding.
    #[must_use]
    pub fn from_dense(v: Vec<f32>) -> Self {
        Embedding::Dense(v)
    }

    /// Returns the dimensionality hint of the embedding.
    #[must_use]
    pub fn dim(&self) -> usize {
        match self {
            Embedding::Dense(v) => v.len(),
            Embedding::Sparse(s) => s
                .entries
                .values()
                .len()
                .max(s.entries.keys().map(|k| *k as usize + 1).max().unwrap_or(0)),
            Embedding::Multi(m) => m.vectors.first().map_or(0, |v| v.len()),
        }
    }

    /// Computes the similarity to another embedding using the given metric.
    /// Behaviour depends on the types involved:
    /// * Dense + Dense: standard cosine / dot / euclidean.
    /// * Sparse + Sparse: cosine.
    /// * Multi + Multi: late-interaction MaxSim (only meaningful for dot product).
    /// * Mixed types: returns 0.0.
    #[must_use]
    pub fn similarity(&self, other: &Self, sim: Similarity) -> f32 {
        match (self, other) {
            (Embedding::Dense(a), Embedding::Dense(b)) => match sim {
                Similarity::Cosine => cosine_similarity(a, b),
                Similarity::Dot => dot_product(a, b),
                Similarity::Euclidean => -euclidean_distance(a, b),
            },
            (Embedding::Sparse(a), Embedding::Sparse(b)) => match sim {
                Similarity::Cosine => a.cosine(b),
                Similarity::Dot => a.dot(b),
                _ => 0.0,
            },
            (Embedding::Multi(a), Embedding::Multi(b)) => a.max_sim(b),
            _ => 0.0,
        }
    }
}

/// Trait every vector index backend must implement.
#[async_trait]
pub trait VectorIndex: Send + Sync + 'static {
    /// Inserts or replaces a vector for the given document.
    async fn upsert(&self, point: VectorPoint) -> AppResult<()>;
    /// Removes the vector for the given document.
    async fn delete(&self, id: &DocumentId) -> AppResult<bool>;
    /// Returns the top-k most similar documents to the query vector.
    async fn search(&self, query: &[f32], k: usize) -> AppResult<Vec<(DocumentId, f32)>>;
    /// Returns the number of vectors currently stored.
    async fn len(&self) -> AppResult<usize>;
    /// Returns true if the index is empty.
    async fn is_empty(&self) -> AppResult<bool> {
        Ok(self.len().await? == 0)
    }
}

/// In-memory brute-force vector index. Suitable for small to medium
/// collections; replace with HNSW for production scale.
pub struct InMemoryVectorIndex {
    similarity: Similarity,
    points: DashMap<DocumentId, Vec<f32>>,
}

impl InMemoryVectorIndex {
    /// Creates a new in-memory index.
    #[must_use]
    pub fn new(similarity: Similarity) -> Self {
        Self {
            similarity,
            points: DashMap::new(),
        }
    }

    /// Returns the configured similarity metric.
    #[must_use]
    pub fn similarity(&self) -> Similarity {
        self.similarity
    }

    fn score(&self, a: &[f32], b: &[f32]) -> f32 {
        match self.similarity {
            Similarity::Cosine => cosine_similarity(a, b),
            Similarity::Dot => dot_product(a, b),
            Similarity::Euclidean => -euclidean_distance(a, b),
        }
    }
}

#[async_trait]
impl VectorIndex for InMemoryVectorIndex {
    async fn upsert(&self, point: VectorPoint) -> AppResult<()> {
        self.points.insert(point.id, point.vector);
        Ok(())
    }

    async fn delete(&self, id: &DocumentId) -> AppResult<bool> {
        Ok(self.points.remove(id).is_some())
    }

    async fn search(&self, query: &[f32], k: usize) -> AppResult<Vec<(DocumentId, f32)>> {
        let mut scored: Vec<(DocumentId, f32)> = self
            .points
            .iter()
            .map(|kv| (kv.key().clone(), self.score(query, kv.value())))
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(k);
        Ok(scored)
    }

    async fn len(&self) -> AppResult<usize> {
        Ok(self.points.len())
    }
}

/// Cosine similarity, in `[-1.0, 1.0]`.
#[must_use]
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f64;
    let mut na = 0.0f64;
    let mut nb = 0.0f64;
    for (x, y) in a.iter().zip(b.iter()) {
        let x = *x as f64;
        let y = *y as f64;
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    (dot / (na.sqrt() * nb.sqrt())) as f32
}

/// Dot product of two equal-length vectors.
#[must_use]
pub fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// Euclidean distance between two equal-length vectors.
#[must_use]
pub fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return f32::INFINITY;
    }
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| {
            let d = (*x as f64) - (*y as f64);
            (d * d) as f32
        })
        .sum::<f32>()
        .sqrt()
}

/// Computes a per-collection vector index, keyed by collection UID.
pub struct VectorIndexStore {
    indexes: DashMap<String, Arc<InMemoryVectorIndex>>,
}

impl VectorIndexStore {
    /// Creates a new store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            indexes: DashMap::new(),
        }
    }

    /// Returns (or lazily creates) the vector index for a collection.
    pub fn for_collection(
        &self,
        collection: &str,
        similarity: Similarity,
    ) -> Arc<InMemoryVectorIndex> {
        self.indexes
            .entry(collection.to_string())
            .or_insert_with(|| Arc::new(InMemoryVectorIndex::new(similarity)))
            .clone()
    }

    /// Drops the index for a collection.
    pub fn drop_collection(&self, collection: &str) {
        self.indexes.remove(collection);
    }

    /// Returns the total number of vectors across all collections.
    pub fn total_vectors(&self) -> usize {
        self.indexes.iter().map(|kv| kv.value().points.len()).sum()
    }

    /// Returns a snapshot of the per-collection vector counts.
    pub fn per_collection_counts(&self) -> HashMap<String, usize> {
        self.indexes
            .iter()
            .map(|kv| (kv.key().clone(), kv.value().points.len()))
            .collect()
    }
}

impl Default for VectorIndexStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn in_memory_index_upsert_search_delete() {
        let idx = InMemoryVectorIndex::new(Similarity::Cosine);
        idx.upsert(VectorPoint::new(DocumentId::new("a"), vec![1.0, 0.0, 0.0]))
            .await
            .unwrap();
        idx.upsert(VectorPoint::new(DocumentId::new("b"), vec![0.0, 1.0, 0.0]))
            .await
            .unwrap();
        let hits = idx.search(&[1.0, 0.0, 0.0], 1).await.unwrap();
        assert_eq!(hits[0].0.as_str(), "a");
        assert!(idx.delete(&DocumentId::new("a")).await.unwrap());
        assert_eq!(idx.len().await.unwrap(), 1);
    }

    #[test]
    fn cosine_similarity_identical_is_one() {
        assert!((cosine_similarity(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_orthogonal_is_zero() {
        assert!((cosine_similarity(&[1.0, 0.0], &[0.0, 1.0])).abs() < 1e-6);
    }

    #[test]
    fn dot_product_works() {
        assert!((dot_product(&[1.0, 2.0, 3.0], &[4.0, 5.0, 6.0]) - 32.0).abs() < 1e-6);
    }

    #[test]
    fn euclidean_distance_works() {
        assert!((euclidean_distance(&[0.0, 0.0], &[3.0, 4.0]) - 5.0).abs() < 1e-6);
    }

    #[tokio::test]
    async fn store_creates_indices_per_collection() {
        let s = VectorIndexStore::new();
        let a = s.for_collection("col_a", Similarity::Cosine);
        let b = s.for_collection("col_b", Similarity::Cosine);
        a.upsert(VectorPoint::new(DocumentId::new("1"), vec![1.0, 0.0]))
            .await
            .unwrap();
        b.upsert(VectorPoint::new(DocumentId::new("1"), vec![0.0, 1.0]))
            .await
            .unwrap();
        assert_eq!(a.len().await.unwrap(), 1);
        assert_eq!(b.len().await.unwrap(), 1);
    }
}
