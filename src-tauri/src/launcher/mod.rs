pub mod accounts;
pub mod apicache;
pub mod appimage;
pub mod curseforge;
pub mod export;
pub mod ftb;
pub mod importer;
pub mod install;
pub mod instances;
pub mod java;
pub mod jvmflags;
pub mod launch;
pub mod loaders;
pub mod manual;
pub mod moddeps;
pub mod msbrowser;
pub mod modpacks;
pub mod modrinth;
pub mod net;
pub mod paths;
pub mod technic;
pub mod rules;
pub mod secrets;
pub mod servers;
pub mod serverpack;
pub mod settings;
pub mod skins;
pub mod versions;

#[cfg(test)]
mod tests {
    /// Modrinth: install a mod with a required dependency, list, toggle,
    /// check updates. Run with:
    /// cargo test -- --ignored --nocapture modrinth_flow
    #[tokio::test]
    #[ignore]
    async fn modrinth_flow() {
        use super::modrinth::{self, Kind};
        let dir = std::env::temp_dir().join(format!("peak-mr-test-{}", std::process::id()));
        let paths = super::paths::Paths::new(dir.clone());
        let inst = super::instances::create(&paths, "Mods", "1.21.4", "fabric", Some("0.16.9".into())).await.unwrap();

        let found = modrinth::search(&paths, &inst.id, Kind::Mod, "mod menu", "relevance", 0).await.unwrap();
        println!("search hits: {} (first: {})", found.total_hits, found.hits[0].title);
        assert!(found.hits.iter().any(|h| h.slug == "modmenu"));

        // Mod Menu requires Fabric API, which must come along automatically.
        let installed = modrinth::install(&paths, &inst.id, Kind::Mod, "mOgUt4GM", None).await.unwrap();
        println!("installed: {installed:?}");
        let items = modrinth::list(&paths, &inst.id, Kind::Mod).await.unwrap();
        for i in &items {
            println!("  {} {} [{}] enabled={}", i.title, i.version_number.clone().unwrap_or_default(), i.file_name, i.enabled);
        }
        assert!(items.iter().any(|i| i.project_id.as_deref() == Some("P7dR8mSH")), "Fabric API dependency missing");

        let first = items[0].file_name.clone();
        modrinth::set_enabled(&paths, &inst.id, Kind::Mod, &first, false).await.unwrap();
        let items = modrinth::list(&paths, &inst.id, Kind::Mod).await.unwrap();
        assert!(items.iter().any(|i| !i.enabled), "disable failed");
        assert!(items.iter().all(|i| i.project_id.is_some()), "disabled file lost its metadata");

        let updates = modrinth::check_updates(&paths, &inst.id, Kind::Mod).await.unwrap();
        println!("updates: {}", updates.len());

        let shaders = modrinth::search(&paths, &inst.id, Kind::Shader, "", "downloads", 0).await.unwrap();
        println!("shaders: {} (first: {})", shaders.total_hits, shaders.hits[0].title);
        let _ = std::fs::remove_dir_all(dir);
    }

    use super::*;
    use std::sync::Arc;

    /// End-to-end check against Mojang's servers. Run with:
    /// cargo test -- --ignored --nocapture install_and_plan
    #[tokio::test]
    #[ignore]
    async fn install_and_plan() {
        let version = std::env::var("PEAK_TEST_VERSION").unwrap_or_else(|_| "1.21.4".into());
        let root = std::path::PathBuf::from(std::env::var("APPDATA").unwrap()).join("PeakLauncher");
        let paths = paths::Paths::new(root);
        let last = Arc::new(std::sync::Mutex::new(String::new()));
        let l2 = last.clone();
        let progress: install::ProgressFn = Arc::new(move |p| {
            let s = format!("{} {}/{}", p.stage, p.done, p.total);
            let mut g = l2.lock().unwrap();
            if g.split(' ').next() != s.split(' ').next() || p.done == p.total {
                println!("{s}");
            }
            *g = s;
        });
        install::install_version(&paths, &version, progress.clone()).await.expect("install");

        // Optional mod loader: PEAK_TEST_LOADER=fabric|quilt|forge|neoforge
        let loader_name = std::env::var("PEAK_TEST_LOADER").unwrap_or_else(|_| "vanilla".into());
        let loader = loaders::Loader::parse(&loader_name).unwrap();
        let loader_version = match std::env::var("PEAK_TEST_LOADER_VERSION") {
            Ok(v) => v,
            Err(_) => loaders::versions(loader, &version)
                .await
                .expect("loader versions")
                .into_iter()
                .find(|v| v.recommended)
                .map(|v| v.id)
                .unwrap_or_default(),
        };
        println!("loader: {loader_name} {loader_version}");
        let id = loaders::install(&paths, loader, &version, &loader_version, &progress).await.expect("loader install");
        let v = versions::resolve(&paths, &id).await.expect("resolve");
        println!("profile: {id} main: {}", v.main_class);

        let inst = instances::create(&paths, "Test", &version, &loader_name, Some(loader_version)).await.unwrap();
        let account = accounts::offline_account("PeakTester").unwrap();
        let plan = launch::plan(&paths, &inst, &v, &account, &settings::Settings::default()).await.expect("plan");
        instances::delete(&paths, &inst.id).await.unwrap();

        // Every classpath entry must exist on disk.
        let cp_idx = plan.args.iter().position(|a| a == "-cp").expect("-cp");
        let sep = if cfg!(windows) { ';' } else { ':' };
        let missing: Vec<_> = plan.args[cp_idx + 1].split(sep).filter(|p| !std::path::Path::new(p).exists()).collect();
        assert!(missing.is_empty(), "missing classpath entries: {missing:?}");
        assert!(plan.args.contains(&v.main_class));
        assert!(!plan.args.iter().any(|a| a.contains("${")), "unsubstituted: {:?}", plan.args.iter().filter(|a| a.contains("${")).collect::<Vec<_>>());

        let out = std::process::Command::new(java::console_java(&plan.java))
            .arg("-version")
            .output()
            .expect("run java");
        println!("{}", String::from_utf8_lossy(&out.stderr));
        assert!(out.status.success());
        println!("args: {}", plan.args.len());
    }
}
