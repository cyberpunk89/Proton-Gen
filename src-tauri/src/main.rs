// Prevents an extra console window on Windows in release; harmless elsewhere.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::Result;

fn main() -> Result<()> {
    // Non-interactive dump mode: `protongen --list` prints discovered runtimes,
    // games and catalog size, then exits. Useful for verification and scripting.
    if std::env::args().any(|a| a == "--list" || a == "--scan") {
        return protongen_lib::dump();
    }

    protongen_lib::run();
    Ok(())
}
