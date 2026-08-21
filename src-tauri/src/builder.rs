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
    /// `game-performance` — CachyOS's performance wrapper (power profile +
    /// sched-ext gaming scheduler for the game's lifetime). The modern
    /// replacement for `gamemoderun` on CachyOS.
    GamePerformance,
    /// `gamemoderun`
    Gamemoderun,
    /// `mangohud`
    Mangohud,
}

/// Program tokens for the wrapper binaries and `umu-run`.
///
/// [`Default`] is the bare names — what an ordinary install wants, and what
/// every builder test asserts, so a byte-identical expectation *is* the
/// assertion that the default is a no-op.
///
/// Overridable because the emitted command is pasted into Steam and run with
/// Steam's `$PATH`, which — launched from a `.desktop` file — frequently omits
/// `~/.local/bin`. That is the exact case where a bare `umu-run` resolves in the
/// user's terminal and not in the game, and it is why an override has to change
/// the emitted token and not merely the installed/missing badge: a
/// detection-only override would be exercised *only* when it badges the binary
/// green and then emits a command that fails.
///
/// Any non-empty override is emitted verbatim — not "absolute paths only", since
/// overriding `gamescope` with `gamescope-git` should emit `gamescope-git`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bins {
    pub gamescope: String,
    pub gamemoderun: String,
    pub mangohud: String,
    pub umu_run: String,
}

impl Default for Bins {
    fn default() -> Self {
        Self {
            gamescope: "gamescope".to_string(),
            gamemoderun: "gamemoderun".to_string(),
            mangohud: "mangohud".to_string(),
            umu_run: "umu-run".to_string(),
        }
    }
}

impl Bins {
    /// Apply overrides keyed by catalog `requires` name (plus `umu-run`).
    /// Blank values are ignored, so a half-typed Settings row is not an override.
    pub fn with_overrides(map: &std::collections::BTreeMap<String, String>) -> Self {
        let mut b = Self::default();
        let pick = |slot: &mut String, key: &str| {
            if let Some(v) = map.get(key) {
                let v = v.trim();
                if !v.is_empty() {
                    *slot = v.to_string();
                }
            }
        };
        pick(&mut b.gamescope, "gamescope");
        pick(&mut b.gamemoderun, "gamemoderun");
        pick(&mut b.mangohud, "mangohud");
        pick(&mut b.umu_run, "umu-run");
        b
    }

    /// (catalog name, program actually emitted), for `compute_requires_status`
    /// — so the installed/missing badge reflects the token the builder emits
    /// rather than the one it would have emitted by default.
    pub fn pairs(&self) -> [(&'static str, &str); 4] {
        [
            ("gamescope", &self.gamescope),
            ("gamemoderun", &self.gamemoderun),
            ("mangohud", &self.mangohud),
            ("umu-run", &self.umu_run),
        ]
    }
}

impl Wrapper {
    /// Lower rank = more outer (placed further left).
    fn rank(&self) -> u8 {
        match self {
            Wrapper::Gamescope(_) => 0,
            // game-performance and gamemoderun are alternatives; if both are on,
            // game-performance sits just outside gamemoderun. Both stay inside
            // gamescope and outside mangohud.
            Wrapper::GamePerformance => 1,
            Wrapper::Gamemoderun => 2,
            Wrapper::Mangohud => 3,
        }
    }
}

/// Emit the env-var assignments followed by the wrappers (sorted outer->inner).
/// Shared by the Steam (`%command%`) and umu (`umu-run`) builders so both apply
/// identical ordering and `--` handling.
fn env_and_wrappers(env: &[(String, String)], wrappers: &[Wrapper], bins: &Bins) -> Vec<String> {
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
                    parts.push(format!("{} --", bins.gamescope));
                } else {
                    parts.push(format!("{} {args} --", bins.gamescope));
                }
            }
            // A fixed CachyOS system binary in /usr/bin, so — unlike the others —
            // it has no Settings override slot; emit the bare name.
            Wrapper::GamePerformance => parts.push("game-performance".to_string()),
            Wrapper::Gamemoderun => parts.push(bins.gamemoderun.clone()),
            Wrapper::Mangohud => parts.push(bins.mangohud.clone()),
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
pub fn build_command(
    env: &[(String, String)],
    wrappers: &[Wrapper],
    game_args: &str,
    bins: &Bins,
) -> String {
    let mut parts = env_and_wrappers(env, wrappers, bins);

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
    bins: &Bins,
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

    let mut parts = env_and_wrappers(&lead, wrappers, bins);

    parts.push(bins.umu_run.clone());
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

    /// Every expectation below is byte-identical to before `Bins` existed,
    /// which is exactly the assertion that the default is a no-op.
    fn bins() -> Bins {
        Bins::default()
    }

    fn env(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn bare_command() {
        assert_eq!(build_command(&[], &[], "", &bins()), "%command%");
    }

    #[test]
    fn env_only() {
        let e = env(&[("PROTON_ENABLE_WAYLAND", "1"), ("DXVK_ASYNC", "1")]);
        assert_eq!(
            build_command(&e, &[], "", &bins()),
            "PROTON_ENABLE_WAYLAND=1 DXVK_ASYNC=1 %command%"
        );
    }

    #[test]
    fn wrappers_only_sorted() {
        // Toggled in "wrong" order; output must be gamemoderun before mangohud.
        let w = vec![Wrapper::Mangohud, Wrapper::Gamemoderun];
        assert_eq!(
            build_command(&[], &w, "", &bins()),
            "gamemoderun mangohud %command%"
        );
    }

    #[test]
    fn game_performance_sits_inside_gamescope_and_outside_mangohud() {
        // Toggled in "wrong" order; ranks must place it gamescope > game-performance
        // > mangohud, and it emits the bare CachyOS binary name.
        let w = vec![
            Wrapper::Mangohud,
            Wrapper::GamePerformance,
            Wrapper::Gamescope("-f".to_string()),
        ];
        assert_eq!(
            build_command(&[], &w, "", &bins()),
            "gamescope -f -- game-performance mangohud %command%"
        );
    }

    #[test]
    fn gamescope_wraps_with_separator() {
        let w = vec![
            Wrapper::Mangohud,
            Wrapper::Gamescope("-W 2560 -H 1440 -f".to_string()),
        ];
        assert_eq!(
            build_command(&[], &w, "", &bins()),
            "gamescope -W 2560 -H 1440 -f -- mangohud %command%"
        );
    }

    #[test]
    fn gamescope_no_args() {
        let w = vec![Wrapper::Gamescope(String::new())];
        assert_eq!(build_command(&[], &w, "", &bins()), "gamescope -- %command%");
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
            &bins(),
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
            &bins(),
        );
        assert_eq!(
            out,
            "WINEPREFIX=/home/u/prefix GAMEID=umu-42 PROTONPATH=/opt/proton gamemoderun mangohud umu-run \"/games/My Game/game.exe\" --windowed"
        );
    }

    #[test]
    fn umu_gamescope_wraps_outermost() {
        let w = vec![Wrapper::Gamescope("-f".to_string()), Wrapper::Mangohud];
        let out = build_umu_command(&[], &w, "/opt/proton", "", None, "g.exe", "", &bins());
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
        let out = build_command(&e, &w, "--skip-launcher", &bins());
        assert_eq!(
            out,
            "PROTON_USE_NTSYNC=1 gamescope -f -- gamemoderun mangohud %command% --skip-launcher"
        );
        // %command% appears exactly once.
        assert_eq!(out.matches("%command%").count(), 1);
    }

    #[test]
    fn the_default_bins_are_the_bare_names() {
        // Every expectation above relies on this, so state it once explicitly.
        let b = Bins::default();
        assert_eq!(
            b.pairs().map(|(_, p)| p.to_string()).to_vec(),
            vec!["gamescope", "gamemoderun", "mangohud", "umu-run"]
        );
    }

    #[test]
    fn an_override_replaces_the_wrapper_token() {
        // The point of the feature: Steam launched from a .desktop file has a
        // $PATH that often lacks ~/.local/bin, so a bare name resolves in the
        // user's terminal and not in the game.
        let bins = Bins::with_overrides(&std::collections::BTreeMap::from([
            ("mangohud".to_string(), "/usr/local/bin/mangohud".to_string()),
            ("gamescope".to_string(), "gamescope-git".to_string()),
        ]));
        let w = vec![Wrapper::Mangohud, Wrapper::Gamescope("-f".into())];
        assert_eq!(
            build_command(&[], &w, "", &bins),
            "gamescope-git -f -- /usr/local/bin/mangohud %command%"
        );
    }

    #[test]
    fn an_override_replaces_umu_run_too() {
        let bins = Bins::with_overrides(&std::collections::BTreeMap::from([(
            "umu-run".to_string(),
            "/home/u/.local/bin/umu-run".to_string(),
        )]));
        let out = build_umu_command(&[], &[], "/opt/proton", "", None, "g.exe", "", &bins);
        assert_eq!(
            out,
            "GAMEID=umu-0 PROTONPATH=/opt/proton /home/u/.local/bin/umu-run g.exe"
        );
    }

    #[test]
    fn a_blank_override_is_not_an_override() {
        // A half-typed Settings row must not blank out the program name.
        let bins = Bins::with_overrides(&std::collections::BTreeMap::from([
            ("mangohud".to_string(), "   ".to_string()),
            ("gamescope".to_string(), String::new()),
        ]));
        assert_eq!(bins, Bins::default());
    }
}
