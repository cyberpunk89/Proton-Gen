//! Pure assembly of a Steam Launch Options string.
//!
//! Steam replaces `%command%` with the game's executable. Anything *before*
//! `%command%` is environment variables and wrapper programs; anything *after*
//! is passed as arguments to the game. Wrapper ordering matters: `gamescope`
//! launches everything else, so it must be outermost and use a `--` separator
//! before the inner command.
//!
//! Produced shape:
//! `ENV1=v ENV2=v  gamescope <args> --  gamemoderun mangohud  %command%  <game args>`

/// A wrapper program placed before `%command%`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Wrapper {
    /// `gamescope <args> --` — the outermost wrapper, owns the `--` separator.
    Gamescope(String),
    /// `gamemoderun`
    Gamemoderun,
    /// `mangohud`
    Mangohud,
}

impl Wrapper {
    /// Lower rank = more outer (placed further left).
    fn rank(&self) -> u8 {
        match self {
            Wrapper::Gamescope(_) => 0,
            Wrapper::Gamemoderun => 1,
            Wrapper::Mangohud => 2,
        }
    }
}

/// Emit the env-var assignments followed by the wrappers (sorted outer->inner).
/// Shared by the Steam (`%command%`) and umu (`umu-run`) builders so both apply
/// identical ordering and `--` handling.
fn env_and_wrappers(env: &[(String, String)], wrappers: &[Wrapper]) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();

    // Environment variables, in the order given.
    for (k, v) in env {
        parts.push(format!("{k}={v}"));
    }

    // Wrappers, sorted outer -> inner so output is deterministic regardless of
    // the order the user toggled them in.
    let mut ws: Vec<&Wrapper> = wrappers.iter().collect();
    ws.sort_by_key(|w| w.rank());
    for w in ws {
        match w {
            Wrapper::Gamescope(args) => {
                let args = args.trim();
                if args.is_empty() {
                    parts.push("gamescope --".to_string());
                } else {
                    parts.push(format!("gamescope {args} --"));
                }
            }
            Wrapper::Gamemoderun => parts.push("gamemoderun".to_string()),
            Wrapper::Mangohud => parts.push("mangohud".to_string()),
        }
    }
    parts
}

/// Quote a path for a shell command only if it contains whitespace.
fn shell_quote(s: &str) -> String {
    if s.contains(char::is_whitespace) {
        format!("\"{s}\"")
    } else {
        s.to_string()
    }
}

/// Build the Steam Launch Options string from environment variables, wrappers
/// and trailing game arguments. `%command%` always appears exactly once.
pub fn build_command(env: &[(String, String)], wrappers: &[Wrapper], game_args: &str) -> String {
    let mut parts = env_and_wrappers(env, wrappers);

    // The mandatory placeholder.
    parts.push("%command%".to_string());

    // Trailing game arguments.
    let game_args = game_args.trim();
    if !game_args.is_empty() {
        parts.push(game_args.to_string());
    }

    parts.join(" ")
}

/// Build a standalone `umu-run` command for running a game outside Steam.
///
/// Shape: `[WINEPREFIX=…] GAMEID=… PROTONPATH=… ENV=v …  <wrappers>  umu-run "<exe>"  <args>`
/// `protonpath` is the selected runtime's install directory; `gameid` defaults
/// to `umu-0` when empty.
pub fn build_umu_command(
    env: &[(String, String)],
    wrappers: &[Wrapper],
    protonpath: &str,
    gameid: &str,
    wineprefix: Option<&str>,
    exe: &str,
    game_args: &str,
) -> String {
    // umu-specific assignments come first, then the user's env vars + wrappers.
    let mut lead: Vec<(String, String)> = Vec::new();
    if let Some(wp) = wineprefix.map(str::trim).filter(|s| !s.is_empty()) {
        lead.push(("WINEPREFIX".to_string(), wp.to_string()));
    }
    let gameid = {
        let g = gameid.trim();
        if g.is_empty() { "umu-0" } else { g }
    };
    lead.push(("GAMEID".to_string(), gameid.to_string()));
    lead.push(("PROTONPATH".to_string(), protonpath.trim().to_string()));
    lead.extend(env.iter().cloned());

    let mut parts = env_and_wrappers(&lead, wrappers);

    parts.push("umu-run".to_string());
    let exe = exe.trim();
    parts.push(shell_quote(if exe.is_empty() { "<game.exe>" } else { exe }));

    let game_args = game_args.trim();
    if !game_args.is_empty() {
        parts.push(game_args.to_string());
    }

    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn bare_command() {
        assert_eq!(build_command(&[], &[], ""), "%command%");
    }

    #[test]
    fn env_only() {
        let e = env(&[("PROTON_ENABLE_WAYLAND", "1"), ("DXVK_ASYNC", "1")]);
        assert_eq!(
            build_command(&e, &[], ""),
            "PROTON_ENABLE_WAYLAND=1 DXVK_ASYNC=1 %command%"
        );
    }

    #[test]
    fn wrappers_only_sorted() {
        // Toggled in "wrong" order; output must be gamemoderun before mangohud.
        let w = vec![Wrapper::Mangohud, Wrapper::Gamemoderun];
        assert_eq!(
            build_command(&[], &w, ""),
            "gamemoderun mangohud %command%"
        );
    }

    #[test]
    fn gamescope_wraps_with_separator() {
        let w = vec![
            Wrapper::Mangohud,
            Wrapper::Gamescope("-W 2560 -H 1440 -f".to_string()),
        ];
        assert_eq!(
            build_command(&[], &w, ""),
            "gamescope -W 2560 -H 1440 -f -- mangohud %command%"
        );
    }

    #[test]
    fn gamescope_no_args() {
        let w = vec![Wrapper::Gamescope(String::new())];
        assert_eq!(build_command(&[], &w, ""), "gamescope -- %command%");
    }

    #[test]
    fn umu_basic_prefix_order() {
        let e = env(&[("PROTON_USE_NTSYNC", "1")]);
        let out = build_umu_command(
            &e,
            &[],
            "/usr/share/steam/compatibilitytools.d/proton-cachyos-slr",
            "",
            None,
            "/games/Game/game.exe",
            "",
        );
        assert_eq!(
            out,
            "GAMEID=umu-0 PROTONPATH=/usr/share/steam/compatibilitytools.d/proton-cachyos-slr PROTON_USE_NTSYNC=1 umu-run /games/Game/game.exe"
        );
    }

    #[test]
    fn umu_wraps_umu_run_and_quotes_exe() {
        let w = vec![Wrapper::Mangohud, Wrapper::Gamemoderun];
        let out = build_umu_command(
            &[],
            &w,
            "/opt/proton",
            "umu-42",
            Some("/home/u/prefix"),
            "/games/My Game/game.exe",
            "--windowed",
        );
        assert_eq!(
            out,
            "WINEPREFIX=/home/u/prefix GAMEID=umu-42 PROTONPATH=/opt/proton gamemoderun mangohud umu-run \"/games/My Game/game.exe\" --windowed"
        );
    }

    #[test]
    fn umu_gamescope_wraps_outermost() {
        let w = vec![Wrapper::Gamescope("-f".to_string()), Wrapper::Mangohud];
        let out = build_umu_command(&[], &w, "/opt/proton", "", None, "g.exe", "");
        assert_eq!(
            out,
            "GAMEID=umu-0 PROTONPATH=/opt/proton gamescope -f -- mangohud umu-run g.exe"
        );
    }

    #[test]
    fn all_combined() {
        let e = env(&[("PROTON_USE_NTSYNC", "1")]);
        let w = vec![
            Wrapper::Gamemoderun,
            Wrapper::Mangohud,
            Wrapper::Gamescope("-f".to_string()),
        ];
        let out = build_command(&e, &w, "--skip-launcher");
        assert_eq!(
            out,
            "PROTON_USE_NTSYNC=1 gamescope -f -- gamemoderun mangohud %command% --skip-launcher"
        );
        // %command% appears exactly once.
        assert_eq!(out.matches("%command%").count(), 1);
    }
}
