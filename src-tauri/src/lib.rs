//! protongen — a GUI to build Steam / umu-launcher commands for Proton.
//!
//! Read-only by default: it scans installed Proton runtimes, Steam games,
//! non-Steam shortcuts and sideloaded Heroic games, lets you toggle common
//! env-vars / wrappers, and previews + copies the resulting launch command. It
//! never writes to Steam config files.
//!
//! Five sanctioned writes outside protongen's own `state.toml`:
//! - [`heroic::inject`]: Heroic reads structured per-game JSON rather than a
//!   launch string, so applying tweaks means writing them into its config
//!   (backing up first, preserving every key it doesn't own).
//! - [`optiscaler_upgrade::fetch_and_extract`]: fetches the latest OptiScaler
//!   release and extracts it over a *game's* existing manual install (onto
//!   its live proxy DLL), at the user's explicit per-click request. Never automatic, never executes anything —
//!   see that module's doc comment for the full rationale.
//! - [`mangohud_export::write_system_config`]: writes the overlay built in
//!   protongen's MangoHud builder into the real, system-wide `MangoHud.conf`
//!   (backing up first, preserving every line it doesn't own), so it becomes
//!   the default for every MangoHud-enabled program, not just this app's own
//!   generated command.
//! - [`vkbasalt_export::write_system_config`]: the same shape again, for the
//!   effect chain built in protongen's vkBasalt builder, written into the
//!   real, system-wide `vkBasalt.conf`.
//! - [`nexus::set_launch`]: hands a game's tuning to Nexus (the user's own
//!   launcher) by running `nexus-cli --set-launch`. protongen writes nothing
//!   itself; Nexus updates the files it owns for that game.
//!
//! This crate is a Tauri backend: the pure logic modules below are exposed to
//! the web frontend through `ipc`.

mod anticheat;
mod art;
mod builder;
pub mod cli;
mod compose;
mod conf_merge;
mod diff;
mod explain;
mod folders;
mod fsutil;
mod games;
mod hardware;
mod heroic;
mod ipc;
mod lint;
mod logs;
mod lsfg;
mod llm;
mod mangohud_export;
mod nexus;
mod optiscaler_upgrade;
mod params;
mod parser;
mod protondb;
mod recipes;
mod runtime;
mod runtime_updates;
mod steam;
mod steamcfg;
mod store;
mod update;
mod vkbasalt_export;
mod which;

use anyhow::Result;
use tauri::{Emitter, Manager};

/// Event the running window gets when `protongen --game <id>` is run again.
pub const OPEN_GAME_EVENT: &str = "open-game";

/// Payload of [`OPEN_GAME_EVENT`]; `app_id` is `None` for a bare `protongen`.
#[derive(Clone, serde::Serialize)]
struct OpenGame {
    app_id: Option<u32>,
}

/// The `--game <appid>` argument (a Steam appid, or protongen's hashed id for a
/// Heroic game — what Nexus's "Tune in protongen" passes). Shared by startup
/// and the single-instance handoff so both read the same flag the same way.
pub fn game_arg(args: &[String]) -> Option<u32> {
    args.iter()
        .position(|a| a == "--game")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.trim().parse::<u32>().ok())
}

/// Launch the Tauri application.
pub fn run() {
    run_with(None)
}

/// Launch the Tauri application, optionally opening on one game (`--game`).
pub fn run_with(initial_game: Option<u32>) {
    tauri::Builder::default()
        // Must be the first plugin. A second launch (Nexus's "Tune in
        // protongen" clicked again) hands its argv to this process and exits,
        // instead of opening a second window with its own copy of the store.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
            let app_id = game_arg(&argv);
            if let Some(id) = app_id {
                app.state::<ipc::AppState>().request_game(id);
            }
            let _ = app.emit(OPEN_GAME_EVENT, OpenGame { app_id });
        }))
        .plugin(tauri_plugin_opener::init())
        // Game art, by the key `game_art` registered. Asynchronous: the file is
        // read on a worker thread, never the UI thread — a grid of tiles
        // loading at once used to stall the window.
        .register_asynchronous_uri_scheme_protocol("art", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            let key = request.uri().path().trim_start_matches('/').to_string();
            tauri::async_runtime::spawn_blocking(move || {
                let path = app.state::<ipc::AppState>().art_files.lock().ok().and_then(|m| m.get(&key).cloned());
                responder.respond(art::response(path.as_deref()));
            });
        })
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(ipc::AppState::new().with_initial_game(initial_game))
        .invoke_handler(tauri::generate_handler![
            ipc::bootstrap,
            ipc::rescan,
            ipc::build_command,
            ipc::inject_heroic,
            ipc::apply_to_nexus,
            ipc::game_folders,
            ipc::open_game_folder,
            ipc::heroic_running,
            ipc::parse_command,
            ipc::explain_command,
            ipc::launch_diff,
            ipc::steam_user_config,
            ipc::launch_statuses,
            ipc::apply_recipe,
            ipc::preview_recipe,
            ipc::lint,
            ipc::protondb_url,
            ipc::protondb_fetch,
            ipc::anticheat_lookup,
            ipc::game_art,
            ipc::read_proton_log,
            ipc::llm_analyze,
            ipc::llm_troubleshoot,
            ipc::llm_models,
            ipc::save_store,
            ipc::check_for_update,
            ipc::run_update,
            ipc::check_runtime_updates,
            ipc::lsfg_status,
            ipc::optiscaler_status,
            ipc::optiscaler_latest,
            ipc::optiscaler_fetch,
            ipc::export_mangohud_system,
            ipc::vkbasalt_read_config,
            ipc::export_vkbasalt_system,
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
    if runtimes.is_empty() {
        println!(
            "  {}",
            runtime::no_runtimes_message(&steam::user_compat_tools_dir(&dir), &paths.proton_dirs)
        );
    }
    for r in &runtimes {
        println!(
            "  - {:<55} [{}]  internal: {}",
            r.display_name,
            r.kind.label(),
            r.internal_name
        );
    }

    let hw = hardware::detect();
    println!("\nDetected hardware: {}", hw.summary());
    for m in &hw.monitors {
        let hz = m.refresh_hz.map(|r| format!(" @ {r} Hz")).unwrap_or_default();
        println!("  display {}: {}x{}{hz}", m.connector, m.width, m.height);
    }

    let app_cfgs = steamcfg::current_app_cfgs(&dir, &mut warnings);
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

    let lossless = games
        .iter()
        .find(|g| g.app_id == lsfg::LOSSLESS_SCALING_APPID)
        .and_then(|g| g.install_dir.as_deref());
    let ls = lsfg::detect(lossless);
    println!("\nLossless Scaling frame generation (lsfg-vk):");
    match &ls.layer {
        Some(l) if l.legacy => println!("  layer: 1.x at {} — update to 2.x for LSFGVK_*", l.manifest),
        Some(l) => println!("  layer: v{} at {}", l.version, l.manifest),
        None => println!("  layer: not installed"),
    }
    println!(
        "  dll:   {}",
        match (&ls.dll, &ls.dll_source) {
            (Some(d), Some(src)) => format!("{d} (from {src})"),
            _ => "not found — install Lossless Scaling from Steam".to_string(),
        }
    );
    match (&ls.config_found, &ls.config_error) {
        (false, _) => println!("  config: none at {}", ls.config_path),
        (true, Some(e)) => println!("  config: {} failed to parse: {e}", ls.config_path),
        (true, None) => {
            println!("  config: {} ({} profiles)", ls.config_path, ls.profiles.len());
            for p in &ls.profiles {
                println!("    - {}", p.name);
            }
        }
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

#[cfg(test)]
mod tests {
    use super::game_arg;

    fn argv(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn game_arg_reads_the_value_after_the_flag() {
        assert_eq!(game_arg(&argv(&["protongen", "--game", "1245620"])), Some(1245620));
        assert_eq!(game_arg(&argv(&["protongen", "--game", "2147483905"])), Some(2147483905));
    }

    #[test]
    fn game_arg_ignores_a_missing_or_bad_value() {
        assert_eq!(game_arg(&argv(&["protongen"])), None);
        assert_eq!(game_arg(&argv(&["protongen", "--game"])), None);
        assert_eq!(game_arg(&argv(&["protongen", "--game", "half-life"])), None);
        assert_eq!(game_arg(&argv(&["protongen", "--game", "-1"])), None);
    }
}
