//! Shared utilities for AccelerateSearch.
//!
//! Provides:
//!
//! * Cryptographically secure random key and ID generation.
//! * Hashing helpers (SHA-256, BLAKE3).
//! * Time and duration utilities.
//! * String sanitization helpers.
//! * File-system helpers (atomic write, ensure directory).
//! * Tiny ANSI color helpers.

pub mod color;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use rand_core::Rng;
use sha2::{Digest, Sha256};

/// Length in characters of a generated API key (64 hex chars = 32 bytes).
pub const API_KEY_HEX_LENGTH: usize = 64;

/// Generates a cryptographically secure random hex string of `byte_count`
/// bytes (output is `2 * byte_count` hex characters).
///
/// # Panics
/// Panics if the OS RNG fails.
#[must_use]
pub fn random_hex(byte_count: usize) -> String {
    let mut bytes = vec![0u8; byte_count];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Generates a fresh API key string (64 hex characters by default).
#[must_use]
pub fn generate_api_key() -> String {
    random_hex(API_KEY_HEX_LENGTH / 2)
}

/// SHA-256 hash of `input`, returned as lowercase hex.
#[must_use]
pub fn sha256_hex(input: impl AsRef<[u8]>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_ref());
    hex::encode(hasher.finalize())
}

/// BLAKE3 hash of `input`, returned as lowercase hex.
#[must_use]
pub fn blake3_hex(input: impl AsRef<[u8]>) -> String {
    let hash = blake3::hash(input.as_ref());
    hex::encode(hash.as_bytes())
}

/// Generates a new UUID v4 (random).
#[must_use]
pub fn new_uuid_v4() -> uuid::Uuid {
    uuid::Uuid::new_v4()
}

/// Generates a new UUID v7 (time-ordered).
#[must_use]
pub fn new_uuid_v7() -> uuid::Uuid {
    uuid::Uuid::now_v7()
}

/// Returns the current UTC time.
#[must_use]
pub fn now() -> DateTime<Utc> {
    Utc::now()
}

/// Returns the current Unix timestamp in seconds.
#[must_use]
pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Returns the current Unix timestamp in milliseconds.
#[must_use]
pub fn unix_now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Parses a duration string like "30s", "5m", "1h", "2d" into a [`Duration`].
///
/// Supported units: `ns`, `us`, `ms`, `s`, `m`, `h`, `d`.
///
/// # Errors
/// Returns `None` if the string is empty, has an unknown unit, or contains
/// a non-numeric prefix.
#[must_use]
pub fn parse_duration(s: &str) -> Option<Duration> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let (num, unit) = s.split_at(
        s.find(|c: char| !c.is_ascii_digit() && c != '.')
            .unwrap_or(s.len()),
    );
    let value: f64 = num.parse().ok()?;
    let multiplier = match unit {
        "ns" => 1e-9,
        "us" | "µs" => 1e-6,
        "ms" => 1e-3,
        "s" | "" => 1.0,
        "m" => 60.0,
        "h" => 3_600.0,
        "d" => 86_400.0,
        _ => return None,
    };
    let secs = value * multiplier;
    if !secs.is_finite() || secs < 0.0 {
        return None;
    }
    Some(Duration::from_secs_f64(secs))
}

/// Formats a byte count as a human-readable string.
#[must_use]
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{:.2} {}", size, UNITS[unit])
    }
}

/// Sanitizes a user-supplied string by stripping control characters and
/// null bytes, then trimming whitespace.
#[must_use]
pub fn sanitize_string(s: &str) -> String {
    s.chars()
        .filter(|c| *c != '\u{0}' && (!c.is_control() || c.is_whitespace()))
        .collect::<String>()
        .trim()
        .to_string()
}

/// Ensures the directory at `path` exists, creating it if necessary.
///
/// # Errors
/// Returns the underlying I/O error if directory creation fails.
pub fn ensure_dir(path: impl AsRef<Path>) -> std::io::Result<()> {
    let p = path.as_ref();
    if p.exists() {
        if p.is_dir() {
            Ok(())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("path exists and is not a directory: {}", p.display()),
            ))
        }
    } else {
        std::fs::create_dir_all(p)
    }
}

/// Atomically writes `content` to `path` by writing to a temporary file
/// in the same directory and then renaming it.
///
/// # Errors
/// Returns the underlying I/O error if the write or rename fails.
pub fn atomic_write(path: impl AsRef<Path>, content: impl AsRef<[u8]>) -> std::io::Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    let tmp = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|s| s.to_str())
            .unwrap_or("accelerate")
    ));
    std::fs::write(&tmp, content.as_ref())?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Returns the process uptime since the given start instant.
#[must_use]
pub fn uptime_since(start: Instant) -> Duration {
    start.elapsed()
}

/// A monotonic timer used to measure `processingTimeMs` for search requests.
#[derive(Debug, Clone, Copy)]
pub struct Stopwatch {
    start: Instant,
}

impl Default for Stopwatch {
    fn default() -> Self {
        Self::new()
    }
}

impl Stopwatch {
    /// Starts a new stopwatch.
    #[must_use]
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    /// Returns the elapsed duration.
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    /// Returns the elapsed duration in milliseconds.
    #[must_use]
    pub fn elapsed_ms(&self) -> u128 {
        self.start.elapsed().as_millis()
    }

    /// Restarts the stopwatch and returns the previous elapsed duration.
    #[must_use]
    pub fn reset(&mut self) -> Duration {
        let now = Instant::now();
        let prev = now.duration_since(self.start);
        self.start = now;
        prev
    }
}

/// Normalizes a path by collapsing `.` and `..` components without touching
/// the file system.
#[must_use]
pub fn normalize_path(path: impl AsRef<Path>) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for comp in path.as_ref().components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Validates a host string (IPv4, IPv6, or DNS name). Returns true on
/// success.
#[must_use]
pub fn is_valid_host(host: &str) -> bool {
    if host.is_empty() || host.len() > 253 {
        return false;
    }
    if host == "localhost" {
        return true;
    }
    if host.parse::<std::net::IpAddr>().is_ok() {
        return true;
    }
    host.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_hex_produces_correct_length() {
        let s = random_hex(16);
        assert_eq!(s.len(), 32);
        assert!(s.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn generate_api_key_has_expected_length() {
        let k = generate_api_key();
        assert_eq!(k.len(), API_KEY_HEX_LENGTH);
    }

    #[test]
    fn sha256_blake3_are_deterministic() {
        assert_eq!(sha256_hex("abc"), sha256_hex("abc"));
        assert_eq!(blake3_hex("abc"), blake3_hex("abc"));
        assert_ne!(sha256_hex("abc"), sha256_hex("abd"));
    }

    #[test]
    fn parse_duration_supports_units() {
        assert_eq!(parse_duration("5s").unwrap(), Duration::from_secs(5));
        assert_eq!(parse_duration("100ms").unwrap(), Duration::from_millis(100));
        assert_eq!(parse_duration("2m").unwrap(), Duration::from_secs(120));
        assert_eq!(parse_duration("1h").unwrap(), Duration::from_secs(3600));
        assert_eq!(parse_duration("2d").unwrap(), Duration::from_secs(172_800));
        assert!(parse_duration("garbage").is_none());
        assert!(parse_duration("").is_none());
    }

    #[test]
    fn format_bytes_is_readable() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1_048_576), "1.00 MB");
    }

    #[test]
    fn sanitize_string_strips_nulls_and_controls() {
        let s = sanitize_string("hello\u{0} \tworld\n");
        assert_eq!(s, "hello \tworld");
    }

    #[test]
    fn stopwatch_measures_time() {
        let sw = Stopwatch::new();
        std::thread::sleep(Duration::from_millis(2));
        assert!(sw.elapsed_ms() >= 1);
    }

    #[test]
    fn host_validation_works() {
        assert!(is_valid_host("0.0.0.0"));
        assert!(is_valid_host("::1"));
        assert!(is_valid_host("localhost"));
        assert!(is_valid_host("example.com"));
        assert!(!is_valid_host(""));
        assert!(!is_valid_host("bad host"));
    }

    #[test]
    fn normalize_path_works() {
        let p = normalize_path("./a/b/../c/./d");
        assert_eq!(p, PathBuf::from("a/c/d"));
    }
}
