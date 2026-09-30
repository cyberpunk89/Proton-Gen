// Prevents an extra console window on Windows in release; harmless elsewhere.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::Result;

fn main() -> Result<()> {
    // Non-interactive dump mode: `protongen --list` prints discovered runtimes,
    // games and catalog size, then exits. Useful for verification and scripting.
    if std::env::args().any(|a| a == "--list" || a == "--scan") {
        return protongen_lib::dump();
    }

    // `protongen --game <appid>`: open straight on that game (Nexus's "Tune in
    // protongen" passes a Steam appid, or the hashed id of a Heroic game).
    let args: Vec<String> = std::env::args().collect();
    let initial_game = args
        .iter()
        .position(|a| a == "--game")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u32>().ok());

    protongen_lib::run_with(initial_game);
    Ok(())
}
