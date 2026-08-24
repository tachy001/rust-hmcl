//! Microsoft account authentication via the OAuth device code flow.
//!
//! Port of `org.jackhuang.hmcl.auth.microsoft.MicrosoftService` and
//! `org.jackhuang.hmcl.auth.OAuth` (device code grant).
//!
//! The Azure client id can be overridden with the `HMCL_MICROSOFT_CLIENT_ID`
//! environment variable; without one the flow will not authenticate.

use serde::{Deserialize, Serialize};

use super::Account;

/// The Azure application client id used for device code authentication.
pub fn client_id() -> String {
    std::env::var("HMCL_MICROSOFT_CLIENT_ID").unwrap_or_default()
}

const SCOPE: &str = "XboxLive.signin offline_access";
const DEVICE_CODE_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const XBL_AUTH_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS_AUTH_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MC_LOGIN_URL: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
const MC_STORE_URL: &str = "https://api.minecraftservices.com/entitlements/mcstore";
const MC_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";

/// The response of the device code endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: i64,
    pub interval: i64,
    pub message: Option<String>,
}

/// Errors produced by the Microsoft authentication flow.
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("the azure client id is not configured (set HMCL_MICROSOFT_CLIENT_ID)")]
    NoClientId,
    #[error("microsoft rejected the request: {0}")]
    Microsoft(String),
    #[error("no Xbox account is linked ({0})")]
    NoXboxAccount(i64),
    #[error("xbox account region is restricted ({0})")]
    RegionRestricted(i64),
    #[error("xbox child account cannot log in ({0})")]
    ChildAccount(i64),
    #[error("you must own Minecraft (Java edition) to log in")]
    NoMinecraftOwnership,
    #[error("server response was malformed")]
    MalformedResponse,
}

/// A Microsoft device-code authenticator.
pub struct MicrosoftAuthenticator {
    client: reqwest::Client,
}

impl Default for MicrosoftAuthenticator {
    fn default() -> Self {
        Self::new()
    }
}

impl MicrosoftAuthenticator {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// Request a device code from Microsoft.
    pub async fn request_device_code(&self) -> Result<DeviceCodeResponse, AuthError> {
        let client_id = client_id();
        if client_id.is_empty() {
            return Err(AuthError::NoClientId);
        }
        let response = self
            .client
            .post(DEVICE_CODE_URL)
            .form(&[("client_id", client_id.as_str()), ("scope", SCOPE)])
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(AuthError::Microsoft(format!(
                "device code request failed: {}",
                response.status()
            )));
        }
        response.json().await.map_err(AuthError::Network)
    }

    /// Poll the token endpoint until the user completes the flow.
    ///
    /// Returns `None` while authorization is still pending.
    pub async fn poll_token(
        &self,
        device_code: &str,
    ) -> Result<Option<DeviceTokenResponse>, AuthError> {
        let client_id = client_id();
        if client_id.is_empty() {
            return Err(AuthError::NoClientId);
        }
        let response = self
            .client
            .post(TOKEN_URL)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", client_id.as_str()),
                ("device_code", device_code),
            ])
            .send()
            .await?;
        let status = response.status();
        let text = response.text().await?;
        if status.is_success() {
            let token: DeviceTokenResponse =
                serde_json::from_str(&text).map_err(|_| AuthError::MalformedResponse)?;
            return Ok(Some(token));
        }
        let error: DeviceTokenError = serde_json::from_str(&text).unwrap_or(DeviceTokenError {
            error: "unknown".to_owned(),
        });
        match error.error.as_str() {
            "authorization_pending" | "slow_down" => Ok(None),
            "authorization_declined" => Err(AuthError::Microsoft("authorization declined".into())),
            "expired_token" => Err(AuthError::Microsoft("device code expired".into())),
            other => Err(AuthError::Microsoft(format!("token error: {other}"))),
        }
    }

    /// Complete the authentication chain with a live access token.
    ///
    /// Returns the resulting `Account`.
    pub async fn authenticate_with_access_token(
        &self,
        live_access_token: &str,
        live_refresh_token: &str,
    ) -> Result<Account, AuthError> {
        let xbl_token = self.authenticate_xbl(live_access_token).await?;
        let (uhs, xsts_token) = self.authenticate_xsts(&xbl_token).await?;
        let minecraft_token = self.authenticate_minecraft(&uhs, &xsts_token).await?;
        self.check_ownership(&minecraft_token.access_token).await?;
        let profile = self.fetch_profile(&minecraft_token.access_token).await?;

        Ok(Account::Microsoft {
            username: profile.name,
            uuid: profile.id,
            access_token: minecraft_token.access_token,
            token_type: minecraft_token.token_type,
            refresh_token: live_refresh_token.to_owned(),
            expires_at_ms: (minecraft_token.expires_in as i64) * 1000
                + std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0),
        })
    }

    async fn authenticate_xbl(&self, access_token: &str) -> Result<String, AuthError> {
        let response: XblResponse = self
            .client
            .post(XBL_AUTH_URL)
            .json(&serde_json::json!({
                "Properties": {
                    "AuthMethod": "RPS",
                    "SiteName": "user.auth.xboxlive.com",
                    "RpsTicket": format!("d={access_token}")
                },
                "RelyingParty": "http://auth.xboxlive.com",
                "TokenType": "JWT"
            }))
            .send()
            .await?
            .json()
            .await?;
        Ok(response.token)
    }

    async fn authenticate_xsts(&self, xbl_token: &str) -> Result<(String, String), AuthError> {
        let response = self
            .client
            .post(XSTS_AUTH_URL)
            .json(&serde_json::json!({
                "Properties": {
                    "SandboxId": "RETAIL",
                    "UserTokens": [xbl_token]
                },
                "RelyingParty": "rp://api.minecraftservices.com/",
                "TokenType": "JWT"
            }))
            .send()
            .await?;
        let status = response.status();
        let text = response.text().await?;
        let parsed: Result<XblResponse, _> = serde_json::from_str(&text);
        let response = parsed.map_err(|_| AuthError::MalformedResponse)?;
        if status != reqwest::StatusCode::OK {
            let code = response.error_code;
            match code {
                2148916233 => return Err(AuthError::NoXboxAccount(code)),
                2148916235 => return Err(AuthError::RegionRestricted(code)),
                2148916238 => return Err(AuthError::ChildAccount(code)),
                _ => return Err(AuthError::Microsoft(format!("xsts error code {code}"))),
            }
        }
        let uhs = response
            .display_claims
            .and_then(|claims| claims.xui.into_iter().next())
            .and_then(|xui| xui.get("uhs").and_then(|v| v.as_str().map(String::from)))
            .ok_or(AuthError::MalformedResponse)?;
        Ok((uhs, response.token))
    }

    async fn authenticate_minecraft(
        &self,
        uhs: &str,
        xsts_token: &str,
    ) -> Result<MinecraftTokenResponse, AuthError> {
        Ok(self
            .client
            .post(MC_LOGIN_URL)
            .json(&serde_json::json!({
                "identityToken": format!("XBL3.0 x={uhs};{xsts_token}")
            }))
            .send()
            .await?
            .json()
            .await?)
    }

    async fn check_ownership(&self, access_token: &str) -> Result<(), AuthError> {
        let response = self
            .client
            .get(MC_STORE_URL)
            .bearer_auth(access_token)
            .send()
            .await?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(AuthError::NoMinecraftOwnership)
        }
    }

    async fn fetch_profile(
        &self,
        access_token: &str,
    ) -> Result<MinecraftProfileResponse, AuthError> {
        Ok(self
            .client
            .get(MC_PROFILE_URL)
            .bearer_auth(access_token)
            .send()
            .await?
            .json()
            .await?)
    }
}

/// The device token poll result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceTokenResponse {
    pub access_token: String,
    pub refresh_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeviceTokenError {
    error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct XblResponse {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "ErrorCode", default)]
    error_code: i64,
    #[serde(rename = "DisplayClaims", default)]
    display_claims: Option<XblDisplayClaims>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct XblDisplayClaims {
    xui: Vec<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MinecraftTokenResponse {
    access_token: String,
    token_type: String,
    expires_in: i64,
    username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MinecraftProfileResponse {
    id: String,
    name: String,
}
