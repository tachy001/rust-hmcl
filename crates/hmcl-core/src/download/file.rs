//! File downloading with progress, verification and resuming.
//!
//! Port of HMCL's `task.FileDownloadTask` essentials.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use futures_util::StreamExt;

use sha1::{Digest, Sha1};

/// Shared download progress counters.
#[derive(Debug, Default, Clone)]
pub struct DownloadProgress {
    pub done: Arc<AtomicU64>,
    pub total: Arc<AtomicU64>,
}

impl DownloadProgress {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn done(&self) -> u64 {
        self.done.load(Ordering::SeqCst)
    }

    pub fn total(&self) -> u64 {
        self.total.load(Ordering::SeqCst)
    }

    pub fn add_done(&self, delta: u64) {
        self.done.fetch_add(delta, Ordering::SeqCst);
    }

    pub fn add_total(&self, delta: u64) {
        self.total.fetch_add(delta, Ordering::SeqCst);
    }

    pub fn fraction(&self) -> f32 {
        let total = self.total();
        if total == 0 {
            0.0
        } else {
            (self.done() as f32 / total as f32).clamp(0.0, 1.0)
        }
    }
}

/// Whether `path` already exists with the expected size (`None` = any size).
pub fn file_matches_size(path: &Path, size: Option<u64>) -> bool {
    match (path.exists(), size) {
        (true, Some(expected)) => std::fs::metadata(path)
            .map(|meta| meta.len() == expected)
            .unwrap_or(false),
        (true, None) => true,
        (false, _) => false,
    }
}

/// Whether `path` exists and its SHA-1 matches `sha1`.
pub fn file_matches_sha1(path: &Path, sha1: &str) -> bool {
    if !path.exists() {
        return false;
    }
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    let mut hasher = Sha1::new();
    hasher.update(&bytes);
    format!("{:x}", hasher.finalize()).eq_ignore_ascii_case(sha1)
}

/// Download `url` to `dest`, skipping when the file already matches the
/// expected size or SHA-1. Progress is accumulated into `progress`.
pub async fn download_file(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    sha1: Option<&str>,
    size: Option<u64>,
    progress: &DownloadProgress,
) -> anyhow::Result<()> {
    // Skip files already downloaded.
    if let Some(sha1) = sha1 {
        if file_matches_sha1(dest, sha1) {
            return Ok(());
        }
    } else if file_matches_size(dest, size) {
        return Ok(());
    }

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = dest.with_extension("tmp");
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("failed to GET {url}: {e}"))?;
    if !response.status().is_success() {
        anyhow::bail!("failed to download {url}: HTTP {}", response.status());
    }

    let mut file = tokio::fs::File::create(&tmp).await?;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await?;
        progress.add_done(chunk.len() as u64);
    }
    tokio::io::AsyncWriteExt::flush(&mut file).await?;
    drop(file);

    if let Some(expected) = sha1
        && !file_matches_sha1(&tmp, expected) {
            let _ = std::fs::remove_file(&tmp);
            anyhow::bail!("checksum mismatch for {url}");
        }
    std::fs::rename(&tmp, dest)?;
    Ok(())
}
