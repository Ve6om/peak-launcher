//! Accounts: Microsoft sign-in (device code flow -> Xbox Live -> XSTS ->
//! Minecraft services) plus offline accounts in development builds.

use super::net::client;
use super::paths::{self, Paths};
use crate::error::{Error, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Your Azure app's client ID. Set PEAK_MS_CLIENT_ID when building. Each
/// launcher needs its own ID, approved by Mojang for the Minecraft API.
pub const MS_CLIENT_ID: Option<&str> = option_env!("PEAK_MS_CLIENT_ID");
const SCOPE: &str = "XboxLive.signin offline_access";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AccountKind {
    Microsoft,
    Offline,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    /// Minecraft profile UUID (no dashes).
    pub id: String,
    pub username: String,
    pub kind: AccountKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
}

/// What the frontend sees: no tokens.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    pub id: String,
    pub username: String,
    pub kind: AccountKind,
    pub active: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AccountStore {
    pub active: Option<String>,
    pub accounts: Vec<Account>,
}

pub async fn load(paths: &Paths) -> Result<AccountStore> {
    Ok(paths::read_json(&paths.accounts_file()).await?.unwrap_or_default())
}

pub async fn save(paths: &Paths, store: &AccountStore) -> Result<()> {
    paths::write_json(&paths.accounts_file(), store).await
}

pub fn info(store: &AccountStore) -> Vec<AccountInfo> {
    store
        .accounts
        .iter()
        .map(|a| AccountInfo {
            id: a.id.clone(),
            username: a.username.clone(),
            kind: a.kind.clone(),
            active: store.active.as_deref() == Some(a.id.as_str()),
        })
        .collect()
}

pub async fn upsert(paths: &Paths, account: Account) -> Result<()> {
    let mut store = load(paths).await?;
    store.accounts.retain(|a| a.id != account.id);
    store.active = Some(account.id.clone());
    store.accounts.push(account);
    save(paths, &store).await
}

pub fn offline_account(username: &str) -> Result<Account> {
    let valid = (3..=16).contains(&username.len()) && username.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !valid {
        return Err(Error::msg("Usernames must be 3-16 characters: letters, numbers and _"));
    }
    Ok(Account {
        id: uuid::Uuid::new_v4().simple().to_string(),
        username: username.to_string(),
        kind: AccountKind::Offline,
        access_token: None,
        refresh_token: None,
        expires_at: None,
    })
}

// ---------- Microsoft device code flow ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCode {
    #[serde(alias = "device_code")]
    pub device_code: String,
    #[serde(alias = "user_code")]
    pub user_code: String,
    #[serde(alias = "verification_uri")]
    pub verification_uri: String,
    #[serde(alias = "expires_in")]
    pub expires_in: u64,
    pub interval: u64,
}

fn client_id() -> Result<&'static str> {
    MS_CLIENT_ID.filter(|s| !s.is_empty()).ok_or_else(|| {
        Error::msg(
            "Microsoft sign-in isn't configured yet. Build with PEAK_MS_CLIENT_ID set to your Azure app's client ID.",
        )
    })
}

pub async fn start_device_code() -> Result<DeviceCode> {
    let resp = client()
        .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode")
        .form(&[("client_id", client_id()?), ("scope", SCOPE)])
        .send()
        .await?
        .error_for_status()?;
    Ok(resp.json().await?)
}

#[derive(Deserialize)]
struct MsToken {
    access_token: String,
    refresh_token: String,
}

#[derive(Deserialize)]
struct MsError {
    error: String,
}

/// Poll until the user finishes signing in in their browser, then complete
/// the Xbox/Minecraft chain and return the account.
pub async fn poll_device_code(code: &DeviceCode) -> Result<Account> {
    let deadline = Utc::now() + Duration::seconds(code.expires_in as i64);
    let mut interval = code.interval.max(1);
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
        if Utc::now() > deadline {
            return Err(Error::msg("The sign-in code expired. Please try again."));
        }
        let resp = client()
            .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", client_id()?),
                ("device_code", code.device_code.as_str()),
            ])
            .send()
            .await?;
        if resp.status().is_success() {
            let tok: MsToken = resp.json().await?;
            return complete_login(&tok.access_token, tok.refresh_token).await;
        }
        let err: MsError = resp.json().await.unwrap_or(MsError { error: "unknown".into() });
        match err.error.as_str() {
            "authorization_pending" => {}
            "slow_down" => interval += 5,
            "authorization_declined" => return Err(Error::msg("Sign-in was cancelled.")),
            "expired_token" => return Err(Error::msg("The sign-in code expired. Please try again.")),
            other => return Err(Error::msg(format!("Microsoft sign-in failed: {other}"))),
        }
    }
}

/// Refresh a Microsoft account's Minecraft token if it is close to expiring.
pub async fn ensure_fresh(paths: &Paths, account: Account) -> Result<Account> {
    if account.kind != AccountKind::Microsoft {
        return Ok(account);
    }
    let fresh = account.expires_at.map(|e| e > Utc::now() + Duration::minutes(5)).unwrap_or(false);
    if fresh && account.access_token.is_some() {
        return Ok(account);
    }
    let refresh = account.refresh_token.clone().ok_or_else(|| Error::msg("Please sign in again."))?;
    let resp = client()
        .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", client_id()?),
            ("refresh_token", refresh.as_str()),
            ("scope", SCOPE),
        ])
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(Error::msg(format!("Your session for {} expired. Please sign in again.", account.username)));
    }
    let tok: MsToken = resp.json().await?;
    let updated = complete_login(&tok.access_token, tok.refresh_token).await?;
    upsert(paths, updated.clone()).await?;
    Ok(updated)
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct XblResponse {
    token: String,
    display_claims: XblClaims,
}

#[derive(Deserialize)]
struct XblClaims {
    xui: Vec<XblXui>,
}

#[derive(Deserialize)]
struct XblXui {
    uhs: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct XstsError {
    x_err: u64,
}

#[derive(Deserialize)]
struct McLogin {
    access_token: String,
    expires_in: i64,
}

#[derive(Deserialize)]
struct McProfile {
    id: String,
    name: String,
}

async fn complete_login(ms_access: &str, ms_refresh: String) -> Result<Account> {
    // 1. Xbox Live
    let xbl: XblResponse = client()
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&json!({
            "Properties": { "AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": format!("d={ms_access}") },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let uhs = xbl.display_claims.xui.first().map(|x| x.uhs.clone()).ok_or_else(|| Error::msg("Xbox Live returned no user hash"))?;

    // 2. XSTS
    let resp = client()
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&json!({
            "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl.token] },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }))
        .send()
        .await?;
    if resp.status().as_u16() == 401 {
        let err: XstsError = resp.json().await.unwrap_or(XstsError { x_err: 0 });
        return Err(Error::msg(match err.x_err {
            2148916233 => "This Microsoft account doesn't have an Xbox profile yet. Sign in at xbox.com once, then try again.",
            2148916235 => "Xbox Live isn't available in your country.",
            2148916236 | 2148916237 => "This account needs adult verification on xbox.com.",
            2148916238 => "This is a child account. An adult must add it to a Microsoft family first.",
            _ => "Xbox Live sign-in failed.",
        }));
    }
    let xsts: XblResponse = resp.error_for_status()?.json().await?;

    // 3. Minecraft services
    let resp = client()
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&json!({ "identityToken": format!("XBL3.0 x={uhs};{}", xsts.token) }))
        .send()
        .await?;
    // New Azure apps must be approved by Mojang before they can sign in.
    if resp.status().as_u16() == 403 {
        return Err(Error::msg(
            "Mojang hasn't approved this launcher's Microsoft app ID for Minecraft yet. Submit the client ID at aka.ms/mce-reviewappid and try again once it's approved.",
        ));
    }
    let mc: McLogin = resp.error_for_status()?.json().await?;

    // 4. Profile (fails with 404 if the account doesn't own Minecraft)
    let resp = client()
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(&mc.access_token)
        .send()
        .await?;
    if resp.status().as_u16() == 404 {
        return Err(Error::msg("This Microsoft account doesn't own Minecraft: Java Edition."));
    }
    let profile: McProfile = resp.error_for_status()?.json().await?;

    Ok(Account {
        id: profile.id,
        username: profile.name,
        kind: AccountKind::Microsoft,
        access_token: Some(mc.access_token),
        refresh_token: Some(ms_refresh),
        expires_at: Some(Utc::now() + Duration::seconds(mc.expires_in)),
    })
}
