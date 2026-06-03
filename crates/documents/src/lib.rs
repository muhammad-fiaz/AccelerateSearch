//! Document service: bridges between the API layer and the indexing
//! pipeline.

use std::sync::Arc;

use tracing::info;

use errors::{AppError, AppResult};
use indexing::IndexingPipeline;
use models::{Collection, CollectionId, CollectionSettings, Document, DocumentId};

use collections::CollectionStore;

/// High-level document service.
pub struct DocumentService {
    collections: Arc<CollectionStore>,
    pipeline: Arc<IndexingPipeline>,
}

impl DocumentService {
    /// Creates a new service.
    #[must_use]
    pub fn new(collections: Arc<CollectionStore>, pipeline: Arc<IndexingPipeline>) -> Self {
        Self {
            collections,
            pipeline,
        }
    }

    /// Adds or replaces documents in `collection`.
    pub async fn add_or_replace(
        &self,
        collection: &CollectionId,
        documents: Vec<Document>,
    ) -> AppResult<usize> {
        let c = self.require_collection(collection)?;
        self.run_index(&c, documents).await
    }

    /// Adds or updates documents in `collection` (partial update by primary
    /// key).
    pub async fn add_or_update(
        &self,
        collection: &CollectionId,
        documents: Vec<Document>,
    ) -> AppResult<usize> {
        let c = self.require_collection(collection)?;
        self.run_index(&c, documents).await
    }

    /// Returns a single document by id.
    pub async fn get(
        &self,
        collection: &CollectionId,
        doc_id: &DocumentId,
    ) -> AppResult<Option<Document>> {
        self.pipeline.get_document(collection, doc_id).await
    }

    /// Returns a paginated list of documents.
    pub async fn list(
        &self,
        collection: &CollectionId,
        offset: usize,
        limit: usize,
    ) -> AppResult<Vec<Document>> {
        self.pipeline
            .list_documents(collection, offset, limit)
            .await
    }

    /// Deletes a single document by id.
    pub async fn delete(&self, collection: &CollectionId, doc_id: &DocumentId) -> AppResult<bool> {
        self.pipeline.remove_document(collection, doc_id).await?;
        self.update_count(collection).await?;
        Ok(true)
    }

    /// Deletes multiple documents by id.
    pub async fn delete_many(
        &self,
        collection: &CollectionId,
        doc_ids: &[DocumentId],
    ) -> AppResult<usize> {
        let mut removed = 0;
        for id in doc_ids {
            self.pipeline.remove_document(collection, id).await?;
            removed += 1;
        }
        self.update_count(collection).await?;
        Ok(removed)
    }

    /// Deletes all documents in `collection`.
    pub async fn delete_all(&self, collection: &CollectionId) -> AppResult<()> {
        self.pipeline.remove_all_documents(collection).await?;
        self.update_count(collection).await?;
        info!(collection = %collection, "deleted all documents");
        Ok(())
    }

    async fn run_index(
        &self,
        collection: &Collection,
        documents: Vec<Document>,
    ) -> AppResult<usize> {
        if documents.is_empty() {
            return Ok(0);
        }
        let CollectionSettings {
            searchable_attributes,
            ..
        } = &collection.settings;
        let searchable = if searchable_attributes.is_empty() {
            // Fall back to indexing all scalar fields present in the first
            // document.
            documents
                .first()
                .map(|d| d.keys().cloned().collect::<Vec<_>>())
                .unwrap_or_default()
        } else {
            searchable_attributes.clone()
        };
        let result = self
            .pipeline
            .index_batch(
                &collection.uid,
                &collection.primary_key,
                &searchable,
                &collection.settings.separator,
                &collection.settings.stop_words,
                documents,
            )
            .await?;
        self.update_count(&collection.uid).await?;
        Ok(result.indexed)
    }

    async fn update_count(&self, collection: &CollectionId) -> AppResult<()> {
        // Recompute the collection's documents_count by listing all stored
        // documents. For large collections a separate counter table would be
        // preferable; this keeps the implementation self-contained.
        let count = self
            .pipeline
            .list_documents(collection, 0, usize::MAX)
            .await?
            .len() as u64;
        self.collections
            .set_documents_count(collection, count)
            .await?;
        Ok(())
    }

    fn require_collection(&self, uid: &CollectionId) -> AppResult<Collection> {
        self.collections
            .get(uid)
            .ok_or_else(|| AppError::not_found(format!("collection '{uid}' not found")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use collections::CollectionStore;
    use indexing::{IndexStore, IndexingPipeline};
    use models::CollectionSettings;
    use storage::RedbStorage;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn add_get_delete_document() {
        let backend = Arc::new(RedbStorage::open_temp().unwrap());
        let collections = Arc::new(CollectionStore::new(backend.clone()));
        let store = Arc::new(IndexStore::new(backend.clone()));
        let pipeline = Arc::new(IndexingPipeline::new(store, backend));
        let svc = DocumentService::new(collections.clone(), pipeline);
        let uid = CollectionId::new("c");
        collections
            .create(&uid, "id", CollectionSettings::default())
            .await
            .unwrap();
        let mut d = Document::new();
        d.insert("id".into(), serde_json::json!("1"));
        d.insert("title".into(), serde_json::json!("hello"));
        svc.add_or_replace(&uid, vec![d]).await.unwrap();
        let got = svc.get(&uid, &DocumentId::new("1")).await.unwrap();
        assert!(got.is_some());
        svc.delete(&uid, &DocumentId::new("1")).await.unwrap();
        let got = svc.get(&uid, &DocumentId::new("1")).await.unwrap();
        assert!(got.is_none());
    }
}
