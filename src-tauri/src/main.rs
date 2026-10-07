// Prevents an extra console window on Windows in release; harmless elsewhere.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::Result;

fn main() -> Result<()> {
    // Non-interactive dump mode: `protongen --list` prints discovered runtimes,
    // games and catalog size, then exits. Useful for verification and scripting.
    // `--json` / `--game-config` / `--catalog` print machine-readable JSON
    // instead (see `cli`), for scripts and for Nexus.
    let args: Vec<String> = std::env::args().collect();
    match protongen_lib::cli::parse(&args) {
        Ok(Some(protongen_lib::cli::Request::Dump)) => return protongen_lib::dump(),
        Ok(Some(req)) => return protongen_lib::cli::run(req).map_err(anyhow::Error::msg),
        Ok(None) => {}
        Err(e) => anyhow::bail!("{e}"),
    }

    // `protongen --game <appid>`: open straight on that game (Nexus's "Tune in
    // protongen" passes a Steam appid, or the hashed id of a Heroic game).
    protongen_lib::run_with(protongen_lib::game_arg(&args));
    Ok(())
}
