//! Sharding (skeleton, future-ready).
//!
//! A future implementation of AccelerateSearch will partition a collection
//! into one or more shards, each owned by a node. This crate defines the
//! public [`ShardRouter`] trait and the default single-shard
//! implementation. The single-node binary always uses the single-shard
//! router.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use errors::AppResult;
use models::DocumentId;

/// Description of a shard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shard {
    /// Shard identifier.
    pub id: String,
    /// Shard index (0-based) within its collection.
    pub index: u32,
    /// Total number of shards for the collection.
    pub total: u32,
}

/// Routes a document to the shard that should own it.
#[async_trait]
pub trait ShardRouter: Send + Sync + 'static {
    /// Returns the shard that should own `doc_id`.
    async fn route(&self, doc_id: &DocumentId) -> AppResult<Shard>;
    /// Returns the total number of shards.
    async fn shard_count(&self) -> AppResult<u32>;
}

/// Single-shard router: all documents map to shard 0 of 1.
pub struct SingleShardRouter;

#[async_trait]
impl ShardRouter for SingleShardRouter {
    async fn route(&self, _doc_id: &DocumentId) -> AppResult<Shard> {
        Ok(Shard {
            id: "shard-0".into(),
            index: 0,
            total: 1,
        })
    }
    async fn shard_count(&self) -> AppResult<u32> {
        Ok(1)
    }
}
