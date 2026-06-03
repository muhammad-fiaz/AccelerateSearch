//! Document indexing pipeline.

use std::sync::Arc;

use rayon::prelude::*;
use serde_json::Value;
use tracing::info;

use errors::{AppError, AppResult};
use models::{CollectionId, Document, DocumentId};
use storage::StorageBackend;

use crate::StopWords;
use crate::analyzer::{Analyzer, AnalyzerConfig};
use crate::index_store::IndexStore;
use crate::inverted_index::InvertedIndex;

/// Result of a single indexing call.
#[derive(Debug, Clone, Default)]
pub struct IndexingResult {
    /// Number of documents successfully indexed.
    pub indexed: usize,
    /// Number of documents that failed validation.
    pub failed: usize,
    /// Total tokens added to the inverted index.
    pub tokens: usize,
}

/// The indexing pipeline.
pub struct IndexingPipeline {
    store: Arc<IndexStore>,
    storage: Arc<dyn StorageBackend>,
}

impl IndexingPipeline {
    /// Creates a new indexing pipeline.
    #[must_use]
    pub fn new(store: Arc<IndexStore>, storage: Arc<dyn StorageBackend>) -> Self {
        Self { store, storage }
    }

    /// Indexes a batch of documents into `collection`.
    pub async fn index_batch(
        &self,
        collection: &CollectionId,
        primary_key: &str,
        searchable: &[String],
        separator: &str,
        stop_words: &[String],
        docs: Vec<Document>,
    ) -> AppResult<IndexingResult> {
        if docs.is_empty() {
            return Ok(IndexingResult::default());
        }
        // Validate primary key presence and uniqueness.
        let mut seen_ids = std::collections::HashSet::new();
        for d in &docs {
            let pk = d.get(primary_key).ok_or_else(|| {
                AppError::bad_request(format!("document missing primary key '{primary_key}'"))
            })?;
            let id = extract_doc_id(pk)?;
            if !seen_ids.insert(id.clone()) {
                return Err(AppError::bad_request(format!(
                    "duplicate primary key '{id}' in batch"
                )));
            }
        }

        let stop_words = StopWords::new(stop_words.iter().cloned());
        let analyzer = Analyzer::new(AnalyzerConfig {
            stop_words: Some(stop_words),
            ..AnalyzerConfig::default()
        });

        let searchable = searchable.to_vec();
        let separator = separator.to_string();

        let collection_for_task = collection.clone();
        let store = self.store.clone();
        let storage = self.storage.clone();

        // Process in parallel on the rayon pool, then apply mutations under
        // a write lock to the in-memory index.
        let primary_key_owned = primary_key.to_string();
        let analyzer = std::sync::Arc::new(analyzer);
        let processed: Vec<_> = tokio::task::spawn_blocking(move || {
            docs.par_iter()
                .map(|d| {
                    let pk = d.get(&primary_key_owned).expect("validated above");
                    let id = match extract_doc_id(pk) {
                        Ok(v) => v,
                        Err(_) => return None,
                    };
                    let mut field_tokens: std::collections::BTreeMap<
                        String,
                        Vec<crate::analyzer::Token>,
                    > = std::collections::BTreeMap::new();
                    for field in &searchable {
                        if let Some(value) = d.get(field) {
                            let text = value_to_text(value, &separator);
                            let toks = analyzer.analyze(&text);
                            field_tokens.insert(field.clone(), toks);
                        }
                    }
                    Some((id, field_tokens, d.clone()))
                })
                .filter_map(|x| x)
                .collect::<Vec<_>>()
        })
        .await
        .map_err(|e| AppError::Internal(format!("join: {e}")))?;

        let total_tokens: usize = processed
            .iter()
            .map(|(_, ft, _)| ft.values().map(|v| v.len()).sum::<usize>())
            .sum();

        let indexed_count = processed.len();
        let index_arc = store.get_or_load(&collection_for_task)?;
        // First pass: index all documents (no storage I/O while holding the lock).
        {
            let mut idx = index_arc.write();
            for (doc_id, field_tokens, _doc) in processed.iter() {
                idx.remove_document(doc_id);
                for (field, tokens) in field_tokens {
                    let token_count = tokens.len();
                    for tok in tokens {
                        idx.add_occurrence(&tok.term, field, doc_id);
                    }
                    idx.add_field_length(doc_id, field, token_count);
                }
            }
            idx.recompute_stats();
        }
        // Second pass: persist documents to storage (lock not held here).
        for (doc_id, _field_tokens, doc) in processed {
            let key = storage_key(&collection_for_task, &doc_id);
            let bytes = serde_json::to_vec(&doc)?;
            storage.put(storage::TABLE_DOCUMENTS, &key, bytes).await?;
        }
        store.persist(&collection_for_task).await?;
        info!(
            collection = %collection_for_task,
            indexed = indexed_count,
            tokens = total_tokens,
            "indexed batch"
        );
        Ok(IndexingResult {
            indexed: indexed_count,
            failed: 0,
            tokens: total_tokens,
        })
    }

    /// Removes a document from `collection`.
    pub async fn remove_document(
        &self,
        collection: &CollectionId,
        doc_id: &DocumentId,
    ) -> AppResult<()> {
        let key = storage_key(collection, doc_id);
        self.storage.delete(storage::TABLE_DOCUMENTS, &key).await?;
        let index_arc = self.store.get_or_load(collection)?;
        {
            let mut idx = index_arc.write();
            idx.remove_document(doc_id);
        }
        self.store.persist(collection).await?;
        Ok(())
    }

    /// Removes all documents from `collection`.
    pub async fn remove_all_documents(&self, collection: &CollectionId) -> AppResult<()> {
        let prefix = format!("{collection}\u{0}");
        let keys = self.storage.list(storage::TABLE_DOCUMENTS, &prefix).await?;
        for k in keys {
            self.storage.delete(storage::TABLE_DOCUMENTS, &k).await?;
        }
        let index_arc = self.store.get_or_load(collection)?;
        {
            let mut idx = index_arc.write();
            *idx = InvertedIndex::new();
        }
        self.store.persist(collection).await?;
        Ok(())
    }

    /// Returns a document by id, or `None`.
    pub async fn get_document(
        &self,
        collection: &CollectionId,
        doc_id: &DocumentId,
    ) -> AppResult<Option<Document>> {
        let key = storage_key(collection, doc_id);
        match self.storage.get(storage::TABLE_DOCUMENTS, &key).await? {
            Some(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            None => Ok(None),
        }
    }

    /// Returns all documents in a collection (paginated).
    pub async fn list_documents(
        &self,
        collection: &CollectionId,
        offset: usize,
        limit: usize,
    ) -> AppResult<Vec<Document>> {
        let prefix = format!("{collection}\u{0}");
        let mut keys = self.storage.list(storage::TABLE_DOCUMENTS, &prefix).await?;
        keys.sort();
        let mut out = Vec::with_capacity(limit.min(keys.len()));
        for key in keys.into_iter().skip(offset).take(limit) {
            if let Some(bytes) = self.storage.get(storage::TABLE_DOCUMENTS, &key).await? {
                out.push(serde_json::from_slice(&bytes)?);
            }
        }
        Ok(out)
    }
}

/// Returns the storage key for a document in a collection.
#[must_use]
pub fn storage_key(collection: &CollectionId, doc_id: &DocumentId) -> String {
    format!("{collection}\u{0}{doc_id}")
}

/// Extracts a document id from the JSON value of a primary key.
pub fn extract_doc_id(value: &Value) -> AppResult<DocumentId> {
    match value {
        Value::String(s) => {
            if s.is_empty() {
                Err(AppError::bad_request("primary key string is empty"))
            } else {
                Ok(DocumentId::new(s.clone()))
            }
        }
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(DocumentId::new(i.to_string()))
            } else if let Some(u) = n.as_u64() {
                Ok(DocumentId::new(u.to_string()))
            } else if let Some(f) = n.as_f64() {
                Ok(DocumentId::new(f.to_string()))
            } else {
                Err(AppError::bad_request("unsupported primary key number"))
            }
        }
        _ => Err(AppError::bad_request(
            "primary key must be string or number",
        )),
    }
}

/// Converts a JSON value into a searchable text representation.
pub fn value_to_text(value: &Value, separator: &str) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Array(arr) => arr
            .iter()
            .map(|v| value_to_text(v, separator))
            .collect::<Vec<_>>()
            .join(separator),
        Value::Object(map) => map
            .values()
            .map(|v| value_to_text(v, separator))
            .collect::<Vec<_>>()
            .join(separator),
        Value::Null => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::RedbStorage;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn index_and_retrieve_document() {
        let backend = Arc::new(RedbStorage::open_temp().unwrap());
        let store = Arc::new(IndexStore::new(backend.clone()));
        let pipeline = IndexingPipeline::new(store, backend);
        let col = CollectionId::new("products");
        let mut d = Document::new();
        d.insert("id".into(), serde_json::json!("1"));
        d.insert("title".into(), serde_json::json!("Hello world"));
        pipeline
            .index_batch(&col, "id", &["title".into()], " ", &[], vec![d])
            .await
            .unwrap();
        let got = pipeline
            .get_document(&col, &DocumentId::new("1"))
            .await
            .unwrap();
        assert!(got.is_some());
    }

    #[test]
    fn value_to_text_handles_arrays() {
        let v = serde_json::json!(["a", "b", "c"]);
        assert_eq!(value_to_text(&v, " "), "a b c");
        let v = serde_json::json!({"a": 1, "b": 2});
        assert!(value_to_text(&v, " ").contains('1'));
    }

    #[test]
    fn extract_doc_id_accepts_string_and_number() {
        assert_eq!(
            extract_doc_id(&serde_json::json!("abc")).unwrap().as_str(),
            "abc"
        );
        assert_eq!(
            extract_doc_id(&serde_json::json!(42)).unwrap().as_str(),
            "42"
        );
        assert!(extract_doc_id(&serde_json::json!(true)).is_err());
    }
}
