//! protongen — a GUI to build Steam / umu-launcher commands for CachyOS Proton.
//!
//! Read-only: it scans installed Proton runtimes, Steam games and non-Steam
//! shortcuts, lets you toggle common env-vars / wrappers, and previews + copies
//! the resulting launch command. It never writes to Steam config files.
//!
//! This crate is a Tauri backend: the pure logic modules below are exposed to
//! the web frontend through `ipc`.

mod art;
mod builder;
mod compose;
mod diff;
mod explain;
mod games;
mod hardware;
mod ipc;
mod lint;
mod params;
mod parser;
mod protondb;
mod recipes;
mod runtime;
mod steam;
mod steamcfg;
mod store;
mod update;
mod which;

use anyhow::Result;

/// Launch the Tauri application.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(ipc::AppState::new())
        .invoke_handler(tauri::generate_handler![
            ipc::bootstrap,
            ipc::rescan,
            ipc::build_command,
            ipc::parse_command,
            ipc::explain_command,
            ipc::launch_diff,
            ipc::apply_recipe,
            ipc::lint,
            ipc::protondb_url,
            ipc::protondb_fetch,
            ipc::game_art,
            ipc::save_store,
            ipc::check_for_update,
            ipc::run_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Print discovered Steam install, Proton runtimes, games and catalog summary.
/// Mirrors the old `--list` mode for verification and scripting.
pub fn dump() -> Result<()> {
    let dir = steam::locate_native()?;
    println!("Steam root: {}", steam::root_display(&dir));

    let runtimes = runtime::discover(&dir);
    println!("\nProton runtimes:");
    for r in &runtimes {
        println!(
            "  - {:<55} [{}]  internal: {}",
            r.display_name,
            r.kind.label(),
            r.internal_name
        );
    }

    println!("\nDetected hardware: {}", hardware::detect().summary());

    let app_cfgs = steamcfg::current_app_cfgs(&dir);
    let current = steamcfg::launch_options(&app_cfgs);
    println!("Games with existing launch options set: {}", current.len());
    println!(
        "Games with a recorded last-played time: {}",
        app_cfgs.values().filter(|c| c.last_played.is_some()).count()
    );

    let games = games::list_games(&dir);
    println!("\nGames + shortcuts ({}):", games.len());
    for g in &games {
        let state = if g.installed { "" } else { "  (not installed)" };
        println!(
            "  - {:<45} ({})  [{}]{}",
            g.name,
            g.app_id,
            g.source.label(),
            state
        );
    }

    let cat = params::Catalog::load();
    println!(
        "\nCatalog: {} wrappers, {} env vars across {} categories.",
        cat.wrappers.len(),
        cat.envs.len(),
        cat.categories().len()
    );

    if let (Some(catalog_build), Some(installed)) = (
        cat.meta.proton_cachyos_build.as_deref(),
        runtime::installed_cachyos_build(&runtimes).as_deref(),
    ) {
        if installed > catalog_build {
            let when = cat.meta.updated.as_deref().unwrap_or("?");
            println!(
                "\n⚠ catalog stale: proton-cachyos {installed} installed, catalog refreshed for {catalog_build} ({when}).\n  Run /update-proton-params in Claude Code to refresh."
            );
        }
    }
    Ok(())
}
