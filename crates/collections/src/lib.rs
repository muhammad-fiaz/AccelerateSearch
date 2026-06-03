//! Collection CRUD and settings management.

use std::collections::BTreeMap;
use std::sync::Arc;

use chrono::Utc;
use dashmap::DashMap;
use tracing::info;

use errors::{AppError, AppResult};
use models::{Collection, CollectionId, CollectionSettings, CollectionStats, FieldStats};
use storage::{StorageBackend, TABLE_COLLECTIONS, TABLE_SETTINGS, get_json, put_json};

/// In-memory store of collection metadata, backed by the storage layer.
pub struct CollectionStore {
    storage: Arc<dyn StorageBackend>,
    cache: DashMap<CollectionId, Collection>,
}

impl CollectionStore {
    /// Creates a new collection store.
    #[must_use]
    pub fn new(storage: Arc<dyn StorageBackend>) -> Self {
        Self {
            storage,
            cache: DashMap::new(),
        }
    }

    /// Loads all collections from storage into memory.
    pub async fn load_all(&self) -> AppResult<()> {
        let keys = self.storage.list(TABLE_COLLECTIONS, "").await?;
        for k in keys {
            if let Some(bytes) = self.storage.get(TABLE_COLLECTIONS, &k).await? {
                let c: Collection = serde_json::from_slice(&bytes)?;
                self.cache.insert(c.uid.clone(), c);
            }
        }
        Ok(())
    }

    /// Creates a new collection.
    pub async fn create(
        &self,
        uid: &CollectionId,
        primary_key: &str,
        settings: CollectionSettings,
    ) -> AppResult<Collection> {
        if self.cache.contains_key(uid) {
            return Err(AppError::Conflict(format!(
                "collection '{uid}' already exists"
            )));
        }
        let now = Utc::now();
        let c = Collection {
            uid: uid.clone(),
            created_at: now,
            updated_at: now,
            primary_key: primary_key.to_string(),
            documents_count: 0,
            settings,
        };
        self.persist(&c).await?;
        self.cache.insert(uid.clone(), c.clone());
        info!(collection = %uid, "created collection");
        Ok(c)
    }

    /// Returns a collection by id.
    #[must_use]
    pub fn get(&self, uid: &CollectionId) -> Option<Collection> {
        self.cache.get(uid).map(|c| c.clone())
    }

    /// Returns all collections currently known.
    #[must_use]
    pub fn list(&self) -> Vec<Collection> {
        self.cache.iter().map(|kv| kv.value().clone()).collect()
    }

    /// Updates the settings of an existing collection.
    pub async fn update_settings(
        &self,
        uid: &CollectionId,
        settings: CollectionSettings,
    ) -> AppResult<Collection> {
        let mut entry = self
            .cache
            .get_mut(uid)
            .ok_or_else(|| AppError::not_found(format!("collection '{uid}' not found")))?;
        entry.settings = settings;
        entry.updated_at = Utc::now();
        let snapshot = entry.clone();
        drop(entry);
        self.persist(&snapshot).await?;
        Ok(snapshot)
    }

    /// Resets the settings of a collection to the defaults.
    pub async fn reset_settings(&self, uid: &CollectionId) -> AppResult<Collection> {
        self.update_settings(uid, CollectionSettings::default())
            .await
    }

    /// Updates the documents_count for a collection.
    pub async fn set_documents_count(&self, uid: &CollectionId, count: u64) -> AppResult<()> {
        let mut entry = self
            .cache
            .get_mut(uid)
            .ok_or_else(|| AppError::not_found(format!("collection '{uid}' not found")))?;
        entry.documents_count = count;
        entry.updated_at = Utc::now();
        let snapshot = entry.clone();
        drop(entry);
        self.persist(&snapshot).await
    }

    /// Deletes a collection from the store and the underlying storage.
    pub async fn delete(&self, uid: &CollectionId) -> AppResult<bool> {
        let removed = self.cache.remove(uid).is_some();
        if removed {
            self.storage.delete(TABLE_COLLECTIONS, uid.as_str()).await?;
            self.storage.delete(TABLE_SETTINGS, uid.as_str()).await?;
            info!(collection = %uid, "deleted collection");
        }
        Ok(removed)
    }

    /// Computes statistics for a collection.
    #[must_use]
    pub fn stats(&self, uid: &CollectionId) -> Option<CollectionStats> {
        let c = self.get(uid)?;
        let field_distribution = c
            .settings
            .searchable_attributes
            .iter()
            .map(|f| {
                (
                    f.clone(),
                    FieldStats {
                        number_of_documents: c.documents_count,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        Some(CollectionStats {
            number_of_documents: c.documents_count,
            is_indexing: false,
            field_distribution,
        })
    }

    /// Persists a collection record to the underlying storage.
    async fn persist(&self, c: &Collection) -> AppResult<()> {
        put_json(self.storage.as_ref(), TABLE_COLLECTIONS, c.uid.as_str(), c).await
    }

    /// Returns a snapshot of the per-collection document counts.
    #[must_use]
    pub fn document_counts(&self) -> BTreeMap<CollectionId, u64> {
        self.cache
            .iter()
            .map(|kv| (kv.key().clone(), kv.value().documents_count))
            .collect()
    }

    /// Reloads the cache from storage. Used by tests.
    pub async fn reload(&self) -> AppResult<()> {
        self.cache.clear();
        self.load_all().await
    }
}

/// Helper for loading and storing collection settings as a separate blob.
pub async fn get_settings(
    storage: &dyn StorageBackend,
    uid: &CollectionId,
) -> AppResult<Option<CollectionSettings>> {
    get_json::<CollectionSettings>(storage, TABLE_SETTINGS, uid.as_str()).await
}

/// Helper for storing collection settings.
pub async fn put_settings(
    storage: &dyn StorageBackend,
    uid: &CollectionId,
    settings: &CollectionSettings,
) -> AppResult<()> {
    put_json(storage, TABLE_SETTINGS, uid.as_str(), settings).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::RedbStorage;

    #[tokio::test]
    async fn create_get_delete_round_trip() {
        let backend = Arc::new(RedbStorage::open_temp().unwrap());
        let store = CollectionStore::new(backend);
        let uid = CollectionId::new("products");
        let c = store
            .create(&uid, "id", CollectionSettings::default())
            .await
            .unwrap();
        assert_eq!(c.uid.as_str(), "products");
        assert!(store.get(&uid).is_some());
        assert!(store.delete(&uid).await.unwrap());
        assert!(store.get(&uid).is_none());
    }

    #[tokio::test]
    async fn duplicate_create_is_conflict() {
        let backend = Arc::new(RedbStorage::open_temp().unwrap());
        let store = CollectionStore::new(backend);
        let uid = CollectionId::new("dup");
        store
            .create(&uid, "id", CollectionSettings::default())
            .await
            .unwrap();
        let err = store
            .create(&uid, "id", CollectionSettings::default())
            .await
            .unwrap_err();
        assert_eq!(err.code(), "conflict");
    }

    #[tokio::test]
    async fn update_settings_changes_in_place() {
        let backend = Arc::new(RedbStorage::open_temp().unwrap());
        let store = CollectionStore::new(backend);
        let uid = CollectionId::new("c");
        store
            .create(&uid, "id", CollectionSettings::default())
            .await
            .unwrap();
        let mut s = CollectionSettings::default();
        s.searchable_attributes.push("title".into());
        store.update_settings(&uid, s).await.unwrap();
        let c = store.get(&uid).unwrap();
        assert!(
            c.settings
                .searchable_attributes
                .contains(&"title".to_string())
        );
    }
}
