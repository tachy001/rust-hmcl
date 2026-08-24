//! I/O helpers mirroring parts of HMCL's `util.io.FileUtils`.

use std::path::Path;
use std::time::Duration;

use anyhow::Context;

/// Compute the SHA-1 checksum of a file, returning a hex string.
pub fn sha1_hex(path: &Path) -> anyhow::Result<String> {
    use sha1::{Digest, Sha1};
    let bytes = std::fs::read(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let mut hasher = Sha1::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

/// Atomically-ish write `bytes` to `path`, creating parent directories.
pub fn write_bytes(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))
}

/// Wait for a condition to become true, polling every `interval`.
pub async fn wait_until<F>(mut predicate: F, interval: Duration, timeout: Duration) -> bool
where
    F: FnMut() -> bool,
{
    let deadline = tokio::time::Instant::now() + timeout;
    while tokio::time::Instant::now() < deadline {
        if predicate() {
            return true;
        }
        tokio::time::sleep(interval).await;
    }
    predicate()
}
