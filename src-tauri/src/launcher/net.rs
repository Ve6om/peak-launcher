use crate::error::{Error, Result};
use futures::{StreamExt, stream};
use sha1::{Digest, Sha1};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

static CLIENT: RwLock<Option<reqwest::Client>> = RwLock::new(None);
static CONCURRENCY: AtomicUsize = AtomicUsize::new(24);

/// Network options from settings (timeouts, proxy, parallel downloads).
pub struct NetConfig {
    pub timeout_secs: u64,
    pub concurrency: usize,
    /// e.g. "http://host:8080" or "socks5://host:1080"
    pub proxy: Option<String>,
}

fn build(cfg: &NetConfig) -> Result<reqwest::Client> {
    let mut b = reqwest::Client::builder()
        .user_agent(concat!("PeakLauncher/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(cfg.timeout_secs.max(5)))
        .read_timeout(Duration::from_secs(cfg.timeout_secs.max(5) * 2));
    if let Some(p) = &cfg.proxy {
        b = b.proxy(reqwest::Proxy::all(p).map_err(|e| Error::msg(format!("Invalid proxy: {e}")))?);
    }
    Ok(b.build()?)
}

pub fn configure(cfg: &NetConfig) -> Result<()> {
    let client = build(cfg)?;
    *CLIENT.write().unwrap() = Some(client);
    CONCURRENCY.store(cfg.concurrency.clamp(1, 64), Ordering::Relaxed);
    Ok(())
}

pub fn client() -> reqwest::Client {
    if let Some(c) = CLIENT.read().unwrap().as_ref() {
        return c.clone();
    }
    let c = build(&NetConfig { timeout_secs: 30, concurrency: 24, proxy: None }).expect("failed to build HTTP client");
    *CLIENT.write().unwrap() = Some(c.clone());
    c
}

pub fn concurrency() -> usize {
    CONCURRENCY.load(Ordering::Relaxed)
}

pub async fn get_json<T: serde::de::DeserializeOwned>(url: &str) -> Result<T> {
    Ok(client().get(url).send().await?.error_for_status()?.json::<T>().await?)
}

pub async fn sha1_file(path: &Path) -> Result<String> {
    let path = path.to_owned();
    tokio::task::spawn_blocking(move || -> Result<String> {
        use std::io::Read;
        let mut f = std::fs::File::open(&path)?;
        let mut hasher = Sha1::new();
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = f.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        Ok(hex::encode(hasher.finalize()))
    })
    .await
    .map_err(|e| Error::msg(e.to_string()))?
}

#[derive(Debug, Clone)]
pub struct Download {
    pub url: String,
    pub path: PathBuf,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    pub executable: bool,
}

impl Download {
    pub fn new(url: impl Into<String>, path: PathBuf, sha1: Option<String>, size: Option<u64>) -> Self {
        Self { url: url.into(), path, sha1, size, executable: false }
    }
}

/// Is the file already present and valid? With a hash we verify it; without
/// one we trust an existing file whose size matches.
async fn is_valid(d: &Download) -> bool {
    let Ok(meta) = tokio::fs::metadata(&d.path).await else { return false };
    if let Some(size) = d.size {
        if meta.len() != size {
            return false;
        }
    }
    match &d.sha1 {
        Some(expected) => sha1_file(&d.path).await.map(|h| h.eq_ignore_ascii_case(expected)).unwrap_or(false),
        None => true,
    }
}

async fn download_one(d: &Download) -> Result<u64> {
    if is_valid(d).await {
        return Ok(0);
    }
    if let Some(parent) = d.path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut last_err = None;
    for attempt in 0..3u64 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(500 * attempt)).await;
        }
        match fetch_to(d).await {
            Ok(n) => return Ok(n),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| Error::msg("download failed")))
}

async fn fetch_to(d: &Download) -> Result<u64> {
    use tokio::io::AsyncWriteExt;
    let resp = client().get(&d.url).send().await?.error_for_status()?;
    let tmp = d.path.with_extension("part");
    let mut file = tokio::fs::File::create(&tmp).await?;
    let mut hasher = Sha1::new();
    let mut written = 0u64;
    let mut body = resp.bytes_stream();
    while let Some(chunk) = body.next().await {
        let chunk = chunk?;
        hasher.update(&chunk);
        file.write_all(&chunk).await?;
        written += chunk.len() as u64;
    }
    file.flush().await?;
    drop(file);
    if let Some(expected) = &d.sha1 {
        let got = hex::encode(hasher.finalize());
        if !got.eq_ignore_ascii_case(expected) {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(Error::msg(format!("Checksum mismatch for {}", d.url)));
        }
    }
    tokio::fs::rename(&tmp, &d.path).await?;
    #[cfg(unix)]
    if d.executable {
        use std::os::unix::fs::PermissionsExt;
        let _ = tokio::fs::set_permissions(&d.path, std::fs::Permissions::from_mode(0o755)).await;
    }
    Ok(written)
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
    pub bytes: u64,
}

/// Download many files in parallel, skipping ones that are already valid.
pub async fn download_all<F>(files: Vec<Download>, concurrency: usize, on_progress: F) -> Result<()>
where
    F: Fn(Progress) + Send + Sync + 'static,
{
    let total = files.len();
    let done = Arc::new(AtomicUsize::new(0));
    let bytes = Arc::new(AtomicU64::new(0));
    let on_progress = Arc::new(on_progress);
    on_progress(Progress { done: 0, total, bytes: 0 });

    let results: Vec<Result<()>> = stream::iter(files)
        .map(|d| {
            let done = done.clone();
            let bytes = bytes.clone();
            let on_progress = on_progress.clone();
            async move {
                let n = download_one(&d).await?;
                let b = bytes.fetch_add(n, Ordering::Relaxed) + n;
                let c = done.fetch_add(1, Ordering::Relaxed) + 1;
                // Throttle UI updates for big batches (thousands of assets).
                if c == total || c % 16 == 0 {
                    on_progress(Progress { done: c, total, bytes: b });
                }
                Ok(())
            }
        })
        .buffer_unordered(concurrency)
        .collect()
        .await;

    results.into_iter().collect::<Result<Vec<_>>>()?;
    Ok(())
}
