//! Replication (skeleton, future-ready).
//!
//! A future implementation of AccelerateSearch will replicate primary
//! writes to one or more secondary nodes using a write-ahead log
//! protocol. This crate defines the public traits for the replicator and
//! the log record format, but the single-node binary does not invoke any
//! of this code.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use errors::AppResult;
use models::{CollectionId, DocumentId};

/// A single replicated log record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplRecord {
    /// Monotonic sequence number assigned by the primary.
    pub sequence: u64,
    /// Collection the record belongs to.
    pub collection: CollectionId,
    /// Operation kind.
    pub op: ReplOp,
    /// Time the record was generated.
    pub timestamp: DateTime<Utc>,
}

/// Kinds of replicated operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReplOp {
    /// Insert or update a document.
    Put {
        /// Document id.
        id: DocumentId,
        /// Serialised document.
        document: serde_json::Value,
    },
    /// Delete a document.
    Delete {
        /// Document id.
        id: DocumentId,
    },
}

/// Trait implemented by the primary-side replicator.
#[async_trait]
pub trait Replicator: Send + Sync + 'static {
    /// Appends a record to the replication log.
    async fn append(&self, record: ReplRecord) -> AppResult<()>;
    /// Returns the current high-water mark (the sequence number of the
    /// last appended record).
    async fn high_water_mark(&self) -> AppResult<u64>;
}

/// No-op replicator used in single-node deployments.
pub struct NoopReplicator;

#[async_trait]
impl Replicator for NoopReplicator {
    async fn append(&self, _record: ReplRecord) -> AppResult<()> {
        Ok(())
    }
    async fn high_water_mark(&self) -> AppResult<u64> {
        Ok(0)
    }
}
