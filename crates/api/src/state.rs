//! Shared application state accessible to every HTTP handler.

use std::sync::Arc;

use auth::AuthService;
use collections::CollectionStore;
use documents::DocumentService;
use indexing::IndexStore;
use search::SearchEngine;
use snapshots::SnapshotService;
use storage::StorageBackend;
use tasks::TaskQueue;
use vector::VectorIndexStore;

/// Global application state passed to every handler.
#[derive(Clone)]
pub struct AppState {
    /// Storage backend.
    pub storage: Arc<dyn StorageBackend>,
    /// Authentication service.
    pub auth: Arc<AuthService>,
    /// Collection metadata store.
    pub collections: Arc<CollectionStore>,
    /// Document service.
    pub documents: Arc<DocumentService>,
    /// In-memory inverted index store.
    pub indexes: Arc<IndexStore>,
    /// Search engine.
    pub search: Arc<SearchEngine>,
    /// Task queue.
    pub tasks: Arc<TaskQueue>,
    /// Snapshot service.
    pub snapshots: Arc<SnapshotService>,
    /// Vector index store.
    pub vectors: Arc<VectorIndexStore>,
    /// Configuration for the running instance.
    pub config: Arc<config_crate::AppConfig>,
}
