//! Find the broken mod: turn off half the suspected mods, play, say whether
//! it crashed, and repeat until one mod is left.
//!
//! The first test turns every mod off, so a crash that isn't caused by a mod
//! is caught instead of blamed on one. After that, turning a mod off also
//! turns off every mod that needs it, so a test never crashes just because a
//! dependency is missing. Mods others need (libraries) are kept on as long as
//! possible, since turning them off takes their dependents with them.
//!
//! The search is saved in the instance folder, so it carries on after Peak
//! restarts. Stopping it puts every mod back the way it was.

use super::moddeps::{installed_mods, Installed};
use super::modrinth::{content_dir, Kind};
use super::paths::Paths;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

const FILE: &str = "peak-mod-search.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    /// Testing with every mod off.
    AllOff,
    /// Testing with some of the suspects off.
    Halves,
    /// Down to one mod (or a few that can't be split).
    Found,
    /// It crashed with every mod off: not a mod.
    NotAMod,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Search {
    started_at: String,
    /// Mods that were on when it started, by file name (without ".disabled").
    /// Only these are ever switched; mods already off are left alone.
    was_on: Vec<String>,
    names: BTreeMap<String, String>,
    /// Mod file -> mods that need it (directly), among `was_on`.
    needed_by: BTreeMap<String, Vec<String>>,
    phase: Phase,
    /// Mods one of which is the cause.
    suspects: Vec<String>,
    /// Mods off for the current test.
    off: Vec<String>,
    /// Tests answered so far.
    step: u32,
}

/// What the app shows.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub phase: Phase,
    /// The test being run now (1-based).
    pub test: u32,
    /// Roughly how many tests it takes in all.
    pub tests: u32,
    pub mods: usize,
    pub suspects: usize,
    pub off: usize,
    /// When found: the mod(s), by display name and file.
    pub culprits: Vec<Named>,
    /// When found: mods that need the culprit, which go off with it.
    pub needs_it: Vec<Named>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Named {
    pub file_name: String,
    pub name: String,
}

fn state_file(paths: &Paths, inst: &str) -> PathBuf {
    super::instances::game_dir(paths, inst).join(FILE)
}

fn base(file_name: &str) -> &str {
    file_name.strip_suffix(".disabled").unwrap_or(file_name)
}

fn load(paths: &Paths, inst: &str) -> Option<Search> {
    std::fs::read(state_file(paths, inst)).ok().and_then(|b| serde_json::from_slice(&b).ok())
}

fn store(paths: &Paths, inst: &str, s: &Search) -> Result<()> {
    std::fs::write(state_file(paths, inst), serde_json::to_vec_pretty(s)?)?;
    Ok(())
}

/// Every mod that stops working if `start` is off: itself and, over and
/// over, whatever needs those.
fn with_dependents(start: &str, needed_by: &BTreeMap<String, Vec<String>>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut todo = vec![start.to_string()];
    while let Some(m) = todo.pop() {
        if out.insert(m.clone()) {
            todo.extend(needed_by.get(&m).into_iter().flatten().cloned());
        }
    }
    out
}

/// Pick the mods to turn off next: about half the suspects, each with what
/// needs it. Mods that take the fewest others with them go first, so
/// libraries stay on. None when the suspects can't be split (they all need
/// each other).
fn split(suspects: &[String], needed_by: &BTreeMap<String, Vec<String>>) -> Option<BTreeSet<String>> {
    let all: BTreeSet<&String> = suspects.iter().collect();
    let mut groups: Vec<BTreeSet<String>> = suspects.iter().map(|m| with_dependents(m, needed_by)).collect();
    groups.sort_by_key(|g| g.iter().filter(|m| all.contains(m)).count());
    let target = suspects.len() / 2;
    let mut off = BTreeSet::new();
    let count = |set: &BTreeSet<String>| set.iter().filter(|m| all.contains(m)).count();
    for g in groups {
        if count(&off) >= target.max(1) {
            break;
        }
        let mut next = off.clone();
        next.extend(g);
        // Always leave at least one suspect on.
        if count(&next) < suspects.len() {
            off = next;
        }
    }
    (count(&off) > 0).then_some(off)
}

/// Turn the mods on or off for the current test: everything that was on,
/// except `off`.
fn apply(dir: &Path, s: &Search) -> Result<()> {
    let off: BTreeSet<&String> = s.off.iter().collect();
    for m in &s.was_on {
        set_on(dir, m, !off.contains(m))?;
    }
    Ok(())
}

fn set_on(dir: &Path, file: &str, on: bool) -> Result<()> {
    let (plain, disabled) = (dir.join(file), dir.join(format!("{file}.disabled")));
    let (from, to) = if on { (disabled, plain) } else { (plain, disabled) };
    if from.exists() && !to.exists() {
        std::fs::rename(from, to)?;
    }
    Ok(())
}

fn tests_left(suspects: usize) -> u32 {
    (suspects.max(1) as f64).log2().ceil() as u32
}

fn view(s: &Search) -> View {
    let named = |f: &String| Named { file_name: f.clone(), name: s.names.get(f).cloned().unwrap_or_else(|| f.clone()) };
    let (culprits, needs_it) = if s.phase == Phase::Found {
        let mut needs: BTreeSet<String> = BTreeSet::new();
        for c in &s.suspects {
            needs.extend(with_dependents(c, &s.needed_by));
        }
        for c in &s.suspects {
            needs.remove(c);
        }
        (s.suspects.iter().map(named).collect(), needs.iter().map(named).collect())
    } else {
        (vec![], vec![])
    };
    let test = s.step + 1;
    let remaining = match s.phase {
        Phase::AllOff => 1 + tests_left(s.suspects.len()),
        Phase::Halves => tests_left(s.suspects.len()),
        _ => 0,
    };
    View {
        phase: s.phase,
        test,
        tests: s.step + remaining,
        mods: s.was_on.len(),
        suspects: s.suspects.len(),
        off: s.off.len(),
        culprits,
        needs_it,
    }
}

/// The search going on in an instance, if any.
pub fn current(paths: &Paths, inst: &str) -> Option<View> {
    load(paths, inst).map(|s| view(&s))
}

fn build(mods: &[Installed]) -> Search {
    let on: Vec<&Installed> = mods.iter().filter(|m| m.enabled).collect();
    let mut providers: HashMap<&str, Vec<String>> = HashMap::new();
    for m in &on {
        for p in &m.provides {
            providers.entry(p.as_str()).or_default().push(base(&m.file_name).to_string());
        }
    }
    let mut needed_by: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for m in &on {
        let me = base(&m.file_name).to_string();
        for r in &m.requires {
            for p in providers.get(r.as_str()).into_iter().flatten() {
                if *p != me {
                    let list = needed_by.entry(p.clone()).or_default();
                    if !list.contains(&me) {
                        list.push(me.clone());
                    }
                }
            }
        }
    }
    let was_on: Vec<String> = on.iter().map(|m| base(&m.file_name).to_string()).collect();
    Search {
        started_at: chrono::Utc::now().to_rfc3339(),
        names: on.iter().map(|m| (base(&m.file_name).to_string(), m.name.clone())).collect(),
        needed_by,
        phase: Phase::AllOff,
        suspects: was_on.clone(),
        off: was_on.clone(),
        was_on,
        step: 0,
    }
}

/// Start a search: the first test runs with every mod off.
pub fn start(paths: &Paths, inst: &str) -> Result<View> {
    if load(paths, inst).is_some() {
        return Err(Error::msg("A search is already going on in this instance."));
    }
    let dir = content_dir(paths, inst, Kind::Mod);
    let s = build(&installed_mods(&dir));
    if s.was_on.len() < 2 {
        return Err(Error::msg("This instance needs at least two mods turned on to search through."));
    }
    apply(&dir, &s)?;
    store(paths, inst, &s)?;
    Ok(view(&s))
}

/// Move a search on by one answer.
fn advance(s: &mut Search, crashed: bool) {
    s.step += 1;
    match s.phase {
        Phase::AllOff if crashed => {
            s.phase = Phase::NotAMod;
            s.off.clear();
            return;
        }
        Phase::AllOff => {}
        Phase::Halves => {
            let off: BTreeSet<&String> = s.off.iter().collect();
            // Crashed: the cause is among the suspects that were on.
            // Worked: it's among the ones that were off.
            s.suspects.retain(|m| off.contains(m) != crashed);
        }
        Phase::Found | Phase::NotAMod => return,
    }
    match (s.suspects.len() > 1).then(|| split(&s.suspects, &s.needed_by)).flatten() {
        Some(off) => {
            s.phase = Phase::Halves;
            s.off = off.into_iter().collect();
        }
        None => {
            s.phase = Phase::Found;
            s.off.clear();
        }
    }
}

/// Answer the current test: did the game crash?
pub fn answer(paths: &Paths, inst: &str, crashed: bool) -> Result<View> {
    let mut s = load(paths, inst).ok_or_else(|| Error::msg("There's no search going on in this instance."))?;
    advance(&mut s, crashed);
    let dir = content_dir(paths, inst, Kind::Mod);
    apply(&dir, &s)?;
    store(paths, inst, &s)?;
    Ok(view(&s))
}

/// End the search: every mod back as it was, except, with `keep_off`, the
/// mod that was found and the mods that need it.
pub fn finish(paths: &Paths, inst: &str, keep_off: bool) -> Result<()> {
    let Some(mut s) = load(paths, inst) else { return Ok(()) };
    s.off = if keep_off && s.phase == Phase::Found {
        s.suspects.iter().flat_map(|c| with_dependents(c, &s.needed_by)).collect::<BTreeSet<_>>().into_iter().collect()
    } else {
        vec![]
    };
    apply(&content_dir(paths, inst, Kind::Mod), &s)?;
    std::fs::remove_file(state_file(paths, inst))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn m(file: &str, provides: &[&str], requires: &[&str]) -> Installed {
        Installed {
            file_name: file.into(),
            name: file.trim_end_matches(".jar").into(),
            enabled: true,
            provides: provides.iter().map(|s| s.to_string()).collect::<HashSet<_>>(),
            requires: requires.iter().map(|s| s.to_string()).collect::<HashSet<_>>(),
        }
    }

    fn pack() -> Vec<Installed> {
        let mut mods = vec![
            m("api.jar", &["fabric-api"], &[]),
            m("lib.jar", &["lib"], &[]),
            m("uses-lib.jar", &["uses-lib"], &["lib"]),
            m("addon.jar", &["addon"], &["uses-lib", "fabric-api"]),
        ];
        for i in 0..12 {
            mods.push(m(&format!("mod{i:02}.jar"), &[&format!("mod{i}")], &["fabric-api"]));
        }
        mods
    }

    /// Run a search where `bad` crashes the game when it's on.
    fn run(mods: &[Installed], bad: Option<&str>) -> Search {
        let mut s = build(mods);
        for _ in 0..30 {
            if matches!(s.phase, Phase::Found | Phase::NotAMod) {
                return s;
            }
            let on: BTreeSet<&String> = s.was_on.iter().filter(|m| !s.off.contains(m)).collect();
            // Never test with a mod on and something it needs off.
            for m in &on {
                for needer in s.needed_by.iter().filter(|(_, v)| v.contains(m)).map(|(k, _)| k) {
                    assert!(on.contains(needer), "{m} is on without {needer}");
                }
            }
            let crashed = match bad {
                Some(b) => on.contains(&b.to_string()),
                None => true,
            };
            advance(&mut s, crashed);
        }
        panic!("didn't finish");
    }

    #[test]
    fn finds_any_single_broken_mod() {
        let mods = pack();
        for bad in mods.iter().map(|m| m.file_name.clone()) {
            let s = run(&mods, Some(&bad));
            assert_eq!(s.phase, Phase::Found, "{bad}");
            assert_eq!(s.suspects, vec![bad.clone()]);
            assert!(s.step <= 8, "{bad} took {} tests", s.step);
        }
    }

    #[test]
    fn crash_with_no_mods_is_not_a_mod() {
        let s = run(&pack(), None);
        assert_eq!((s.phase, s.step), (Phase::NotAMod, 1));
    }

    #[test]
    fn found_mod_takes_what_needs_it() {
        let mut s = run(&pack(), Some("lib.jar"));
        s.phase = Phase::Found;
        let v = view(&s);
        let needs: Vec<&str> = v.needs_it.iter().map(|n| n.file_name.as_str()).collect();
        assert_eq!(needs, vec!["addon.jar", "uses-lib.jar"]);
    }

    #[test]
    fn switches_files_and_puts_them_back() {
        let root = std::env::temp_dir().join(format!("peak-bisect-{}", uuid::Uuid::new_v4()));
        let paths = Paths::new(root.clone());
        let dir = content_dir(&paths, "a", Kind::Mod);
        std::fs::create_dir_all(&dir).unwrap();
        for f in ["one.jar", "two.jar", "three.jar"] {
            std::fs::write(dir.join(f), b"").unwrap();
        }
        std::fs::write(dir.join("old.jar.disabled"), b"").unwrap();

        let v = start(&paths, "a").unwrap();
        assert_eq!((v.mods, v.test), (3, 1));
        assert!(dir.join("one.jar.disabled").exists() && dir.join("old.jar.disabled").exists());
        let v = answer(&paths, "a", false).unwrap();
        assert_eq!(v.phase, Phase::Halves);
        assert_eq!(std::fs::read_dir(&dir).unwrap().filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().ends_with(".disabled")).count(), 1 + v.off);

        finish(&paths, "a", false).unwrap();
        for f in ["one.jar", "two.jar", "three.jar", "old.jar.disabled"] {
            assert!(dir.join(f).exists(), "{f}");
        }
        assert!(current(&paths, "a").is_none());
        let _ = std::fs::remove_dir_all(root);
    }
}
