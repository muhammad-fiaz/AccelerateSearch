//! In-memory index store, one per collection.

use std::collections::BTreeMap;
use std::sync::Arc;

use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tracing::debug;

use errors::AppResult;
use models::CollectionId;
use storage::{StorageBackend, get_json, put_json};

use crate::inverted_index::InvertedIndex;

/// Table used to persist inverted indexes.
pub const TABLE_INDEX: &str = "inverted_index";

/// Persistence record for a single collection's index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexRecord {
    /// Collection id.
    pub collection: CollectionId,
    /// Serialised [`InvertedIndex`].
    pub index: serde_json::Value,
    /// Last update timestamp (ISO-8601).
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// In-memory index store, keyed by collection UID.
pub struct IndexStore {
    storage: Arc<dyn StorageBackend>,
    in_memory: DashMap<CollectionId, Arc<RwLock<InvertedIndex>>>,
}

impl IndexStore {
    /// Creates a new `IndexStore` backed by the given storage.
    #[must_use]
    pub fn new(storage: Arc<dyn StorageBackend>) -> Self {
        Self {
            storage,
            in_memory: DashMap::new(),
        }
    }

    /// Returns the underlying storage backend.
    #[must_use]
    pub fn storage(&self) -> Arc<dyn StorageBackend> {
        self.storage.clone()
    }

    /// Returns the in-memory index for `collection`, loading it from storage
    /// if necessary.
    pub fn get_or_load(&self, collection: &CollectionId) -> AppResult<Arc<RwLock<InvertedIndex>>> {
        if let Some(idx) = self.in_memory.get(collection) {
            return Ok(idx.clone());
        }
        let loaded = self.load_from_storage(collection)?;
        let arc = Arc::new(RwLock::new(loaded));
        self.in_memory.insert(collection.clone(), arc.clone());
        Ok(arc)
    }

    /// Returns a fresh, empty in-memory index for the given collection,
    /// registering it in the in-memory map.
    pub fn fresh(&self, collection: &CollectionId) -> Arc<RwLock<InvertedIndex>> {
        self.in_memory
            .entry(collection.clone())
            .or_insert_with(|| Arc::new(RwLock::new(InvertedIndex::new())))
            .clone()
    }

    /// Persists the in-memory index of `collection` to storage.
    pub async fn persist(&self, collection: &CollectionId) -> AppResult<()> {
        let arc = self
            .in_memory
            .get(collection)
            .map(|i| i.clone())
            .ok_or_else(|| {
                errors::AppError::not_found(format!("no in-memory index for {collection}"))
            })?;
        let snapshot = {
            let guard = arc.read();
            serde_json::to_value(&*guard)?
        };
        let record = IndexRecord {
            collection: collection.clone(),
            index: snapshot,
            updated_at: chrono::Utc::now(),
        };
        put_json(
            self.storage.as_ref(),
            TABLE_INDEX,
            collection.as_str(),
            &record,
        )
        .await?;
        debug!(collection = %collection, "persisted inverted index");
        Ok(())
    }

    /// Removes the in-memory index for `collection` and the persisted
    /// snapshot.
    pub async fn drop(&self, collection: &CollectionId) -> AppResult<()> {
        self.in_memory.remove(collection);
        self.storage
            .delete(TABLE_INDEX, collection.as_str())
            .await?;
        Ok(())
    }

    /// Returns the list of collections currently held in memory.
    #[must_use]
    pub fn cached_collections(&self) -> Vec<CollectionId> {
        self.in_memory.iter().map(|kv| kv.key().clone()).collect()
    }

    fn load_from_storage(&self, collection: &CollectionId) -> AppResult<InvertedIndex> {
        // We use a blocking read because loading happens on demand. The
        // storage backend is fast and the lock is held only briefly.
        if let Some(record) = self.try_load_blocking(collection)? {
            let index: InvertedIndex = serde_json::from_value(record.index)?;
            return Ok(index);
        }
        Ok(InvertedIndex::new())
    }

    fn try_load_blocking(&self, collection: &CollectionId) -> AppResult<Option<IndexRecord>> {
        let storage = self.storage.clone();
        let key = collection.as_str().to_string();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                get_json::<IndexRecord>(storage.as_ref(), TABLE_INDEX, &key).await
            })
        })
    }
    /// Persists all currently cached in-memory indexes.
    pub async fn persist_all(&self) -> AppResult<()> {
        let keys: Vec<CollectionId> = self.cached_collections();
        for k in keys {
            self.persist(&k).await?;
        }
        Ok(())
    }

    /// Returns the total number of documents across all cached indexes.
    #[must_use]
    pub fn cached_doc_count(&self) -> u64 {
        self.in_memory
            .iter()
            .map(|kv| kv.value().read().stats.doc_count)
            .sum()
    }

    /// Returns a per-collection document count map.
    #[must_use]
    pub fn per_collection_doc_count(&self) -> BTreeMap<CollectionId, u64> {
        self.in_memory
            .iter()
            .map(|kv| (kv.key().clone(), kv.value().read().stats.doc_count))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::RedbStorage;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn fresh_and_persist_round_trip() {
        let backend = Arc::new(RedbStorage::open_temp().unwrap());
        let store = IndexStore::new(backend);
        let col = CollectionId::new("products");
        let idx = store.fresh(&col);
        idx.write()
            .add_occurrence("hello", "title", &models::DocumentId::new("1"));
        idx.write()
            .add_field_length(&models::DocumentId::new("1"), "title", 1);
        idx.write().recompute_stats();
        store.persist(&col).await.unwrap();
        // Drop and reload.
        store.in_memory.remove(&col);
        let reloaded = store.get_or_load(&col).unwrap();
        let guard = reloaded.read();
        assert!(guard.terms.contains_key("hello"));
    }
}
