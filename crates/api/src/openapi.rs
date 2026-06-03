//! OpenAPI specification for AccelerateSearch.

use utoipa::OpenApi;

use crate::system::*;
use crate::v1::collections::*;
use crate::v1::documents::*;
use crate::v1::embedders::*;
use crate::v1::keys::*;
use crate::v1::search::*;
use crate::v1::snapshots::*;
use crate::v1::synonyms::*;
use crate::v1::tasks::*;

/// Top-level OpenAPI documentation.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "AccelerateSearch",
        version = "0.0.0",
        description = "AccelerateSearch is a production-grade, modern, open-source self-hosted search engine written in Rust.",
        contact(
            name = "Muhammad Fiaz",
            email = "contact@muhammadfiaz.com"
        ),
        license(
            name = "Apache-2.0",
            url = "https://www.apache.org/licenses/LICENSE-2.0"
        )
    ),
    paths(
        health,
        version,
        stats,
        metrics,
        instance_id,
        // Collections
        list_collections,
        create_collection,
        get_collection,
        update_collection,
        delete_collection,
        collection_stats,
        get_settings,
        update_settings,
        reset_settings,
        // Documents
        add_documents,
        update_documents,
        list_documents,
        get_document,
        delete_document,
        delete_all_documents,
        delete_batch,
        // Search
        search,
        search_get,
        multi_search,
        // Tasks
        list_tasks,
        get_task,
        cancel_all_tasks,
        cancel_tasks,
        // Keys
        list_keys,
        create_key,
        get_key,
        patch_key,
        delete_key,
        // Snapshots
        create_snapshot,
        list_snapshots,
        get_snapshot,
        delete_snapshot,
        restore_snapshot,
        // Synonyms
        get_synonyms,
        put_synonyms,
        delete_synonyms,
        // Embedders
        get_embedders,
        patch_embedders,
        delete_embedders,
    ),
    components(schemas(
        models::Health,
        models::VersionInfo,
        models::GlobalStats,
        models::Collection,
        models::CollectionSettings,
        models::CollectionStats,
        models::TaskInfo,
        models::TaskResult,
        models::ApiKey,
        models::SnapshotMeta,
        models::TypoToleranceSettings,
        models::EmbedderSettings,
        models::Similarity,
        models::Permission,
        models::TaskStatus,
        models::TaskKind,
        ::search::dto::SearchRequest,
        ::search::dto::SearchResponse,
        ::search::dto::SearchHit,
        ::search::dto::HybridConfig,
        CreateCollectionRequest,
        DocumentsBody,
        BatchDeleteRequest,
        PaginationQuery,
        CreateKeyRequest,
        CreateKeyResponse,
        PatchKeyRequest,
        KeysResponse,
        TasksResponse,
        CancelTasksRequest,
        MultiSearchRequest,
        MultiSearchResponse,
        MultiSearchResultEntry,
        MultiSearchQuery,
        GetSearchQuery,
        snapshots::SnapshotSummary,
        errors::ErrorBody,
        InstanceId,
        OkResponse,
    )),
    tags(
        (name = "system", description = "System endpoints"),
        (name = "collections", description = "Collection management"),
        (name = "documents", description = "Document operations"),
        (name = "search", description = "Search"),
        (name = "tasks", description = "Async task management"),
        (name = "keys", description = "API key management"),
        (name = "snapshots", description = "Snapshot management"),
        (name = "synonyms", description = "Synonym management"),
        (name = "embedders", description = "Vector embedder management")
    )
)]
pub struct ApiDoc;
