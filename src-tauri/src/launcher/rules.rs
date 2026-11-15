//! Mojang "rules" evaluation, used by libraries and launch arguments to
//! include/exclude entries per operating system, architecture and feature.

use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct Rule {
    pub action: String,
    #[serde(default)]
    pub os: Option<OsRule>,
    #[serde(default)]
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OsRule {
    pub name: Option<String>,
    pub arch: Option<String>,
}

/// Mojang's name for the current OS.
pub fn os_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

pub fn is_64bit() -> bool {
    cfg!(target_pointer_width = "64")
}

fn rule_matches(rule: &Rule, enabled: &[&str]) -> bool {
    if let Some(os) = &rule.os {
        if let Some(name) = &os.name {
            if name != os_name() {
                return false;
            }
        }
        if let Some(arch) = &os.arch {
            let ours = match std::env::consts::ARCH {
                "x86" => "x86",
                "aarch64" => "arm64",
                _ => "x86_64",
            };
            if arch != ours {
                return false;
            }
        }
    }
    if let Some(features) = &rule.features {
        // Features like custom resolution or quick play are only on when the
        // launcher enables them for this launch.
        if features.iter().any(|(name, &wanted)| enabled.contains(&name.as_str()) != wanted) {
            return false;
        }
    }
    true
}

/// Evaluate a rule list: no rules means allowed; otherwise the last matching
/// rule decides, defaulting to disallowed.
pub fn allowed(rules: Option<&[Rule]>) -> bool {
    allowed_with(rules, &[])
}

/// Like [`allowed`], with a set of enabled launch features.
pub fn allowed_with(rules: Option<&[Rule]>, features: &[&str]) -> bool {
    let Some(rules) = rules else { return true };
    if rules.is_empty() {
        return true;
    }
    let mut result = false;
    for rule in rules {
        if rule_matches(rule, features) {
            result = rule.action == "allow";
        }
    }
    result
}
