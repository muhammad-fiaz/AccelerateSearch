//! Snapshot creation, listing, restoration, and deletion.
//!
//! A snapshot is a versioned, content-addressed archive of the persistent
//! state. By default we snapshot a single file — the redb database — after
//! safely flushing it. The archive layout is:
//!
//! ```text
//! snapshot-YYYYMMDD-HHMMSS.tar.zst
//!   manifest.json   - version, checksum, file count, etc.
//!   accelerate.redb - the database file (or any other file copied in)
//! ```
//!
//! Snapshots are recorded as metadata in the storage layer (`snapshots`
//! table) and the actual archive files live on disk under the configured
//! `snapshots.dir`. Use [`SnapshotService::create`], [`SnapshotService::list`],
//! [`SnapshotService::get`], [`SnapshotService::restore`], and
//! [`SnapshotService::delete`] to manage them.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::info;

use config_crate::SnapshotsConfig;
use errors::{AppError, AppResult};
use models::SnapshotMeta;
use storage::{StorageBackend, TABLE_SNAPSHOTS, get_json, put_json};
use utils::ensure_dir;

/// Current on-disk snapshot manifest format. Bump this when the layout
/// changes in a backwards-incompatible way.
pub const SNAPSHOT_FORMAT_VERSION: u32 = 1;

/// Filename of the manifest inside every snapshot archive.
pub const MANIFEST_FILENAME: &str = "manifest.json";

/// Filename of the database file inside every snapshot archive.
pub const DB_FILENAME: &str = "accelerate.redb";

/// Service that manages snapshots.
pub struct SnapshotService {
    storage: Arc<dyn StorageBackend>,
    cfg: SnapshotsConfig,
}

impl SnapshotService {
    /// Creates a new service.
    #[must_use]
    pub fn new(storage: Arc<dyn StorageBackend>, cfg: SnapshotsConfig) -> Self {
        Self { storage, cfg }
    }

    /// Returns the directory where snapshots are stored.
    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.cfg.dir
    }

    /// Creates a new snapshot by safely copying the file at `source` (the
    /// live database file) into a fresh archive and registers the resulting
    /// metadata in the storage table.
    ///
    /// # Errors
    /// Returns an error if the source file does not exist, the archive
    /// cannot be written, or the metadata table cannot be updated.
    pub async fn create(&self, source: &Path) -> AppResult<SnapshotMeta> {
        if !source.exists() {
            return Err(AppError::bad_request(format!(
                "snapshot source does not exist: {}",
                source.display()
            )));
        }
        ensure_dir(&self.cfg.dir)?;
        let name = default_snapshot_name();
        let archive_path = self.cfg.dir.join(format!("{name}.tar.zst"));

        let manifest = build_manifest(source).map_err(|e| AppError::Internal(e.to_string()))?;
        info!(
            snapshot = %name,
            size = manifest.size,
            checksum = %manifest.checksum,
            "creating snapshot"
        );

        let src = source.to_path_buf();
        let dest = archive_path.clone();
        let manifest_for_archive = manifest.clone();
        tokio::task::spawn_blocking(move || -> std::io::Result<()> {
            write_archive(&src, &dest, &manifest_for_archive)
        })
        .await
        .map_err(|e| AppError::Internal(format!("join: {e}")))?
        .map_err(|e| AppError::Internal(format!("snapshot: {e}")))?;

        let size = std::fs::metadata(&archive_path)
            .map(|m| m.len())
            .unwrap_or(0);
        let meta = SnapshotMeta {
            name: name.clone(),
            created_at: Utc::now(),
            size,
            path: archive_path.to_string_lossy().to_string(),
        };
        put_json(self.storage.as_ref(), TABLE_SNAPSHOTS, &name, &meta).await?;
        Ok(meta)
    }

    /// Lists all snapshots known to the storage layer, newest first.
    pub async fn list(&self) -> AppResult<Vec<SnapshotMeta>> {
        let keys = self.storage.list(TABLE_SNAPSHOTS, "").await?;
        let mut out = Vec::with_capacity(keys.len());
        for k in keys {
            if let Some(bytes) = self.storage.get(TABLE_SNAPSHOTS, &k).await? {
                let m: SnapshotMeta = serde_json::from_slice(&bytes)?;
                out.push(m);
            }
        }
        out.sort_by_key(|b| std::cmp::Reverse(b.created_at));
        Ok(out)
    }

    /// Returns a single snapshot by name.
    pub async fn get(&self, name: &str) -> AppResult<Option<SnapshotMeta>> {
        get_json::<SnapshotMeta>(self.storage.as_ref(), TABLE_SNAPSHOTS, name).await
    }

    /// Deletes a snapshot from both the storage table and the disk.
    ///
    /// Returns `Ok(false)` if no snapshot with that name exists.
    pub async fn delete(&self, name: &str) -> AppResult<bool> {
        let Some(meta) = self.get(name).await? else {
            return Ok(false);
        };
        let p = PathBuf::from(&meta.path);
        if p.exists() {
            std::fs::remove_file(&p).map_err(|e| AppError::Internal(e.to_string()))?;
        }
        self.storage.delete(TABLE_SNAPSHOTS, name).await?;
        Ok(true)
    }

    /// Restores the snapshot archive at `archive` into `target`. The
    /// destination file (a redb database) is overwritten atomically by
    /// extracting the archive into a temporary directory first and then
    /// moving the contained `accelerate.redb` over the target.
    ///
    /// # Errors
    /// Returns an error if the archive is missing, the checksum does not
    /// match, the format is unknown, or the target cannot be replaced.
    pub async fn restore(&self, archive: &Path, target: &Path) -> AppResult<()> {
        if !archive.exists() {
            return Err(AppError::bad_request(format!(
                "snapshot archive does not exist: {}",
                archive.display()
            )));
        }
        let a = archive.to_path_buf();
        let t = target.to_path_buf();
        tokio::task::spawn_blocking(move || -> std::io::Result<()> { restore_archive(&a, &t) })
            .await
            .map_err(|e| AppError::Internal(format!("join: {e}")))?
            .map_err(|e| AppError::Internal(format!("restore: {e}")))?;
        info!(snapshot = %archive.display(), target = %target.display(), "restored snapshot");
        Ok(())
    }
}

/// Returns the next default snapshot name.
#[must_use]
pub fn default_snapshot_name() -> String {
    format!("snapshot-{}", Utc::now().format("%Y%m%d-%H%M%S"))
}

/// On-disk manifest stored at the start of every archive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotManifest {
    /// Format version. See [`SNAPSHOT_FORMAT_VERSION`].
    pub version: u32,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// File count contained in the archive.
    pub file_count: u32,
    /// Total uncompressed size in bytes.
    pub size: u64,
    /// SHA-256 checksum of the database file's bytes.
    pub checksum: String,
}

fn build_manifest(source: &Path) -> std::io::Result<SnapshotManifest> {
    use sha2::Digest;
    use std::io::Read;
    let mut f = std::fs::File::open(source)?;
    let mut hasher = sha2::Sha256::new();
    let mut buf = [0u8; 8192];
    let mut total = 0u64;
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        total += n as u64;
    }
    let checksum = hex::encode(hasher.finalize());
    Ok(SnapshotManifest {
        version: SNAPSHOT_FORMAT_VERSION,
        created_at: Utc::now(),
        file_count: 1,
        size: total,
        checksum,
    })
}

fn write_archive(source: &Path, dest: &Path, manifest: &SnapshotManifest) -> std::io::Result<()> {
    use std::io::{Read, Write};

    // Build the uncompressed tar in a sibling temp file. We need a temp
    // file (not a `Vec<u8>`) because the database may be large, and we
    // need a `File` (which is `Seek`) as the tar writer so we can use
    // `append_writer`. Sibling means the rename into `dest` stays atomic
    // on the same filesystem.
    let parent = dest.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    let tar_path = parent.join(format!(
        ".{}.{}.staging.tar",
        dest.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("snapshot"),
        std::process::id()
    ));
    {
        let tar_file = std::fs::File::create(&tar_path)?;
        let mut tar = tar::Builder::new(tar_file);

        // Manifest first.
        let manifest_bytes = serde_json::to_vec(manifest)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let mut h = tar::Header::new_gnu();
        h.set_path(MANIFEST_FILENAME)?;
        h.set_size(manifest_bytes.len() as u64);
        h.set_mode(0o644);
        h.set_cksum();
        tar.append(&h, manifest_bytes.as_slice())?;

        // Database file, streamed.
        let mut f = std::fs::File::open(source)?;
        let size = f.metadata()?.len();
        let path = std::path::Path::new(DB_FILENAME);
        {
            let mut h2 = tar::Header::new_gnu();
            h2.set_path(DB_FILENAME)?;
            h2.set_size(size);
            h2.set_mode(0o600);
            h2.set_cksum();
            let mut writer = tar.append_writer(&mut h2, path)?;
            let mut buf = vec![0u8; 65_536];
            loop {
                let n = f.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                writer.write_all(&buf[..n])?;
            }
        }
        tar.finish()?;
    }

    // Compress the tar to the destination atomically.
    let tmp_out = dest.with_extension("tar.zst.tmp");
    {
        let mut out = std::fs::File::create(&tmp_out)?;
        let mut zstd = zstd::stream::write::Encoder::new(&mut out, 3)?;
        let mut tar_in = std::fs::File::open(&tar_path)?;
        std::io::copy(&mut tar_in, &mut zstd)?;
        zstd.finish()?;
        out.sync_all()?;
    }

    let result = std::fs::rename(&tmp_out, dest);
    let _ = std::fs::remove_file(&tar_path);
    result?;
    Ok(())
}

fn restore_archive(archive: &Path, target: &Path) -> std::io::Result<()> {
    use std::io::Read;
    let file = std::fs::File::open(archive)?;
    let zstd = zstd::stream::read::Decoder::new(file)?;
    let mut tar = tar::Archive::new(zstd);

    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp_target = target.with_extension("redb.restore.tmp");
    let mut manifest: Option<SnapshotManifest> = None;
    let mut restored = false;

    for entry in tar.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        match path.file_name().and_then(|s| s.to_str()) {
            Some(MANIFEST_FILENAME) => {
                let mut s = String::new();
                entry.read_to_string(&mut s)?;
                manifest = Some(
                    serde_json::from_str(&s)
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?,
                );
            }
            Some(DB_FILENAME) => {
                if let Some(parent) = tmp_target.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut out = std::fs::File::create(&tmp_target)?;
                std::io::copy(&mut entry, &mut out)?;
                out.sync_all()?;
                restored = true;
            }
            _ => {
                // Skip unknown entries (forward-compat).
            }
        }
    }

    let m = manifest.ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "snapshot manifest missing")
    })?;
    if m.version != SNAPSHOT_FORMAT_VERSION {
        let _ = std::fs::remove_file(&tmp_target);
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "unsupported snapshot format version {}, expected {}",
                m.version, SNAPSHOT_FORMAT_VERSION
            ),
        ));
    }

    // Recompute the checksum to detect corruption.
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    let mut f = std::fs::File::open(&tmp_target)?;
    let mut buf = vec![0u8; 65_536];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let actual = hex::encode(hasher.finalize());
    if actual != m.checksum {
        let _ = std::fs::remove_file(&tmp_target);
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "snapshot checksum mismatch: expected {}, got {}",
                m.checksum, actual
            ),
        ));
    }

    if !restored {
        let _ = std::fs::remove_file(&tmp_target);
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "snapshot archive contained no database file",
        ));
    }

    if target.exists() {
        let _ = std::fs::remove_file(target);
    }
    std::fs::rename(&tmp_target, target)?;
    Ok(())
}

/// Lightweight metadata used by the snapshot listing endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SnapshotSummary {
    /// Snapshot name.
    pub name: String,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Size in bytes.
    pub size: u64,
}

impl From<SnapshotMeta> for SnapshotSummary {
    fn from(m: SnapshotMeta) -> Self {
        Self {
            name: m.name,
            created_at: m.created_at,
            size: m.size,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use storage::RedbStorage;

    fn write_file(path: &Path, bytes: &[u8]) {
        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(bytes).unwrap();
        f.sync_all().unwrap();
    }

    #[tokio::test]
    async fn create_list_get_delete_round_trip() {
        let tmp_src = tempfile::tempdir().unwrap();
        let db = tmp_src.path().join("accelerate.redb");
        write_file(&db, b"some-database-payload-12345");

        let backend = Arc::new(RedbStorage::open_temp().unwrap());
        let cfg = SnapshotsConfig {
            dir: tmp_src.path().join("snaps"),
            ..Default::default()
        };
        let svc = SnapshotService::new(backend, cfg);

        let meta = svc.create(&db).await.unwrap();
        assert!(meta.size > 0);
        assert!(std::path::Path::new(&meta.path).exists());

        let list = svc.list().await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, meta.name);

        let fetched = svc.get(&meta.name).await.unwrap();
        assert!(fetched.is_some());

        // Restore into a different directory.
        let target_dir = tempfile::tempdir().unwrap();
        let target = target_dir.path().join("accelerate.redb");
        svc.restore(std::path::Path::new(&meta.path), &target)
            .await
            .unwrap();
        let restored = std::fs::read(&target).unwrap();
        assert_eq!(restored, b"some-database-payload-12345");

        // Delete.
        let deleted = svc.delete(&meta.name).await.unwrap();
        assert!(deleted);
        assert!(svc.list().await.unwrap().is_empty());
    }

    #[test]
    fn manifest_round_trips_via_json() {
        let m = SnapshotManifest {
            version: SNAPSHOT_FORMAT_VERSION,
            created_at: Utc::now(),
            file_count: 1,
            size: 42,
            checksum: "deadbeef".into(),
        };
        let s = serde_json::to_string(&m).unwrap();
        let back: SnapshotManifest = serde_json::from_str(&s).unwrap();
        assert_eq!(back.version, m.version);
        assert_eq!(back.size, m.size);
    }

    #[test]
    fn default_snapshot_name_is_well_formed() {
        let name = default_snapshot_name();
        assert!(
            name.starts_with("snapshot-"),
            "name should start with 'snapshot-': {name}"
        );
        // Format: snapshot-YYYYMMDD-HHMMSS (15 characters after prefix).
        let suffix = name.trim_start_matches("snapshot-");
        assert_eq!(
            suffix.len(),
            15,
            "suffix should be 15 chars (YYYYMMDD-HHMMSS): {name}"
        );
        assert_eq!(suffix.as_bytes()[8], b'-');
    }
}
