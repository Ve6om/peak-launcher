//! Accounts: Microsoft sign-in (device code flow -> Xbox Live -> XSTS ->
//! Minecraft services) plus offline accounts in development builds.

use super::net::client;
use super::paths::{self, Paths};
use super::secrets::{self, SecretKey};
use crate::error::{Error, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};

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

/// An account in memory. Only the id, name, kind and (encrypted) refresh
/// token are saved; the Minecraft token lasts a day and is kept in memory.
#[derive(Debug, Clone)]
pub struct Account {
    /// Minecraft profile UUID (no dashes).
    pub id: String,
    pub username: String,
    pub kind: AccountKind,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    /// An encrypted refresh token that couldn't be opened (e.g. the password
    /// store was locked); kept as is so it isn't lost on the next save.
    pub unopened: Option<String>,
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

#[derive(Debug, Clone, Default)]
pub struct AccountStore {
    pub active: Option<String>,
    pub accounts: Vec<Account>,
}

// ---------- on disk ----------

#[derive(Debug, Default, Serialize, Deserialize)]
struct DiskStore {
    #[serde(default)]
    active: Option<String>,
    #[serde(default)]
    accounts: Vec<DiskAccount>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiskAccount {
    id: String,
    username: String,
    kind: AccountKind,
    /// Microsoft refresh token, encrypted with the key in the OS password store.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sealed_refresh_token: Option<String>,
    /// Unencrypted: only without a password store, or from older versions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    refresh_token: Option<String>,
    /// Older versions saved the Minecraft token too; read once, never written.
    #[serde(default, skip_serializing)]
    access_token: Option<String>,
    #[serde(default, skip_serializing)]
    expires_at: Option<DateTime<Utc>>,
}

/// Minecraft tokens for this session, by account id.
static SESSIONS: LazyLock<Mutex<HashMap<String, (String, DateTime<Utc>)>>> = LazyLock::new(Default::default);

/// Turn the saved file into accounts. Also says whether the file should be
/// rewritten (it holds tokens in an older, unencrypted form).
fn from_disk(disk: DiskStore, key: Option<&SecretKey>) -> (AccountStore, bool) {
    let mut rewrite = false;
    let mut sessions = SESSIONS.lock().unwrap();
    let accounts = disk
        .accounts
        .into_iter()
        .map(|d| {
            let opened = match (&d.sealed_refresh_token, key) {
                (Some(s), Some(k)) => secrets::open(k, &d.id, s),
                _ => None,
            };
            let unopened = if opened.is_none() { d.sealed_refresh_token.clone() } else { None };
            if d.refresh_token.is_some() && key.is_some() {
                rewrite = true;
            }
            if d.access_token.is_some() {
                rewrite = true;
            }
            if let (Some(tok), Some(exp)) = (d.access_token, d.expires_at) {
                sessions.entry(d.id.clone()).or_insert((tok, exp));
            }
            let (access_token, expires_at) = match sessions.get(&d.id) {
                Some((t, e)) => (Some(t.clone()), Some(*e)),
                None => (None, None),
            };
            Account {
                refresh_token: opened.or(d.refresh_token),
                id: d.id,
                username: d.username,
                kind: d.kind,
                access_token,
                expires_at,
                unopened,
            }
        })
        .collect();
    (AccountStore { active: disk.active, accounts }, rewrite)
}

/// What gets written: refresh tokens encrypted when possible, Minecraft
/// tokens only remembered in memory.
fn to_disk(store: &AccountStore, key: Option<&SecretKey>) -> DiskStore {
    let mut sessions = SESSIONS.lock().unwrap();
    let accounts = store
        .accounts
        .iter()
        .map(|a| {
            if let (Some(t), Some(e)) = (&a.access_token, a.expires_at) {
                sessions.insert(a.id.clone(), (t.clone(), e));
            }
            let (sealed, plain) = match (&a.refresh_token, key) {
                (Some(t), Some(k)) => (Some(secrets::seal(k, &a.id, t)), None),
                (Some(t), None) => (None, Some(t.clone())),
                (None, _) => (a.unopened.clone(), None),
            };
            DiskAccount {
                id: a.id.clone(),
                username: a.username.clone(),
                kind: a.kind.clone(),
                sealed_refresh_token: sealed,
                refresh_token: plain,
                access_token: None,
                expires_at: None,
            }
        })
        .collect();
    DiskStore { active: store.active.clone(), accounts }
}

/// Whether sign-in tokens are encrypted on this computer.
pub fn tokens_encrypted() -> bool {
    secrets::key().is_some()
}

async fn key() -> Option<SecretKey> {
    tokio::task::spawn_blocking(secrets::key).await.unwrap_or(None)
}

pub async fn load(paths: &Paths) -> Result<AccountStore> {
    let disk: DiskStore = paths::read_json(&paths.accounts_file()).await?.unwrap_or_default();
    let key = key().await;
    let (store, rewrite) = from_disk(disk, key.as_ref());
    if rewrite {
        // Encrypt tokens saved by older versions straight away.
        save(paths, &store).await?;
    }
    Ok(store)
}

pub async fn save(paths: &Paths, store: &AccountStore) -> Result<()> {
    let key = key().await;
    paths::write_json(&paths.accounts_file(), &to_disk(store, key.as_ref())).await
}

/// Forget an account's in-memory token (when it's removed).
pub fn forget_session(id: &str) {
    SESSIONS.lock().unwrap().remove(id);
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
        unopened: None,
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

/// Bumped whenever a sign-in starts or is cancelled; a waiting sign-in that
/// sees a different number than it began with stops.
static LOGIN_GEN: AtomicU64 = AtomicU64::new(0);

/// Stop any sign-in that is waiting for the user.
pub fn cancel_login() {
    LOGIN_GEN.fetch_add(1, Ordering::SeqCst);
}

pub async fn start_device_code() -> Result<DeviceCode> {
    // A new attempt replaces any older one that is still waiting.
    cancel_login();
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
    let started = LOGIN_GEN.load(Ordering::SeqCst);
    let cancelled = || LOGIN_GEN.load(Ordering::SeqCst) != started;
    loop {
        // Wait in short steps so Cancel takes effect straight away.
        for _ in 0..interval * 4 {
            if cancelled() {
                return Err(Error::msg("Sign-in cancelled."));
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
        if cancelled() {
            return Err(Error::msg("Sign-in cancelled."));
        }
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
    // A launcher's Azure app has to be on Mojang's allow list (aka.ms/mce-reviewappid)
    // before this works. A fresh approval can take a few hours to reach everywhere.
    if resp.status().as_u16() == 403 {
        return Err(Error::msg(
            "Minecraft refused this sign-in (error 403). If this app was only just approved by Mojang it can take a few hours to start working, so try again a little later.",
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
        unopened: None,
    })
}

#[cfg(test)]
mod storage_tests {
    use super::*;

    fn legacy_file(id: &str) -> DiskStore {
        serde_json::from_value(json!({
            "active": id,
            "accounts": [{
                "id": id, "username": "Steve", "kind": "microsoft",
                "accessToken": "mc-access-token", "refreshToken": "ms-refresh-token",
                "expiresAt": "2099-01-01T00:00:00Z"
            }]
        }))
        .unwrap()
    }

    #[test]
    fn upgrades_old_files_and_never_writes_tokens_in_the_clear() {
        let key = [3u8; 32];
        let (store, rewrite) = from_disk(legacy_file("acc-upgrade"), Some(&key));
        assert!(rewrite, "plain tokens should trigger a rewrite");
        let a = &store.accounts[0];
        assert_eq!(a.refresh_token.as_deref(), Some("ms-refresh-token"));
        assert_eq!(a.access_token.as_deref(), Some("mc-access-token"), "kept for this session");

        let text = serde_json::to_string(&to_disk(&store, Some(&key))).unwrap();
        assert!(!text.contains("ms-refresh-token"), "{text}");
        assert!(!text.contains("mc-access-token"), "{text}");
        assert!(!text.contains("expiresAt"), "{text}");
        assert!(text.contains("sealedRefreshToken"));

        // Reading the new file back needs no rewrite and gives the same tokens.
        let (again, rewrite) = from_disk(serde_json::from_str(&text).unwrap(), Some(&key));
        assert!(!rewrite);
        assert_eq!(again.accounts[0].refresh_token.as_deref(), Some("ms-refresh-token"));
        assert_eq!(again.accounts[0].access_token.as_deref(), Some("mc-access-token"));
    }

    #[test]
    fn without_a_password_store_tokens_stay_usable() {
        let (store, rewrite) = from_disk(legacy_file("acc-nostore"), None);
        assert!(rewrite, "the Minecraft token is still dropped from the file");
        let text = serde_json::to_string(&to_disk(&store, None)).unwrap();
        assert!(text.contains("ms-refresh-token"));
        assert!(!text.contains("mc-access-token"));
        let (again, rewrite) = from_disk(serde_json::from_str(&text).unwrap(), None);
        assert!(!rewrite);
        assert_eq!(again.accounts[0].refresh_token.as_deref(), Some("ms-refresh-token"));
    }

    #[test]
    fn keeps_tokens_it_cannot_open_yet() {
        let key = [3u8; 32];
        let (store, _) = from_disk(legacy_file("acc-locked"), Some(&key));
        let text = serde_json::to_string(&to_disk(&store, Some(&key))).unwrap();
        let sealed = serde_json::from_str::<DiskStore>(&text).unwrap().accounts[0].sealed_refresh_token.clone();

        // Password store locked or key missing: the account needs signing in
        // again, but saving must not throw the encrypted token away.
        let (locked, _) = from_disk(serde_json::from_str(&text).unwrap(), None);
        assert_eq!(locked.accounts[0].refresh_token, None);
        let saved = to_disk(&locked, None);
        assert_eq!(saved.accounts[0].sealed_refresh_token, sealed);

        // A different key can't open it either.
        let (wrong, _) = from_disk(serde_json::from_str(&text).unwrap(), Some(&[9u8; 32]));
        assert_eq!(wrong.accounts[0].refresh_token, None);
    }

    #[test]
    fn offline_accounts_have_nothing_secret() {
        let key = [3u8; 32];
        let store = AccountStore { active: None, accounts: vec![offline_account("Alex_1").unwrap()] };
        let text = serde_json::to_string(&to_disk(&store, Some(&key))).unwrap();
        assert!(!text.contains("efreshToken"));
    }
}

#[cfg(test)]
mod login_tests {
    use super::*;

    /// A waiting sign-in must stop soon after Cancel, before it asks Microsoft anything.
    #[tokio::test]
    async fn cancel_stops_a_waiting_sign_in() {
        let code = DeviceCode {
            device_code: "unused".into(),
            user_code: "ABCD1234".into(),
            verification_uri: "https://www.microsoft.com/link".into(),
            expires_in: 900,
            interval: 5,
        };
        let waiting = tokio::spawn(async move { poll_device_code(&code).await });
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        let began = std::time::Instant::now();
        cancel_login();
        let result = tokio::time::timeout(std::time::Duration::from_secs(3), waiting).await.expect("should stop quickly").unwrap();
        assert_eq!(result.unwrap_err().to_string(), "Sign-in cancelled.");
        assert!(began.elapsed() < std::time::Duration::from_secs(2));
    }
}
