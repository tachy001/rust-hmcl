//! Offline accounts.
//!
//! Port of `org.jackhuang.hmcl.auth.offline.OfflineAccountFactory`.

use md5::Digest;

/// Compute the offline-mode UUID for `username` (undashed hex).
///
/// Equivalent to Java's `UUID.nameUUIDFromBytes(("OfflinePlayer:" + name).getBytes(UTF_8))`:
/// the MD5 digest of the prefixed name, marked as a version-3 UUID.
pub fn offline_uuid(username: &str) -> String {
    let bytes = format!("OfflinePlayer:{username}");
    let digest: Digest = md5::compute(bytes.as_bytes());
    let mut uuid = digest.0;
    // Set version to 3 and the RFC 4122 variant bits.
    uuid[6] = (uuid[6] & 0x0F) | 0x30;
    uuid[8] = (uuid[8] & 0x3F) | 0x80;
    uuid.iter().map(|byte| format!("{byte:02x}")).collect()
}
