//! Cluster topology (skeleton, future-ready).
//!
//! This crate defines the public traits and data structures that a future
//! distributed implementation of AccelerateSearch will implement. The
//! current single-node binary does not use any of these abstractions, but
//! the API is designed so that adding cluster support will not require
//! changes to other crates.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use errors::AppResult;
use models::CollectionId;

/// The role of a node in the cluster.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum NodeRole {
    /// Coordinator: receives requests, forwards to primaries.
    Coordinator,
    /// Primary: owns a shard and accepts writes.
    Primary,
    /// Replica: read-only copy of a primary's data.
    Replica,
}

/// Description of a single cluster node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    /// Stable node identifier.
    pub id: String,
    /// Network address of the node (host:port).
    pub address: String,
    /// Node role.
    pub role: NodeRole,
    /// Time the node joined the cluster.
    pub joined_at: DateTime<Utc>,
}

/// Description of a shard (a partition of a collection).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardInfo {
    /// Shard identifier.
    pub id: String,
    /// Collection this shard belongs to.
    pub collection: CollectionId,
    /// Node that owns the primary copy of the shard.
    pub primary: String,
    /// Nodes that hold replica copies.
    pub replicas: Vec<String>,
}

/// Cluster-wide state snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClusterState {
    /// All known nodes.
    pub nodes: Vec<NodeInfo>,
    /// All known shards.
    pub shards: Vec<ShardInfo>,
}

/// Coordinator trait used by the API to dispatch work to the right node.
#[async_trait]
pub trait ClusterCoordinator: Send + Sync + 'static {
    /// Returns the cluster state.
    async fn state(&self) -> AppResult<ClusterState>;
    /// Returns the node that owns `collection`.
    async fn owner(&self, collection: &CollectionId) -> AppResult<Option<NodeInfo>>;
}

/// No-op coordinator used in single-node deployments.
pub struct SingleNodeCoordinator {
    local: NodeInfo,
}

impl SingleNodeCoordinator {
    /// Creates a new single-node coordinator.
    #[must_use]
    pub fn new(id: impl Into<String>, address: impl Into<String>) -> Self {
        Self {
            local: NodeInfo {
                id: id.into(),
                address: address.into(),
                role: NodeRole::Primary,
                joined_at: Utc::now(),
            },
        }
    }
}

#[async_trait]
impl ClusterCoordinator for SingleNodeCoordinator {
    async fn state(&self) -> AppResult<ClusterState> {
        Ok(ClusterState {
            nodes: vec![self.local.clone()],
            shards: Vec::new(),
        })
    }

    async fn owner(&self, _collection: &CollectionId) -> AppResult<Option<NodeInfo>> {
        Ok(Some(self.local.clone()))
    }
}
