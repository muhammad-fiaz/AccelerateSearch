//! Pluggable storage layer for AccelerateSearch.
//!
//! Provides a [`StorageBackend`] trait that abstracts over the persistence
//! engine, and a [`RedbStorage`] implementation that uses the embedded
//! [`redb`](https://docs.rs/redb) key-value store.
//!
//! All keys are stored as UTF-8 strings; values are stored as raw bytes
//! (typically the JSON encoding of a model). Tables are named strings.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::Mutex;
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use tracing::info;

use errors::{AppError, AppResult};
use utils::ensure_dir;

/// Table used to persist collection metadata (key: collection uid, value:
/// JSON-encoded `Collection`).
pub const TABLE_COLLECTIONS: &str = "collections";

/// Table used to persist documents. Key: `{collection_uid}\x00{document_id}`.
pub const TABLE_DOCUMENTS: &str = "documents";

/// Table used to persist async tasks. Key: task uid (string).
pub const TABLE_TASKS: &str = "tasks";

/// Table used to persist API keys. Key: key uid (string).
pub const TABLE_KEYS: &str = "keys";

/// Table used to persist snapshot metadata. Key: snapshot name.
pub const TABLE_SNAPSHOTS: &str = "snapshots";

/// Table used to persist collection settings. Key: collection uid.
pub const TABLE_SETTINGS: &str = "settings";

/// Table used to persist synonyms. Key: collection uid.
pub const TABLE_SYNONYMS: &str = "synonyms";

/// Table used to persist per-document inverted index postings. Key:
/// `{collection_uid}\x00{term}` -> JSON-encoded `Vec<DocumentId>`.
pub const TABLE_POSTINGS: &str = "postings";

/// Table used to persist the inverted index term dictionary per collection.
/// Key: `{collection_uid}\x00{term}` -> JSON-encoded `TermInfo` (doc_freq,
/// total_term_freq, ...).
pub const TABLE_TERMS: &str = "terms";

/// Table used to persist document field lengths for BM25. Key:
/// `{collection_uid}\x00{document_id}` -> JSON-encoded `BTreeMap<field, len>`.
pub const TABLE_FIELD_LENGTHS: &str = "field_lengths";

/// Table used to persist collection-level stats. Key: `{collection_uid}\x00{stat_name}`.
pub const TABLE_COLLECTION_STATS: &str = "collection_stats";

/// Table used to persist per-collection document vectors. Key:
/// `{collection_uid}\x00{document_id}` -> JSON-encoded `Vec<f32>`.
pub const TABLE_VECTORS: &str = "vectors";

/// Inverted index table (per-collection JSON blob).
pub const TABLE_INDEX: &str = "inverted_index";

/// Macro to define a [`TableDefinition`].
macro_rules! table_def {
    ($name:expr) => {
        TableDefinition::<&str, &[u8]>::new($name)
    };
}

/// Document-id key prefix separator.
pub const KEY_SEP: char = '\u{0}';

/// The trait every storage backend must implement.
#[async_trait]
pub trait StorageBackend: Send + Sync + 'static {
    /// Puts a value into the given table.
    async fn put(&self, table: &str, key: &str, value: Vec<u8>) -> AppResult<()>;

    /// Gets a value from the given table.
    async fn get(&self, table: &str, key: &str) -> AppResult<Option<Vec<u8>>>;

    /// Deletes a value from the given table.
    async fn delete(&self, table: &str, key: &str) -> AppResult<bool>;

    /// Returns all keys (with an optional prefix) in the given table.
    async fn list(&self, table: &str, prefix: &str) -> AppResult<Vec<String>>;

    /// Returns the count of keys in the given table (with optional prefix).
    async fn count(&self, table: &str, prefix: &str) -> AppResult<u64>;

    /// Performs a flush of any pending writes to disk.
    async fn flush(&self) -> AppResult<()>;

    /// Compacts / optimises the storage. May be a no-op.
    async fn compact(&self) -> AppResult<()> {
        Ok(())
    }
}

/// Helper that converts a typed model to JSON and writes it to a table.
pub async fn put_json<T: serde::Serialize>(
    backend: &dyn StorageBackend,
    table: &str,
    key: &str,
    value: &T,
) -> AppResult<()> {
    let bytes = serde_json::to_vec(value)?;
    backend.put(table, key, bytes).await
}

/// Helper that reads a typed model from a table.
pub async fn get_json<T: serde::de::DeserializeOwned>(
    backend: &dyn StorageBackend,
    table: &str,
    key: &str,
) -> AppResult<Option<T>> {
    match backend.get(table, key).await? {
        Some(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
        None => Ok(None),
    }
}

/// Builds a compound key from its parts, separated by a NUL byte.
#[must_use]
pub fn compound_key(parts: &[&str]) -> String {
    parts.join(&KEY_SEP.to_string())
}

/// Extracts the document id from a compound key `{collection}\u{0}{doc}`.
#[must_use]
pub fn split_doc_key(key: &str) -> Option<(String, String)> {
    let mut split = key.splitn(2, KEY_SEP);
    let col = split.next()?.to_string();
    let doc = split.next()?.to_string();
    Some((col, doc))
}

/// `redb`-backed implementation of [`StorageBackend`].
pub struct RedbStorage {
    db: Arc<Database>,
    /// `redb` itself is blocking; serialise all DB access through this lock
    /// to keep our async API consistent.
    lock: Arc<Mutex<()>>,
}

impl RedbStorage {
    /// Opens (or creates) a `redb` database at the given path.
    ///
    /// # Errors
    /// Returns an error if the path is not usable.
    pub fn open(path: impl AsRef<Path>) -> AppResult<Self> {
        let p = path.as_ref();
        if let Some(parent) = p.parent() {
            ensure_dir(parent)?;
        }
        let db = Database::create(p).map_err(|e| AppError::StorageError(e.to_string()))?;
        Self::init_tables(&db)?;
        info!(path = %p.display(), "opened redb storage");
        Ok(Self {
            db: Arc::new(db),
            lock: Arc::new(Mutex::new(())),
        })
    }

    /// Opens a temporary database. Used by tests and by other crates for
    /// quick local experiments.
    pub fn open_temp() -> AppResult<Self> {
        let tmp = tempfile::tempdir().map_err(|e| AppError::Internal(e.to_string()))?;
        let path = tmp.path().join("accelerate.redb");
        let s = Self::open(&path)?;
        // Leak the tempdir so it lives for the duration of the test process.
        std::mem::forget(tmp);
        Ok(s)
    }

    fn init_tables(db: &Database) -> AppResult<()> {
        let txn = db.begin_write()?;
        {
            let _ = txn.open_table(table_def!(TABLE_COLLECTIONS))?;
            let _ = txn.open_table(table_def!(TABLE_DOCUMENTS))?;
            let _ = txn.open_table(table_def!(TABLE_TASKS))?;
            let _ = txn.open_table(table_def!(TABLE_KEYS))?;
            let _ = txn.open_table(table_def!(TABLE_SNAPSHOTS))?;
            let _ = txn.open_table(table_def!(TABLE_SETTINGS))?;
            let _ = txn.open_table(table_def!(TABLE_SYNONYMS))?;
            let _ = txn.open_table(table_def!(TABLE_POSTINGS))?;
            let _ = txn.open_table(table_def!(TABLE_TERMS))?;
            let _ = txn.open_table(table_def!(TABLE_FIELD_LENGTHS))?;
            let _ = txn.open_table(table_def!(TABLE_COLLECTION_STATS))?;
            let _ = txn.open_table(table_def!(TABLE_VECTORS))?;
            let _ = txn.open_table(table_def!(TABLE_INDEX))?;
        }
        txn.commit()?;
        Ok(())
    }

    /// Returns the underlying `redb` database. Used by internal code that
    /// needs to perform complex multi-table transactions.
    pub fn database(&self) -> Arc<Database> {
        self.db.clone()
    }

    /// Executes a function inside a write transaction.
    ///
    /// # Errors
    /// Returns the underlying `redb` error.
    pub fn write<F, R>(&self, f: F) -> AppResult<R>
    where
        F: FnOnce(&mut redb::WriteTransaction) -> AppResult<R>,
    {
        let _guard = self.lock.lock();
        let txn = self.db.begin_write()?;
        let mut txn = txn;
        let result = f(&mut txn)?;
        txn.commit()?;
        Ok(result)
    }

    /// Executes a function inside a read transaction.
    pub fn read<F, R>(&self, f: F) -> AppResult<R>
    where
        F: FnOnce(&redb::ReadTransaction) -> AppResult<R>,
    {
        let _guard = self.lock.lock();
        let txn = self.db.begin_read()?;
        f(&txn)
    }
}

#[async_trait]
impl StorageBackend for RedbStorage {
    async fn put(&self, table: &str, key: &str, value: Vec<u8>) -> AppResult<()> {
        let db = self.db.clone();
        let lock = self.lock.clone();
        let key = key.to_string();
        let table = table.to_string();
        tokio::task::spawn_blocking(move || -> AppResult<()> {
            let _g = lock.lock();
            let txn = db.begin_write()?;
            {
                let def: TableDefinition<&str, &[u8]> = TableDefinition::new(&table);
                let mut t = txn.open_table(def)?;
                t.insert(key.as_str(), value.as_slice())?;
            }
            txn.commit()?;
            Ok(())
        })
        .await
        .map_err(|e| AppError::Internal(format!("join: {e}")))?
    }

    async fn get(&self, table: &str, key: &str) -> AppResult<Option<Vec<u8>>> {
        let db = self.db.clone();
        let lock = self.lock.clone();
        let key = key.to_string();
        let table = table.to_string();
        tokio::task::spawn_blocking(move || -> AppResult<Option<Vec<u8>>> {
            let _g = lock.lock();
            let txn = db.begin_read()?;
            let def: TableDefinition<&str, &[u8]> = TableDefinition::new(&table);
            let t = txn.open_table(def)?;
            Ok(t.get(key.as_str())?.map(|v| v.value().to_vec()))
        })
        .await
        .map_err(|e| AppError::Internal(format!("join: {e}")))?
    }

    async fn delete(&self, table: &str, key: &str) -> AppResult<bool> {
        let db = self.db.clone();
        let lock = self.lock.clone();
        let key = key.to_string();
        let table = table.to_string();
        tokio::task::spawn_blocking(move || -> AppResult<bool> {
            let _g = lock.lock();
            let txn = db.begin_write()?;
            let removed = {
                let def: TableDefinition<&str, &[u8]> = TableDefinition::new(&table);
                let mut t = txn.open_table(def)?;
                t.remove(key.as_str())?.is_some()
            };
            txn.commit()?;
            Ok(removed)
        })
        .await
        .map_err(|e| AppError::Internal(format!("join: {e}")))?
    }

    async fn list(&self, table: &str, prefix: &str) -> AppResult<Vec<String>> {
        let db = self.db.clone();
        let lock = self.lock.clone();
        let prefix = prefix.to_string();
        let table = table.to_string();
        tokio::task::spawn_blocking(move || -> AppResult<Vec<String>> {
            let _g = lock.lock();
            let txn = db.begin_read()?;
            let def: TableDefinition<&str, &[u8]> = TableDefinition::new(&table);
            let t = txn.open_table(def)?;
            let iter = t.iter()?;
            let mut out = Vec::new();
            for kv in iter {
                let (k, _) = kv?;
                let s: String = k.value().to_string();
                if prefix.is_empty() || s.starts_with(&prefix) {
                    out.push(s);
                }
            }
            Ok(out)
        })
        .await
        .map_err(|e| AppError::Internal(format!("join: {e}")))?
    }

    async fn count(&self, table: &str, prefix: &str) -> AppResult<u64> {
        let keys = self.list(table, prefix).await?;
        Ok(keys.len() as u64)
    }

    async fn flush(&self) -> AppResult<()> {
        // redb commits on every transaction; explicit flush is a no-op.
        Ok(())
    }

    async fn compact(&self) -> AppResult<()> {
        // redb handles compaction internally.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn put_get_delete_roundtrip() {
        let s = RedbStorage::open_temp().unwrap();
        s.put("test_table", "alpha", b"1".to_vec()).await.unwrap();
        s.put("test_table", "beta", b"2".to_vec()).await.unwrap();
        assert_eq!(
            s.get("test_table", "alpha").await.unwrap(),
            Some(b"1".to_vec())
        );
        assert_eq!(
            s.get("test_table", "beta").await.unwrap(),
            Some(b"2".to_vec())
        );
        assert_eq!(s.get("test_table", "gamma").await.unwrap(), None);
        assert!(s.delete("test_table", "alpha").await.unwrap());
        assert_eq!(s.get("test_table", "alpha").await.unwrap(), None);
    }

    #[tokio::test]
    async fn list_with_prefix() {
        let s = RedbStorage::open_temp().unwrap();
        s.put("t", "user:1", b"a".to_vec()).await.unwrap();
        s.put("t", "user:2", b"b".to_vec()).await.unwrap();
        s.put("t", "post:1", b"c".to_vec()).await.unwrap();
        let keys = s.list("t", "user:").await.unwrap();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"user:1".to_string()));
    }

    #[tokio::test]
    async fn count_returns_total() {
        let s = RedbStorage::open_temp().unwrap();
        s.put("t", "k1", b"1".to_vec()).await.unwrap();
        s.put("t", "k2", b"2".to_vec()).await.unwrap();
        s.put("t", "k3", b"3".to_vec()).await.unwrap();
        assert_eq!(s.count("t", "").await.unwrap(), 3);
        assert_eq!(s.count("t", "k1").await.unwrap(), 1);
    }

    #[tokio::test]
    async fn compound_keys_round_trip() {
        let s = RedbStorage::open_temp().unwrap();
        let key = compound_key(&["col", "doc1"]);
        s.put("t", &key, b"value".to_vec()).await.unwrap();
        let (col, doc) = split_doc_key(&key).unwrap();
        assert_eq!(col, "col");
        assert_eq!(doc, "doc1");
    }
}
