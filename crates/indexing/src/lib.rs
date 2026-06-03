//! Indexing crate: tokenisation, normalisation, inverted index, and the
//! indexing pipeline.

#![deny(missing_docs)]

pub mod analyzer;
pub mod index_store;
pub mod inverted_index;
pub mod pipeline;

pub use analyzer::{Analyzer, AnalyzerConfig, StopWords, Token};
pub use index_store::{IndexRecord, IndexStore, TABLE_INDEX};
pub use inverted_index::{CollectionStats, FieldLengths, InvertedIndex, Posting, TermInfo};
pub use pipeline::{IndexingPipeline, IndexingResult, extract_doc_id, storage_key, value_to_text};
