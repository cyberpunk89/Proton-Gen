//! protongen — a GUI to build Steam / umu-launcher commands for CachyOS Proton.
//!
//! Read-only by default: it scans installed Proton runtimes, Steam games,
//! non-Steam shortcuts and sideloaded Heroic games, lets you toggle common
//! env-vars / wrappers, and previews + copies the resulting launch command. It
//! never writes to Steam config files.
//!
//! Three sanctioned writes outside protongen's own `state.toml`:
//! - [`heroic::inject`]: Heroic reads structured per-game JSON rather than a
//!   launch string, so applying tweaks means writing them into its config
//!   (backing up first, preserving every key it doesn't own).
//! - [`optiscaler_upgrade::fetch_and_extract`]: fetches the latest OptiScaler
//!   release and extracts it into a *game's* install directory, at the user's
//!   explicit per-click request. Never automatic, never executes anything —
//!   see that module's doc comment for the full rationale.
//! - [`mangohud_export::write_system_config`]: writes the overlay built in
//!   protongen's MangoHud builder into the real, system-wide `MangoHud.conf`
//!   (backing up first, preserving every line it doesn't own), so it becomes
//!   the default for every MangoHud-enabled program, not just this app's own
//!   generated command.
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
mod heroic;
mod ipc;
mod lint;
mod llm;
mod mangohud_export;
mod optiscaler_upgrade;
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
            ipc::inject_heroic,
            ipc::heroic_running,
            ipc::parse_command,
            ipc::explain_command,
            ipc::launch_diff,
            ipc::launch_statuses,
            ipc::apply_recipe,
            ipc::preview_recipe,
            ipc::lint,
            ipc::protondb_url,
            ipc::protondb_fetch,
            ipc::game_art,
            ipc::read_proton_log,
            ipc::llm_analyze,
            ipc::llm_troubleshoot,
            ipc::llm_models,
            ipc::save_store,
            ipc::check_for_update,
            ipc::run_update,
            ipc::optiscaler_status,
            ipc::optiscaler_latest,
            ipc::optiscaler_fetch,
            ipc::export_mangohud_system,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Print discovered Steam install, Proton runtimes, games and catalog summary.
/// Mirrors the old `--list` mode for verification and scripting.
pub fn dump() -> Result<()> {
    // Same configured paths the app uses, so `--list` is the answer to "why
    // isn't my Settings path working?" rather than a second, disagreeing view.
    let paths = store::Store::load().paths;
    let mut warnings = Vec::new();

    let dir = steam::locate_native(&paths.steam_roots, &mut warnings)?;
    println!("Steam root: {}", steam::root_display(&dir));

    let runtimes = runtime::discover(&dir, &paths.proton_dirs, &mut warnings);
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

    let games = games::list_games(&dir, &paths.steam_libraries, &mut warnings);
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

    if !warnings.is_empty() {
        println!("\nConfigured paths protongen could not use:");
        for w in &warnings {
            println!("  - {} {}: {}", w.file, w.path, w.error);
        }
    }

    let (cat, cat_warning) = params::Catalog::load();
    if let Some(w) = &cat_warning {
        println!("\nWARNING: {} at {} failed to parse; using the bundled catalog.\n  {}",
            w.file, w.path, w.error);
    }
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
