//! Version 1 API routes.
//!
//! All `/api/v1/*` endpoints live here. When a v2 API is introduced,
//! create a `v2/` sibling module and update `lib.rs` accordingly.

pub mod collections;
pub mod documents;
pub mod embedders;
pub mod keys;
pub mod search;
pub mod snapshots;
pub mod synonyms;
pub mod tasks;

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
            // Documents
            .service(documents::list_documents)
            .service(documents::add_documents)
            .service(documents::update_documents)
            .service(documents::get_document)
            .service(documents::delete_document)
            .service(documents::delete_all_documents)
            .service(documents::delete_batch)
            // Search
            .service(search::search)
            .service(search::search_get)
            .service(search::multi_search)
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
            .service(embedders::delete_embedders),
    );
}
