//! Peak's own release notes from GitHub, so the update prompt can show every
//! version since the one installed, not just the newest.

use super::net;
use crate::error::Result;
use serde::{Deserialize, Serialize};

const RELEASES: &str = "https://api.github.com/repos/Ve6om/peak-launcher/releases?per_page=50";

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseNote {
    pub version: String,
    pub date: Option<String>,
    pub notes: String,
}

/// "v0.1.29" / "0.1.29" as numbers, for comparing.
fn parse(v: &str) -> Option<Vec<u64>> {
    let v = v.trim().trim_start_matches('v');
    let core = v.split(['-', '+']).next()?;
    core.split('.').map(|p| p.parse().ok()).collect()
}

/// Releases newer than `from`, up to and including `to`, newest first.
fn between(all: Vec<GhRelease>, from: &str, to: &str) -> Vec<ReleaseNote> {
    let (Some(lo), Some(hi)) = (parse(from), parse(to)) else { return vec![] };
    let mut out: Vec<(Vec<u64>, ReleaseNote)> = all
        .into_iter()
        .filter(|r| !r.draft && !r.prerelease)
        .filter_map(|r| {
            let v = parse(&r.tag_name)?;
            (v > lo && v <= hi).then(|| {
                let notes = r.body.as_deref().map(str::trim).filter(|b| !b.is_empty()).unwrap_or("Bug fixes and improvements.").to_string();
                (v, ReleaseNote { version: r.tag_name.trim_start_matches('v').to_string(), date: r.published_at, notes })
            })
        })
        .collect();
    out.sort_by(|a, b| b.0.cmp(&a.0));
    out.into_iter().map(|(_, n)| n).collect()
}

pub async fn notes_between(from: &str, to: &str) -> Result<Vec<ReleaseNote>> {
    let all: Vec<GhRelease> = net::client()
        .get(RELEASES)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(between(all, from, to))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(tag: &str, body: &str) -> GhRelease {
        GhRelease { tag_name: tag.into(), body: Some(body.into()), published_at: None, draft: false, prerelease: false }
    }

    #[test]
    fn picks_the_versions_in_between() {
        let all = vec![r("v0.1.29", "c"), r("v0.1.30", "next"), r("v0.1.27", "a"), r("v0.1.28", "b"), r("v0.1.26", "old"), r("v0.1.10", "older")];
        let got: Vec<String> = between(all, "0.1.26", "0.1.29").into_iter().map(|n| n.version).collect();
        assert_eq!(got, vec!["0.1.29", "0.1.28", "0.1.27"]);
        let mut draft = r("v0.1.28", "x");
        draft.draft = true;
        assert!(between(vec![draft], "0.1.27", "0.1.29").is_empty());
        assert_eq!(between(vec![r("v0.1.9", "")], "0.1.8", "0.1.10")[0].notes, "Bug fixes and improvements.");
        assert!(parse("v0.1.10").unwrap() > parse("0.1.9").unwrap());
    }

    #[tokio::test]
    #[ignore]
    async fn releases_live() {
        for n in notes_between("0.1.25", "0.1.29").await.unwrap() {
            println!("{} {:?} {} chars", n.version, n.date, n.notes.len());
        }
    }
}
