//! Extra speed for the Optimize dialog: well-known performance mods, added
//! only where they fit (right loader and Minecraft version, not already in
//! the pack, nothing installed that they clash with). What was added is
//! remembered so it can be taken out again with one click.

use super::instances;
use super::modrinth::{self, Kind, PlanItem};
use super::paths::Paths;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// A mod as it might appear in a pack: its title (lower case) or the start
/// of its file name.
struct Known {
    name: &'static str,
    titles: &'static [&'static str],
    files: &'static [&'static str],
}

const fn k(name: &'static str, titles: &'static [&'static str], files: &'static [&'static str]) -> Known {
    Known { name, titles, files }
}

const SODIUM: Known = k("Sodium", &["sodium"], &["sodium-fabric", "sodium-neoforge", "sodium-forge", "sodium-mc"]);
const EMBEDDIUM: Known = k("Embeddium", &["embeddium"], &["embeddium-"]);
const RUBIDIUM: Known = k("Rubidium", &["rubidium"], &["rubidium-"]);
const OPTIFINE: Known = k("OptiFine", &["optifine"], &["optifine"]);
const OPTIFABRIC: Known = k("OptiFabric", &["optifabric"], &["optifabric"]);
const CANVAS: Known = k("Canvas", &["canvas", "canvas renderer"], &["canvas-fabric", "canvas-mc"]);
const VULKANMOD: Known = k("VulkanMod", &["vulkanmod"], &["vulkanmod"]);
const CELERITAS: Known = k("Celeritas", &["celeritas"], &["celeritas-"]);
const RENDERERS: &[Known] = &[SODIUM, EMBEDDIUM, RUBIDIUM, OPTIFINE, OPTIFABRIC, CANVAS, VULKANMOD, CELERITAS];

struct Candidate {
    id: &'static str,
    what: &'static str,
    loaders: &'static [&'static str],
    own: Known,
    /// Other mods that already do this job or don't get along with it.
    clashes: &'static [Known],
}

const ALL: &[&str] = &["fabric", "quilt", "forge", "neoforge"];

const CANDIDATES: &[Candidate] = &[
    Candidate {
        id: "AANobbMI",
        what: "Much faster rendering, usually the biggest FPS boost",
        loaders: &["fabric", "quilt", "neoforge"],
        own: SODIUM,
        clashes: RENDERERS,
    },
    Candidate {
        id: "sk9rgfiA",
        what: "Faster rendering for Forge (a Sodium port)",
        loaders: &["forge"],
        own: EMBEDDIUM,
        clashes: RENDERERS,
    },
    Candidate {
        id: "gvQqBUqZ",
        what: "Faster game logic: mobs, redstone, chunks",
        loaders: &["fabric", "quilt", "neoforge"],
        own: k("Lithium", &["lithium"], &["lithium-"]),
        clashes: &[k("Canary", &["canary"], &["canary-"]), k("Radium", &["radium"], &["radium-"])],
    },
    Candidate {
        id: "uXXizFIs",
        what: "Uses less memory",
        loaders: ALL,
        own: k("FerriteCore", &["ferritecore"], &["ferritecore-"]),
        clashes: &[],
    },
    Candidate {
        id: "nmDcB62a",
        what: "Faster loading and less memory, best in big packs",
        loaders: &["fabric", "forge", "neoforge"],
        own: k("ModernFix", &["modernfix"], &["modernfix-"]),
        clashes: &[],
    },
    Candidate {
        id: "5ZwdcRci",
        what: "Faster drawing of text, items and the HUD",
        loaders: ALL,
        own: k("ImmediatelyFast", &["immediatelyfast"], &["immediatelyfast-"]),
        clashes: &[],
    },
    Candidate {
        id: "NNAgCjsB",
        what: "Skips drawing mobs and chests you can't see",
        loaders: ALL,
        own: k("Entity Culling", &["entity culling", "entityculling"], &["entityculling-"]),
        clashes: &[],
    },
    Candidate {
        id: "LQ3K71Q1",
        what: "Lowers FPS while the game is in the background, saving power",
        loaders: ALL,
        own: k("Dynamic FPS", &["dynamic fps", "dynamicfps"], &["dynamic-fps-", "dynamicfps-"]),
        clashes: &[k("FPS Reducer", &["fps reducer"], &["fpsreducer-"])],
    },
];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerfMod {
    pub project_id: String,
    pub title: String,
    pub what: String,
    /// "add" | "installed" (already in the pack) | "clash" | "unavailable"
    pub status: &'static str,
    /// Why it isn't offered, for the last three.
    pub note: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerfMods {
    pub mods: Vec<PerfMod>,
    /// Titles of mods added before that "Undo" would take out.
    pub added: Vec<String>,
}

#[derive(Default, Serialize, Deserialize)]
struct Added {
    #[serde(default)]
    mods: Vec<AddedMod>,
}

#[derive(Serialize, Deserialize)]
struct AddedMod {
    project_id: String,
    title: String,
}

const FILE: &str = "peak-perf-mods.json";

async fn load_added(paths: &Paths, inst: &str) -> Added {
    super::paths::read_json(&paths.instance(inst).join(FILE)).await.ok().flatten().unwrap_or_default()
}

async fn installed(paths: &Paths, inst: &str) -> Result<(HashSet<String>, Vec<(String, String)>)> {
    let items = modrinth::list(paths, inst, Kind::Mod).await?;
    let ids = items.iter().filter_map(|i| i.project_id.clone()).collect();
    let names = items.iter().map(|i| (i.title.to_lowercase(), i.file_name.to_lowercase())).collect();
    Ok((ids, names))
}

fn has(names: &[(String, String)], m: &Known) -> bool {
    names.iter().any(|(title, file)| m.titles.contains(&title.as_str()) || m.files.iter().any(|p| file.starts_with(p)))
}

pub async fn suggest(paths: &Paths, inst_id: &str) -> Result<PerfMods> {
    let inst = instances::get(paths, inst_id).await?;
    let added = load_added(paths, inst_id).await;
    let (ids, names) = installed(paths, inst_id).await?;
    let mut mods = Vec::new();
    for c in CANDIDATES.iter().filter(|c| c.loaders.contains(&inst.loader.as_str())) {
        let (status, note) = if ids.contains(c.id) || has(&names, &c.own) {
            ("installed", None)
        } else if let Some(other) = c.clashes.iter().find(|m| m.name != c.own.name && has(&names, m)) {
            ("clash", Some(format!("The pack already has {}", other.name)))
        } else {
            match modrinth::compatible_versions(&inst, c.id, Kind::Mod).await {
                Ok(v) if !v.is_empty() => ("add", None),
                Ok(_) => ("unavailable", Some(format!("No version for Minecraft {} with {}", inst.game_version, inst.loader))),
                Err(_) => ("unavailable", Some("Couldn't check Modrinth".into())),
            }
        };
        mods.push(PerfMod { project_id: c.id.into(), title: c.own.name.into(), what: c.what.into(), status, note });
    }
    let still: Vec<String> = added.mods.iter().filter(|m| ids.contains(&m.project_id)).map(|m| m.title.clone()).collect();
    Ok(PerfMods { mods, added: still })
}

/// Add the chosen mods (and anything they need). Returns their titles.
pub async fn add(paths: &Paths, inst_id: &str, project_ids: &[String]) -> Result<Vec<String>> {
    let offered = suggest(paths, inst_id).await?;
    let mut items: Vec<PlanItem> = Vec::new();
    for id in project_ids {
        if !offered.mods.iter().any(|m| &m.project_id == id && m.status == "add") {
            return Err(Error::msg("That mod can't be added to this instance."));
        }
        for it in modrinth::plan(paths, inst_id, Kind::Mod, id, None).await? {
            if !items.iter().any(|x| x.project_id == it.project_id) {
                items.push(it);
            }
        }
    }
    if items.is_empty() {
        return Ok(vec![]);
    }
    modrinth::install_items(paths, inst_id, &items).await?;
    let mut added = load_added(paths, inst_id).await;
    for it in &items {
        if !added.mods.iter().any(|m| m.project_id == it.project_id) {
            added.mods.push(AddedMod { project_id: it.project_id.clone(), title: it.title.clone() });
        }
    }
    super::paths::write_json(&paths.instance(inst_id).join(FILE), &added).await?;
    Ok(items.into_iter().map(|i| i.title).collect())
}

/// Take out everything `add` put in (that's still there). Returns how many.
pub async fn undo(paths: &Paths, inst_id: &str) -> Result<usize> {
    let added = load_added(paths, inst_id).await;
    let mut n = 0;
    for item in modrinth::list(paths, inst_id, Kind::Mod).await? {
        if item.project_id.as_ref().is_some_and(|p| added.mods.iter().any(|m| &m.project_id == p)) {
            modrinth::remove(paths, inst_id, Kind::Mod, &item.file_name).await?;
            n += 1;
        }
    }
    let _ = tokio::fs::remove_file(paths.instance(inst_id).join(FILE)).await;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[(&str, &str)]) -> Vec<(String, String)> {
        list.iter().map(|(t, f)| (t.to_lowercase(), f.to_lowercase())).collect()
    }

    #[test]
    fn spots_mods_by_title_or_file_without_mixing_up_add_ons() {
        let pack = names(&[("Sodium Extra", "sodium-extra-fabric-0.9.7+mc26.3.jar"), ("OptiFine_1.20.1_HD_U_I6", "OptiFine_1.20.1_HD_U_I6.jar")]);
        assert!(!has(&pack, &SODIUM), "Sodium Extra isn't Sodium");
        assert!(has(&pack, &OPTIFINE));
        let pack = names(&[("Sodium", "sodium-fabric-0.9.4-beta.2+mc26.3.jar")]);
        assert!(has(&pack, &SODIUM));
        assert!(has(&names(&[("x", "sodium-fabric-mc1.20.1-0.5.11.jar")]), &SODIUM), "older file names");
    }
}
