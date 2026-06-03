//! Search engine: BM25 ranking, query parsing, and full search pipeline.

pub mod bm25;
pub mod dto;
pub mod engine;
pub mod query;

pub use bm25::{DEFAULT_B, DEFAULT_K1, bm25_document_score, bm25_score, rank};
pub use dto::{SearchHit, SearchRequest, SearchResponse};
pub use engine::{MultiSearchResult, SearchEngine, apply_typo, expand_terms_via_synonyms};
pub use query::{Query, parse_query};
