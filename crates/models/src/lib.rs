//! Shared domain models and DTOs for AccelerateSearch.
//!
//! This crate contains the strongly-typed wrappers used across all other
//! crates (collection/document/task/key identifiers), as well as the DTO
//! shapes that the REST API and storage layer exchange.

use std::collections::BTreeMap;
use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

// Strongly-typed identifiers

/// A collection (also referred to as "index") identifier.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord, ToSchema)]
#[serde(transparent)]
pub struct CollectionId(pub String);

impl CollectionId {
    /// Wraps a raw string as a collection identifier without validation.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the string slice of the identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CollectionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A document identifier. Documents use a user-chosen primary key (string or
/// integer) but we always store the string form internally.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord, ToSchema)]
#[serde(transparent)]
pub struct DocumentId(pub String);

impl DocumentId {
    /// Wraps a raw string as a document identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the string slice of the identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DocumentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A task identifier (UUID v7).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, ToSchema)]
#[serde(transparent)]
pub struct TaskId(pub Uuid);

impl TaskId {
    /// Generates a fresh task identifier.
    #[must_use]
    pub fn generate() -> Self {
        Self(Uuid::now_v7())
    }

    /// Wraps an existing UUID.
    #[must_use]
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Returns the underlying UUID.
    #[must_use]
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// An API key identifier.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, ToSchema)]
#[serde(transparent)]
pub struct ApiKeyId(pub Uuid);

impl ApiKeyId {
    /// Generates a fresh key identifier.
    #[must_use]
    pub fn generate() -> Self {
        Self(Uuid::now_v7())
    }

    /// Wraps an existing UUID.
    #[must_use]
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl fmt::Display for ApiKeyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A document is an arbitrary map of field names to JSON values. The primary
/// key field is identified by the collection's `primary_key` setting.
pub type Document = BTreeMap<String, serde_json::Value>;

/// A document exposed to the API. Wrapped in a newtype so `utoipa` can
/// describe it as a free-form OpenAPI object schema.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(value_type = Object)]
pub struct DocumentDto(pub Document);

/// Newtype wrapper for a vector embedding.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(transparent)]
pub struct Embedding(pub Vec<f32>);

impl Embedding {
    /// Creates a new embedding.
    #[must_use]
    pub fn new(values: Vec<f32>) -> Self {
        Self(values)
    }

    /// Returns the embedding's dimensions.
    #[must_use]
    pub fn dims(&self) -> usize {
        self.0.len()
    }

    /// Returns the underlying slice.
    #[must_use]
    pub fn as_slice(&self) -> &[f32] {
        &self.0
    }
}

// Permissions / Scopes

/// The set of recognised permission scopes for API keys.
///
/// The wildcard `*` grants all permissions.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    /// Add or update documents.
    #[serde(rename = "documents.add")]
    DocumentsAdd,
    /// Delete documents.
    #[serde(rename = "documents.delete")]
    DocumentsDelete,
    /// Read documents.
    #[serde(rename = "documents.get")]
    DocumentsGet,
    /// Search the collection.
    Search,
    /// Create a collection.
    #[serde(rename = "collections.create")]
    CollectionsCreate,
    /// Delete a collection.
    #[serde(rename = "collections.delete")]
    CollectionsDelete,
    /// Get collection metadata.
    #[serde(rename = "collections.get")]
    CollectionsGet,
    /// Update collection settings.
    #[serde(rename = "settings.update")]
    SettingsUpdate,
    /// Get collection settings.
    #[serde(rename = "settings.get")]
    SettingsGet,
    /// Create API keys.
    #[serde(rename = "keys.create")]
    KeysCreate,
    /// Delete API keys.
    #[serde(rename = "keys.delete")]
    KeysDelete,
    /// Get API key metadata.
    #[serde(rename = "keys.get")]
    KeysGet,
    /// Get task information.
    #[serde(rename = "tasks.get")]
    TasksGet,
    /// Cancel tasks.
    #[serde(rename = "tasks.cancel")]
    TasksCancel,
    /// Create snapshots.
    #[serde(rename = "snapshots.create")]
    SnapshotsCreate,
    /// Get snapshots.
    #[serde(rename = "snapshots.get")]
    SnapshotsGet,
    /// All permissions (admin).
    #[serde(rename = "*")]
    All,
}

impl Permission {
    /// Returns true if this permission is the admin wildcard.
    #[must_use]
    pub fn is_admin(&self) -> bool {
        matches!(self, Self::All)
    }
}

/// Status of an async task.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    /// Task is in the queue, waiting to be processed.
    Enqueued,
    /// Task is currently being processed.
    Processing,
    /// Task completed successfully.
    Succeeded,
    /// Task failed and has an error message.
    Failed,
    /// Task was cancelled before processing.
    Cancelled,
}

impl TaskStatus {
    /// Returns true if the task is in a terminal state.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }
}

/// The kind of work a task represents.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    /// Document ingestion (add or update).
    DocumentAdditionOrUpdate,
    /// Document deletion (single or batch).
    DocumentDeletion,
    /// Collection creation.
    CollectionCreation,
    /// Collection deletion.
    CollectionDeletion,
    /// Collection settings update.
    SettingsUpdate,
    /// Settings reset.
    SettingsReset,
    /// Snapshot creation.
    SnapshotCreation,
    /// Snapshot restoration.
    SnapshotRestoration,
}

/// Detailed task information returned by the API.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TaskInfo {
    /// Unique task identifier.
    pub uid: TaskId,
    /// Task status.
    pub status: TaskStatus,
    /// Task kind.
    #[serde(rename = "type")]
    pub kind: TaskKind,
    /// Time the task was enqueued (UTC).
    pub enqueued_at: DateTime<Utc>,
    /// Time the task started processing (UTC).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,
    /// Time the task finished (UTC).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,
    /// Duration of the task in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// UID of the collection the task operates on, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index_uid: Option<CollectionId>,
    /// Error message if the task failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<TaskError>,
    /// Number of documents affected (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub affected_documents: Option<u64>,
}

/// Error details for a failed task.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TaskError {
    /// Stable error code.
    pub code: String,
    /// Human-readable message.
    pub message: String,
}

/// Lightweight task information returned when a task is enqueued.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TaskResult {
    /// Unique task identifier.
    pub task_uid: TaskId,
    /// Task status (typically `enqueued`).
    pub status: TaskStatus,
    /// Task kind.
    #[serde(rename = "type")]
    pub kind: TaskKind,
    /// Time the task was enqueued (UTC).
    pub enqueued_at: DateTime<Utc>,
    /// UID of the collection the task operates on, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index_uid: Option<CollectionId>,
}

/// Persisted API key metadata.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ApiKey {
    /// Stable identifier for the key.
    pub uid: ApiKeyId,
    /// Human-friendly name.
    pub name: String,
    /// Optional description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// SHA-256 hash of the key string.
    pub key_hash: String,
    /// First few characters of the key, used for display only.
    pub key_prefix: String,
    /// Permissions granted to the key.
    pub actions: Vec<Permission>,
    /// Collections the key is scoped to. `None` means all collections.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indexes: Option<Vec<CollectionId>>,
    /// Expiry timestamp (UTC). `None` means never expires.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// Settings for a single collection.
#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct CollectionSettings {
    /// The document field that serves as the primary key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_key: Option<String>,
    /// Fields that are searchable.
    #[serde(default)]
    pub searchable_attributes: Vec<String>,
    /// Fields that are filterable.
    #[serde(default)]
    pub filterable_attributes: Vec<String>,
    /// Fields that can be used for sorting.
    #[serde(default)]
    pub sortable_attributes: Vec<String>,
    /// Ranking rules with associated weights.
    #[serde(default)]
    pub ranking_rules: Vec<String>,
    /// Stop words removed at index and query time.
    #[serde(default)]
    pub stop_words: Vec<String>,
    /// Synonyms for the collection.
    #[serde(default)]
    pub synonyms: BTreeMap<String, Vec<String>>,
    /// Typo tolerance configuration.
    #[serde(default)]
    pub typo_tolerance: TypoToleranceSettings,
    /// Embedder configuration for vector search.
    #[serde(default)]
    pub embedders: BTreeMap<String, EmbedderSettings>,
    /// Vector attribute definitions.
    #[serde(default)]
    pub vector_attributes: Vec<String>,
    /// Searchable field weights for boosting.
    #[serde(default)]
    pub field_weights: BTreeMap<String, f64>,
    /// Separator used for faceted searches.
    #[serde(default = "default_separator")]
    pub separator: String,
    /// Displayed attributes (used to project hits).
    #[serde(default)]
    pub displayed_attributes: Vec<String>,
    /// Distinct attribute (de-duplication).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distinct_field: Option<String>,
}

fn default_separator() -> String {
    " ".to_string()
}

/// Typo tolerance configuration.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TypoToleranceSettings {
    /// Whether typo tolerance is enabled.
    pub enabled: bool,
    /// Minimum word size before a single typo is allowed.
    pub min_word_size_for_one_typo: usize,
    /// Minimum word size before two typos are allowed.
    pub min_word_size_for_two_typos: usize,
    /// Attributes that have typo tolerance disabled.
    #[serde(default)]
    pub disable_on_attributes: Vec<String>,
    /// Attributes that have typo tolerance disabled for queries.
    #[serde(default)]
    pub disable_on_query: Vec<String>,
}

impl Default for TypoToleranceSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            min_word_size_for_one_typo: 5,
            min_word_size_for_two_typos: 9,
            disable_on_attributes: Vec::new(),
            disable_on_query: Vec::new(),
        }
    }
}

/// Configuration for an external embedder used to vectorise documents.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EmbedderSettings {
    /// Embedder provider (e.g. `"rest"`).
    pub provider: String,
    /// URL of the embedder endpoint.
    pub url: String,
    /// Optional API key for the embedder.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Model identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Embedding dimensions.
    pub dimensions: usize,
    /// Document template (must contain `{{text}}`).
    #[serde(default = "default_document_template")]
    pub document_template: String,
    /// Similarity metric.
    pub similarity: Similarity,
    /// Request timeout in milliseconds.
    #[serde(default = "default_embedder_timeout")]
    pub timeout_ms: u64,
}

fn default_document_template() -> String {
    "{{text}}".to_string()
}

fn default_embedder_timeout() -> u64 {
    30_000
}

/// A vector similarity metric.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Similarity {
    /// Cosine similarity (1.0 = identical, -1.0 = opposite).
    #[default]
    Cosine,
    /// Dot product.
    Dot,
    /// Euclidean distance (smaller = more similar).
    Euclidean,
}

/// Persisted collection metadata.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Collection {
    /// Collection identifier.
    pub uid: CollectionId,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
    /// Collection primary key.
    pub primary_key: String,
    /// Number of documents currently stored.
    pub documents_count: u64,
    /// Collection settings.
    pub settings: CollectionSettings,
}

/// Collection statistics.
#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct CollectionStats {
    /// Number of documents.
    pub number_of_documents: u64,
    /// Whether vector indexing is enabled.
    pub is_indexing: bool,
    /// Field distribution statistics.
    pub field_distribution: BTreeMap<String, FieldStats>,
}

/// Per-field statistics inside a collection.
#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct FieldStats {
    /// Number of documents containing this field.
    pub number_of_documents: u64,
}

/// Global statistics returned by `GET /stats`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct GlobalStats {
    /// Number of collections currently stored.
    pub number_of_collections: u64,
    /// Total number of documents across all collections.
    pub number_of_documents: u64,
    /// Whether an indexing task is currently running.
    pub is_indexing: bool,
}

/// Snapshot metadata persisted in storage.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SnapshotMeta {
    /// Snapshot name (unique).
    pub name: String,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// File size in bytes.
    pub size: u64,
    /// Path on disk.
    pub path: String,
}

/// Response of `GET /health`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Health {
    /// Status string: `"available"`.
    pub status: String,
}

/// Response of `GET /version`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VersionInfo {
    /// Package version.
    pub version: String,
    /// SHA of the commit (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_sha: Option<String>,
    /// Commit date (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_date: Option<String>,
}

/// A search ruleset scoped to a single collection.
///
/// Curated queries attach rules to a query pattern to pin, hide, filter,
/// or sort documents.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Ruleset {
    /// Collection this ruleset applies to.
    pub index_uid: CollectionId,
    /// List of rules.
    #[serde(default)]
    pub rules: Vec<Rule>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// A single search rule.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Rule {
    /// Stable id for this rule.
    pub id: Uuid,
    /// Human-friendly name.
    pub name: String,
    /// Whether the rule is active.
    #[serde(default = "default_rule_enabled")]
    pub enabled: bool,
    /// Query pattern this rule applies to (e.g. `"apple watch"`).
    pub query: String,
    /// Actions to take when the rule matches.
    pub actions: Vec<RuleAction>,
}

fn default_rule_enabled() -> bool {
    true
}

/// Action taken when a rule matches.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuleAction {
    /// Pin a document to a specific position.
    PinnedHit {
        /// The document id to pin.
        doc_id: String,
        /// 1-based position.
        position: usize,
    },
    /// Hide documents whose `_id` is in the list.
    HideHits {
        /// Document ids to hide.
        doc_ids: Vec<String>,
    },
    /// Replace/augment the user query with a different one.
    Query {
        /// The replacement query.
        query: String,
    },
    /// Force a specific filter expression.
    Filter {
        /// Filter expression (in AccelerateSearch DSL).
        filter: String,
    },
    /// Force a specific sort.
    Sort {
        /// Sort fields (`field:asc` / `field:desc`).
        sort: Vec<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collection_id_serializes_as_string() {
        let id = CollectionId::new("products");
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"products\"");
    }

    #[test]
    fn task_id_uses_v7_uuid() {
        let id = TaskId::generate();
        let s = id.to_string();
        assert_eq!(s.len(), 36);
    }

    #[test]
    fn task_status_terminal() {
        assert!(TaskStatus::Succeeded.is_terminal());
        assert!(TaskStatus::Failed.is_terminal());
        assert!(TaskStatus::Cancelled.is_terminal());
        assert!(!TaskStatus::Enqueued.is_terminal());
        assert!(!TaskStatus::Processing.is_terminal());
    }

    #[test]
    fn default_typo_tolerance_values() {
        let t = TypoToleranceSettings::default();
        assert!(t.enabled);
        assert_eq!(t.min_word_size_for_one_typo, 5);
        assert_eq!(t.min_word_size_for_two_typos, 9);
    }

    #[test]
    fn collection_settings_roundtrip() {
        let mut s = CollectionSettings::default();
        s.searchable_attributes.push("title".into());
        s.filterable_attributes.push("category".into());
        let json = serde_json::to_string(&s).unwrap();
        let back: CollectionSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.searchable_attributes, vec!["title".to_string()]);
        assert_eq!(back.filterable_attributes, vec!["category".to_string()]);
    }
}
