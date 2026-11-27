//! Share a game log as a link (mclo.gs), for asking for help.
//!
//! Before anything leaves the computer the log is cleaned: the player's home
//! folder (which holds their computer user name), their Minecraft name and
//! any access token are replaced. mclo.gs hides IP addresses itself.

use super::net;
use crate::error::{Error, Result};
use serde::Deserialize;

/// mclo.gs takes up to 25,000 lines and 10 MB; keep the end, where crashes are.
const MAX_LINES: usize = 25_000;
const MAX_BYTES: usize = 9 * 1024 * 1024;

/// Hide what identifies the player. `home` is their home folder, `names`
/// their Minecraft names.
pub fn clean(log: &str, home: Option<&str>, names: &[String]) -> String {
    let mut out = log.to_string();
    if let Some(home) = home.filter(|h| h.len() > 3) {
        // Both slash styles, since Java prints either on Windows.
        for h in [home.to_string(), home.replace('\\', "/"), home.replace('/', "\\")] {
            out = out.replace(&h, "~");
        }
    }
    // Tokens Peak or the game might print.
    let token = regex::Regex::new(r"(?i)(--accessToken\s+|accessToken[=:]\s*|Bearer\s+)[A-Za-z0-9._\-]{8,}").unwrap();
    out = token.replace_all(&out, "${1}[hidden]").into_owned();
    for name in names.iter().filter(|n| n.len() >= 3) {
        let word = regex::Regex::new(&format!(r"\b{}\b", regex::escape(name))).unwrap();
        out = word.replace_all(&out, "<player>").into_owned();
    }
    out
}

/// The end of a log, within mclo.gs's limits.
fn tail(log: &str) -> String {
    let lines: Vec<&str> = log.lines().collect();
    let mut kept = lines[lines.len().saturating_sub(MAX_LINES)..].join("\n");
    if kept.len() > MAX_BYTES {
        let cut = kept.len() - MAX_BYTES;
        let start = (cut..kept.len()).find(|&i| kept.is_char_boundary(i)).unwrap_or(kept.len());
        kept = kept[start..].to_string();
    }
    kept
}

#[derive(Deserialize)]
struct Answer {
    success: bool,
    url: Option<String>,
    error: Option<String>,
}

/// Upload a cleaned log. Returns its link.
pub async fn upload(log: &str) -> Result<String> {
    let content = tail(log);
    if content.trim().is_empty() {
        return Err(Error::msg("There's no log to share yet. Play the instance first."));
    }
    let res: Answer = net::client()
        .post("https://api.mclo.gs/1/log")
        .form(&[("content", content)])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    match (res.success, res.url) {
        (true, Some(url)) if url.starts_with("https://mclo.gs/") => Ok(url),
        _ => Err(Error::msg(format!("mclo.gs didn't take the log{}", res.error.map(|e| format!(": {e}")).unwrap_or_default()))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hides_who_the_player_is() {
        let log = "[Peak] Launching with C:\\Users\\alex\\AppData\\Roaming\\PeakLauncher\nLoading C:/Users/alex/AppData/x.jar\n\
                   Setting user: Steve_123\n--accessToken eyJhbGciOiJIUzI1NiJ9.abc.def --uuid x\nSteve_1234 joined";
        let out = clean(log, Some("C:\\Users\\alex"), &["Steve_123".into()]);
        assert!(!out.contains("alex"), "{out}");
        assert!(out.contains("~\\AppData") && out.contains("~/AppData"));
        assert!(out.contains("Setting user: <player>"));
        assert!(out.contains("Steve_1234 joined"), "only the whole name: {out}");
        assert!(out.contains("--accessToken [hidden]") && !out.contains("eyJ"));
    }

    #[test]
    fn keeps_the_end_of_long_logs() {
        let log: String = (0..30_000).map(|i| format!("line {i}\n")).collect();
        let t = tail(&log);
        assert_eq!(t.lines().count(), MAX_LINES);
        assert!(t.ends_with("line 29999"));
    }
}
