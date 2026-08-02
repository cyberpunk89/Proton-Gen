//! Parse an existing launch string back into structured options (the inverse of
//! `builder.rs`), so a user can paste a command and have the UI populate.

use crate::builder::Wrapper;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // used in tests; gui reads `Parsed.umu` directly
pub enum ParsedMode {
    Steam,
    Umu,
}

#[derive(Clone, Debug, Default)]
pub struct Parsed {
    pub umu: bool,
    pub env: Vec<(String, String)>,
    /// Pre-target tokens protongen can't model (foreign wrappers like `prime-run`,
    /// bare flags, or `PROTONPATH=` in Steam mode). Kept rather than dropped so
    /// callers can tell "protongen built this" from "something else did".
    /// Consumed by `diff::compare`: a non-empty `unknown` on the Steam side is
    /// on its own enough to call a command drifted.
    pub unknown: Vec<String>,
    pub gamescope: Option<String>,
    pub gamemoderun: bool,
    pub mangohud: bool,
    pub game_args: String,
    pub umu_exe: String,
    pub umu_wineprefix: Option<String>,
    pub umu_gameid: Option<String>,
}

impl Parsed {
    #[allow(dead_code)] // used in tests
    pub fn mode(&self) -> ParsedMode {
        if self.umu { ParsedMode::Umu } else { ParsedMode::Steam }
    }

    /// Wrappers in the canonical set (builder sorts them anyway).
    pub fn wrappers(&self) -> Vec<Wrapper> {
        let mut v = Vec::new();
        if let Some(args) = &self.gamescope {
            v.push(Wrapper::Gamescope(args.clone()));
        }
        if self.gamemoderun {
            v.push(Wrapper::Gamemoderun);
        }
        if self.mangohud {
            v.push(Wrapper::Mangohud);
        }
        v
    }
}

/// Split a command line into tokens, honoring single/double quotes (quotes are
/// stripped from the resulting tokens).
pub fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut has = false;
    for ch in input.chars() {
        match quote {
            Some(q) => {
                if ch == q {
                    quote = None;
                } else {
                    cur.push(ch);
                }
            }
            None => match ch {
                '"' | '\'' => {
                    quote = Some(ch);
                    has = true;
                }
                c if c.is_whitespace() => {
                    if has {
                        tokens.push(std::mem::take(&mut cur));
                        has = false;
                    }
                }
                c => {
                    cur.push(c);
                    has = true;
                }
            },
        }
    }
    if has {
        tokens.push(cur);
    }
    tokens
}

/// The last path component of a token, or the token itself if it has no `/`.
///
/// Wrapper programs are recognised by basename, not by exact string: a command
/// can legitimately name one by path — a user pasting `/usr/bin/gamescope -f --
/// %command%`, or protongen itself emitting a configured binary override. Before
/// this, such a token fell through to `unknown`, which forced a permanent
/// `Drifted` verdict and painted it `Unknown` in the preview.
pub(crate) fn basename(tok: &str) -> &str {
    tok.rsplit('/').next().unwrap_or(tok)
}

/// Consume a wrapper token; returns true if it matched a known wrapper. For
/// `gamescope`, collects args from `iter` up to the `--` separator.
fn take_wrapper(tok: &str, iter: &mut std::iter::Peekable<std::slice::Iter<String>>, p: &mut Parsed) -> bool {
    match basename(tok) {
        "gamemoderun" => {
            p.gamemoderun = true;
            true
        }
        "mangohud" => {
            p.mangohud = true;
            true
        }
        "gamescope" => {
            let mut args = Vec::new();
            while let Some(next) = iter.peek() {
                if next.as_str() == "--" {
                    iter.next();
                    break;
                }
                args.push(iter.next().unwrap().clone());
            }
            p.gamescope = Some(args.join(" "));
            true
        }
        _ => false,
    }
}

/// Parse a Steam launch-options string or a standalone `umu-run` command.
pub fn parse(input: &str) -> Parsed {
    let tokens = tokenize(input);
    let mut p = Parsed::default();

    // `umu-run` can be an absolute path; `%command%` is Steam's literal
    // placeholder and never is. Deriving the split index from the same lookup
    // that decides the mode keeps the two from disagreeing.
    let umu_at = tokens.iter().position(|t| basename(t) == "umu-run");
    let is_umu = umu_at.is_some();
    p.umu = is_umu;

    let split = match umu_at {
        Some(i) => Some(i),
        None => tokens.iter().position(|t| t == "%command%"),
    };
    let (pre, post): (&[String], &[String]) = match split {
        Some(i) => (&tokens[..i], &tokens[i + 1..]),
        None => (&tokens[..], &[]),
    };

    let mut it = pre.iter().peekable();
    while let Some(tok) = it.next() {
        if take_wrapper(tok, &mut it, &mut p) {
            continue;
        }
        if let Some((k, v)) = tok.split_once('=') {
            match k {
                // The umu-specific assignments only have dedicated fields in umu
                // mode; under Steam they're ordinary env vars (and PROTONPATH is
                // inert, but the user should still see that it's there).
                "GAMEID" if is_umu => p.umu_gameid = Some(v.to_string()),
                "WINEPREFIX" if is_umu => p.umu_wineprefix = Some(v.to_string()),
                "PROTONPATH" if is_umu => { /* derived from runtime selection */ }
                "PROTONPATH" => p.unknown.push(tok.clone()),
                _ => p.env.push((k.to_string(), v.to_string())),
            }
            continue;
        }
        // A bare token that isn't a wrapper we model: a foreign wrapper
        // (`prime-run`, `strangle`, …) or a stray flag. Never drop it.
        p.unknown.push(tok.clone());
    }

    if is_umu {
        if let Some((first, rest)) = post.split_first() {
            p.umu_exe = first.clone();
            p.game_args = rest.join(" ");
        }
    } else {
        p.game_args = post.join(" ");
    }

    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder;

    #[test]
    fn roundtrip_steam() {
        let s = "PROTON_ENABLE_WAYLAND=1 DXVK_ASYNC=1 gamescope -W 2560 -H 1440 -f -- gamemoderun mangohud %command% --skip-launcher";
        let p = parse(s);
        assert_eq!(p.mode(), ParsedMode::Steam);
        let rebuilt = builder::build_command(&p.env, &p.wrappers(), &p.game_args);
        assert_eq!(rebuilt, s);
    }

    #[test]
    fn roundtrip_umu() {
        let s = "GAMEID=umu-0 PROTONPATH=/opt/proton PROTON_USE_NTSYNC=1 gamemoderun mangohud umu-run /games/game.exe --windowed";
        let p = parse(s);
        assert_eq!(p.mode(), ParsedMode::Umu);
        assert_eq!(p.umu_exe, "/games/game.exe");
        assert_eq!(p.umu_gameid.as_deref(), Some("umu-0"));
        let rebuilt = builder::build_umu_command(
            &p.env,
            &p.wrappers(),
            "/opt/proton",
            p.umu_gameid.as_deref().unwrap_or(""),
            p.umu_wineprefix.as_deref(),
            &p.umu_exe,
            &p.game_args,
        );
        assert_eq!(rebuilt, s);
    }

    #[test]
    fn quoted_value_and_wineprefix() {
        let p = parse("WINEPREFIX='/home/u/my prefix' WINEDLLOVERRIDES=\"winmm=n,b\" umu-run game.exe");
        assert_eq!(p.umu_wineprefix.as_deref(), Some("/home/u/my prefix"));
        assert_eq!(p.env, vec![("WINEDLLOVERRIDES".to_string(), "winmm=n,b".to_string())]);
        assert_eq!(p.umu_exe, "game.exe");
    }

    #[test]
    fn gamescope_no_args() {
        let p = parse("gamescope -- %command%");
        assert_eq!(p.gamescope.as_deref(), Some(""));
    }

    #[test]
    fn foreign_wrapper_is_kept_as_unknown() {
        let p = parse("DXVK_ASYNC=1 prime-run mangohud %command%");
        assert!(p.mangohud);
        assert_eq!(p.env, vec![("DXVK_ASYNC".to_string(), "1".to_string())]);
        assert_eq!(p.unknown, vec!["prime-run".to_string()]);
    }

    #[test]
    fn no_command_placeholder_still_reports_tokens() {
        // Everything lands in `pre`; previously this parsed to an empty Config.
        let p = parse("-novid strangle 60");
        assert_eq!(p.mode(), ParsedMode::Steam);
        assert_eq!(p.game_args, "");
        assert_eq!(
            p.unknown,
            vec!["-novid".to_string(), "strangle".to_string(), "60".to_string()]
        );
    }

    #[test]
    fn steam_mode_keeps_umu_style_assignments() {
        let p = parse("GAMEID=umu-0 PROTONPATH=/opt/proton WINEPREFIX=/home/u/pfx %command%");
        assert!(!p.umu);
        // GAMEID/WINEPREFIX are plain env vars without umu-run.
        assert_eq!(
            p.env,
            vec![
                ("GAMEID".to_string(), "umu-0".to_string()),
                ("WINEPREFIX".to_string(), "/home/u/pfx".to_string()),
            ]
        );
        assert_eq!(p.umu_gameid, None);
        assert_eq!(p.umu_wineprefix, None);
        // PROTONPATH does nothing under Steam, but it isn't silently erased.
        assert_eq!(p.unknown, vec!["PROTONPATH=/opt/proton".to_string()]);
    }

    #[test]
    fn an_absolute_wrapper_path_is_still_a_wrapper() {
        // Wrappers used to be matched by exact string, so a path fell through to
        // `unknown` — which forced a permanent Drifted verdict against a command
        // that is functionally identical to the one we build.
        let p = parse("/usr/bin/mangohud /usr/bin/gamescope -f -- %command%");
        assert!(p.mangohud);
        assert_eq!(p.gamescope.as_deref(), Some("-f"));
        assert!(p.unknown.is_empty(), "got: {:?}", p.unknown);
    }

    #[test]
    fn an_absolute_umu_run_still_splits_the_command() {
        // Worse than the wrapper case: `is_umu` came out false, so the whole
        // command parsed as a Steam command and the separator was never found.
        let p = parse("GAMEID=umu-0 /home/u/.local/bin/umu-run /games/game.exe --windowed");
        assert!(p.umu);
        assert_eq!(p.umu_gameid.as_deref(), Some("umu-0"));
        assert_eq!(p.umu_exe, "/games/game.exe");
        assert_eq!(p.game_args, "--windowed");
    }

    #[test]
    fn a_relative_binary_variant_is_matched_by_name() {
        // An override needn't be absolute: `gamescope-git` is a different
        // program and must NOT match, while a bare rename in a custom dir must.
        assert!(parse("/opt/tools/gamemoderun %command%").gamemoderun);
        let p = parse("gamescope-git -f -- %command%");
        assert!(p.gamescope.is_none());
        // Unrecognised, so its args are not consumed as wrapper args either —
        // they stay visible in `unknown` rather than being swallowed.
        assert_eq!(p.unknown, vec!["gamescope-git", "-f", "--"]);
    }
}
