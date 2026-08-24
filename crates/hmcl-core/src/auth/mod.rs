//! Account models and authentication flows.
//!
//! Port of HMCL's `org.jackhuang.hmcl.auth` package: offline accounts and
//! Microsoft accounts (OAuth device code flow).

pub mod microsoft;
pub mod offline;

use serde::{Deserialize, Serialize};

pub use microsoft::{DeviceCodeResponse, MicrosoftAuthenticator};
pub use offline::offline_uuid;

/// A stored account, serializable to JSON for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Account {
    Offline {
        /// The account name.
        username: String,
        /// The offline-mode UUID derived from the name.
        uuid: String,
    },
    Microsoft {
        username: String,
        uuid: String,
        /// The Minecraft access token.
        access_token: String,
        token_type: String,
        /// The Microsoft refresh token.
        refresh_token: String,
        /// The unix timestamp (ms) when the access token expires.
        expires_at_ms: i64,
    },
}

impl Account {
    /// The display name of the account.
    pub fn username(&self) -> &str {
        match self {
            Account::Offline { username, .. } => username,
            Account::Microsoft { username, .. } => username,
        }
    }

    /// The player UUID (undashed).
    pub fn uuid(&self) -> &str {
        match self {
            Account::Offline { uuid, .. } => uuid,
            Account::Microsoft { uuid, .. } => uuid,
        }
    }

    /// The player UUID (undashed).
    pub fn uuid_dashed(&self) -> String {
        let uuid = self.uuid();
        if uuid.len() == 32 {
            format!(
                "{}-{}-{}-{}-{}",
                &uuid[0..8],
                &uuid[8..12],
                &uuid[12..16],
                &uuid[16..20],
                &uuid[20..32]
            )
        } else {
            uuid.to_owned()
        }
    }
}

/// The list of accounts, persisted to `accounts.json` in the data directory.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AccountStorage {
    pub accounts: Vec<Account>,
    pub selected: Option<String>,
}

impl AccountStorage {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        if path.exists() {
            let text = std::fs::read_to_string(path)?;
            Ok(serde_json::from_str(&text)?)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self, path: &std::path::Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(path, text)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_offline_uuid_matches_java() {
        // Java: UUID.nameUUIDFromBytes(("OfflinePlayer:" + name).getBytes(UTF_8))
        // MD5("OfflinePlayer:Steve") = 5627dd98e6bebc21f8a8e92344183641
        // with version bits (byte 6) and variant bits (byte 8) applied.
        let uuid = offline_uuid("Steve");
        assert_eq!(uuid, "5627dd98e6be3c21b8a8e92344183641");
    }

    #[test]
    fn test_account_serde() {
        let account = Account::Offline {
            username: "test".to_owned(),
            uuid: offline_uuid("test"),
        };
        let json = serde_json::to_string(&account).unwrap();
        let parsed: Account = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.username(), "test");
        assert_eq!(parsed.uuid(), offline_uuid("test"));
    }
}
