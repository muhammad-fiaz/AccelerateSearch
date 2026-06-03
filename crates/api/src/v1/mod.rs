//! Version 1 API routes.
//!
//! All `/api/v1/*` endpoints live here. When a v2 API is introduced,
//! create a `v2/` sibling module and update `lib.rs` accordingly.

pub mod collections;
pub mod documents;
pub mod embedders;
pub mod hooks;
pub mod indexes;
pub mod keys;
pub mod network;
pub mod rules;
pub mod search;
pub mod settings;
pub mod snapshots;
pub mod synonyms;
pub mod tasks;
pub mod tenant_tokens;

use actix_web::web;

/// Configures all v1 routes on the given scope.
pub fn configure_v1_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1")
            // Collections
            .service(collections::list_collections)
            .service(collections::create_collection)
            .service(collections::get_collection)
            .service(collections::update_collection)
            .service(collections::delete_collection)
            .service(collections::collection_stats)
            .service(collections::get_settings)
            .service(collections::update_settings)
            .service(collections::reset_settings)
            // Per-setting collection endpoints
            .service(settings::get_filterable_attributes)
            .service(settings::put_filterable_attributes)
            .service(settings::delete_filterable_attributes)
            .service(settings::get_sortable_attributes)
            .service(settings::put_sortable_attributes)
            .service(settings::delete_sortable_attributes)
            .service(settings::get_searchable_attributes)
            .service(settings::put_searchable_attributes)
            .service(settings::delete_searchable_attributes)
            .service(settings::get_displayed_attributes)
            .service(settings::put_displayed_attributes)
            .service(settings::delete_displayed_attributes)
            .service(settings::get_stop_words)
            .service(settings::put_stop_words)
            .service(settings::delete_stop_words)
            .service(settings::get_ranking_rules)
            .service(settings::put_ranking_rules)
            .service(settings::delete_ranking_rules)
            .service(settings::get_typo_tolerance)
            .service(settings::put_typo_tolerance)
            .service(settings::delete_typo_tolerance)
            .service(settings::get_distinct_field)
            .service(settings::put_distinct_field)
            .service(settings::delete_distinct_field)
            // Documents
            .service(documents::list_documents)
            .service(documents::add_documents)
            .service(documents::update_documents)
            .service(documents::get_document)
            .service(documents::delete_document)
            .service(documents::delete_all_documents)
            .service(documents::delete_batch)
            .service(documents::export_documents)
            // Search
            .service(search::search)
            .service(search::search_get)
            .service(search::multi_search)
            .service(search::autocomplete)
            // Tasks
            .service(tasks::list_tasks)
            .service(tasks::get_task)
            .service(tasks::cancel_all_tasks)
            .service(tasks::cancel_tasks)
            // Keys
            .service(keys::list_keys)
            .service(keys::create_key)
            .service(keys::get_key)
            .service(keys::patch_key)
            .service(keys::delete_key)
            // Tenant tokens
            .service(tenant_tokens::generate_tenant_token)
            // Snapshots
            .service(snapshots::create_snapshot)
            .service(snapshots::list_snapshots)
            .service(snapshots::get_snapshot)
            .service(snapshots::delete_snapshot)
            .service(snapshots::restore_snapshot)
            // Synonyms
            .service(synonyms::get_synonyms)
            .service(synonyms::put_synonyms)
            .service(synonyms::delete_synonyms)
            // Embedders
            .service(embedders::get_embedders)
            .service(embedders::patch_embedders)
            .service(embedders::delete_embedders)
            // Indexes (alias for collections)
            .service(indexes::list_indexes)
            .service(indexes::create_index)
            .service(indexes::get_index)
            .service(indexes::update_index)
            .service(indexes::delete_index)
            .service(indexes::index_stats)
            .service(indexes::swap_indexes)
            // Hooks / Webhooks
            .service(hooks::list_hooks)
            .service(hooks::get_hook)
            .service(hooks::create_hook)
            .service(hooks::patch_hook)
            .service(hooks::delete_hook)
            // Search rules
            .service(rules::get_rules)
            .service(rules::put_rules)
            .service(rules::delete_rules)
            // Network and experimental features
            .service(network::network_info)
            .service(network::get_experimental_features)
            .service(network::patch_experimental_features),
    );
}
