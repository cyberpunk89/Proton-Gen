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
fn tokenize(input: &str) -> Vec<String> {
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

/// Consume a wrapper token; returns true if it matched a known wrapper. For
/// `gamescope`, collects args from `iter` up to the `--` separator.
fn take_wrapper(tok: &str, iter: &mut std::iter::Peekable<std::slice::Iter<String>>, p: &mut Parsed) -> bool {
    match tok {
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

    let is_umu = tokens.iter().any(|t| t == "umu-run");
    let sep = if is_umu { "umu-run" } else { "%command%" };
    p.umu = is_umu;

    let split = tokens.iter().position(|t| t == sep);
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
                "GAMEID" => p.umu_gameid = Some(v.to_string()),
                "PROTONPATH" => { /* derived from runtime selection, ignore */ }
                "WINEPREFIX" => p.umu_wineprefix = Some(v.to_string()),
                _ => p.env.push((k.to_string(), v.to_string())),
            }
        }
        // bare unknown tokens before the target are ignored
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
}
