//! OpenAPI specification for AccelerateSearch.

use utoipa::OpenApi;

use crate::system::*;
use crate::v1::collections::*;
use crate::v1::documents::*;
use crate::v1::embedders::*;
use crate::v1::hooks::*;
use crate::v1::indexes::*;
use crate::v1::keys::*;
use crate::v1::network::*;
use crate::v1::rules::*;
use crate::v1::search::*;
use crate::v1::settings::*;
use crate::v1::snapshots::*;
use crate::v1::synonyms::*;
use crate::v1::tasks::*;
use crate::v1::tenant_tokens::*;

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
        // Per-setting endpoints
        get_filterable_attributes,
        put_filterable_attributes,
        delete_filterable_attributes,
        get_sortable_attributes,
        put_sortable_attributes,
        delete_sortable_attributes,
        get_searchable_attributes,
        put_searchable_attributes,
        delete_searchable_attributes,
        get_displayed_attributes,
        put_displayed_attributes,
        delete_displayed_attributes,
        get_stop_words,
        put_stop_words,
        delete_stop_words,
        get_ranking_rules,
        put_ranking_rules,
        delete_ranking_rules,
        get_typo_tolerance,
        put_typo_tolerance,
        delete_typo_tolerance,
        get_distinct_field,
        put_distinct_field,
        delete_distinct_field,
        // Documents
        add_documents,
        update_documents,
        list_documents,
        get_document,
        delete_document,
        delete_all_documents,
        delete_batch,
        export_documents,
        // Search
        search,
        search_get,
        multi_search,
        autocomplete,
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
        // Tenant tokens
        generate_tenant_token,
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
        // Indexes (alias)
        list_indexes,
        create_index,
        get_index,
        update_index,
        delete_index,
        index_stats,
        swap_indexes,
        // Hooks
        list_hooks,
        get_hook,
        create_hook,
        patch_hook,
        delete_hook,
        // Rules
        get_rules,
        put_rules,
        delete_rules,
        // Network and experimental features
        network_info,
        get_experimental_features,
        patch_experimental_features,
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
        FilterableAttributes,
        SortableAttributes,
        SearchableAttributes,
        DisplayedAttributes,
        StopWordsPayload,
        RankingRules,
        DistinctField,
        TenantTokenRequest,
        TenantTokenResponse,
        SearchRule,
        Hook,
        HookPatch,
        models::Ruleset,
        models::Rule,
        models::RuleAction,
        RulesetSummary,
        IndexesResponse,
        CreateIndexRequest,
        SwapIndexesRequest,
        SwapEntry,
        NetworkInfo,
        RemoteNode,
        ExperimentalFeature,
        ExperimentalFeaturesResponse,
        ExperimentalFeaturesPatch,
        AutocompleteResponse,
        TermSuggestion,
        AutocompleteQuery,
    )),
    tags(
        (name = "system", description = "System endpoints"),
        (name = "collections", description = "Collection management"),
        (name = "documents", description = "Document operations"),
        (name = "search", description = "Search"),
        (name = "tasks", description = "Async task management"),
        (name = "keys", description = "API key management"),
        (name = "tenant-tokens", description = "Tenant token management"),
        (name = "snapshots", description = "Snapshot management"),
        (name = "synonyms", description = "Synonym management"),
        (name = "embedders", description = "Vector embedder management"),
        (name = "indexes", description = "Index (alias for collection) management"),
        (name = "hooks", description = "Webhook / hooks management"),
        (name = "rules", description = "Search rule management")
    )
)]
pub struct ApiDoc;
