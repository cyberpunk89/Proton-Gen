//! Tokenize an assembled launch command so the preview can colour and annotate
//! each part.
//!
//! The hard requirement is **byte-exactness**: concatenating every token's
//! `text` must reproduce the input character for character, including runs of
//! whitespace and quote characters. Anything less and a tokenized preview would
//! corrupt what the user copies. That's why this lives in Rust next to
//! `builder.rs` (which it must agree with) and is covered by a round-trip test
//! over every builder fixture.
//!
//! It cannot reuse `parser::tokenize`: that one strips quotes and throws
//! whitespace away.

use serde::Serialize;

use crate::parser;

/// What a token is, for the frontend's colouring and hover copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenKind {
    /// A run of whitespace between words. Carried so the tokens reassemble exactly.
    Space,
    /// `KEY=value` before the target.
    Env,
    /// `gamescope` / `gamemoderun` / `mangohud`.
    Wrapper,
    /// An argument belonging to `gamescope`, before its `--`.
    WrapperArg,
    /// The `--` that ends gamescope's arguments.
    Separator,
    /// `%command%` (Steam) or `umu-run` (umu).
    Target,
    /// The game executable — umu only, the first token after `umu-run`.
    Exe,
    /// An argument passed to the game, after the target.
    GameArg,
    /// Something protongen didn't emit and can't classify.
    Unknown,
}

/// One piece of the command line. `text` is verbatim source; `key` is the
/// catalog lookup key (env var name / wrapper name) when there is one — the
/// frontend resolves help/details/url from the already-loaded catalog.
#[derive(Clone, Debug, Serialize)]
pub struct Token {
    pub text: String,
    pub kind: TokenKind,
    pub key: Option<String>,
}

/// A word (verbatim, quotes included) or a run of whitespace.
enum Piece {
    Space(String),
    Word(String),
}

/// Split on whitespace outside quotes, **keeping** both the whitespace and the
/// quote characters. Same quoting rules as `parser::tokenize`, opposite policy
/// on what to discard: nothing.
fn split_preserving(input: &str) -> Vec<Piece> {
    let mut pieces = Vec::new();
    let mut cur = String::new();
    let mut in_space = true;
    let mut quote: Option<char> = None;

    for ch in input.chars() {
        let is_break = quote.is_none() && ch.is_whitespace();
        if is_break != in_space {
            if !cur.is_empty() {
                pieces.push(if in_space {
                    Piece::Space(std::mem::take(&mut cur))
                } else {
                    Piece::Word(std::mem::take(&mut cur))
                });
            }
            in_space = is_break;
        }
        if !is_break {
            match quote {
                Some(q) if ch == q => quote = None,
                Some(_) => {}
                None if ch == '"' || ch == '\'' => quote = Some(ch),
                None => {}
            }
        }
        cur.push(ch);
    }
    if !cur.is_empty() {
        pieces.push(if in_space { Piece::Space(cur) } else { Piece::Word(cur) });
    }
    pieces
}

/// The word with its quote characters removed, for matching only.
fn unquote(word: &str) -> String {
    let mut out = String::with_capacity(word.len());
    let mut quote: Option<char> = None;
    for ch in word.chars() {
        match quote {
            Some(q) if ch == q => quote = None,
            Some(_) => out.push(ch),
            None if ch == '"' || ch == '\'' => quote = Some(ch),
            None => out.push(ch),
        }
    }
    out
}

/// `KEY=value` where KEY looks like an environment variable name.
fn env_key(word: &str) -> Option<&str> {
    let (k, _) = word.split_once('=')?;
    let mut chars = k.chars();
    let first = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    Some(k)
}

/// Tokenize an assembled launch command.
///
/// Classification mirrors `builder::env_and_wrappers` and `build_umu_command`:
/// env assignments and wrappers precede the target, game arguments follow it.
pub fn explain(command: &str) -> Vec<Token> {
    let pieces = split_preserving(command);

    // umu commands are recognised the same way `parser::parse` recognises them,
    // by basename — the program can be named by path.
    let is_umu = pieces
        .iter()
        .any(|p| matches!(p, Piece::Word(w) if parser::basename(&unquote(w)) == "umu-run"));

    let mut out = Vec::with_capacity(pieces.len());
    let mut past_target = false;
    let mut in_gamescope_args = false;
    let mut post_count = 0usize;

    for piece in &pieces {
        let word = match piece {
            Piece::Space(s) => {
                out.push(Token { text: s.clone(), kind: TokenKind::Space, key: None });
                continue;
            }
            Piece::Word(w) => w,
        };
        let bare = unquote(word);
        // Program tokens are matched by basename; `key` stays the *catalog* key
        // so the frontend can still resolve it, even when the command names the
        // binary by path.
        let prog = parser::basename(&bare).to_string();

        let (kind, key) = if past_target {
            post_count += 1;
            // umu names the executable explicitly; Steam hides it behind %command%.
            if is_umu && post_count == 1 {
                (TokenKind::Exe, None)
            } else {
                (TokenKind::GameArg, None)
            }
        } else if if is_umu { prog == "umu-run" } else { bare == "%command%" } {
            past_target = true;
            in_gamescope_args = false;
            (TokenKind::Target, None)
        } else if bare == "--" {
            in_gamescope_args = false;
            (TokenKind::Separator, None)
        } else if in_gamescope_args {
            (TokenKind::WrapperArg, None)
        } else if prog == "gamescope" {
            in_gamescope_args = true;
            (TokenKind::Wrapper, Some(prog.clone()))
        } else if prog == "gamemoderun" || prog == "mangohud" {
            (TokenKind::Wrapper, Some(prog.clone()))
        } else if let Some(k) = env_key(&bare) {
            (TokenKind::Env, Some(k.to_string()))
        } else {
            (TokenKind::Unknown, None)
        };

        out.push(Token { text: word.clone(), kind, key });
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::{self, Wrapper};
    use crate::params::{self, Catalog, Options};

    fn env(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    fn kinds(cmd: &str) -> Vec<TokenKind> {
        explain(cmd)
            .into_iter()
            .filter(|t| t.kind != TokenKind::Space)
            .map(|t| t.kind)
            .collect()
    }

    fn texts(cmd: &str) -> Vec<String> {
        explain(cmd)
            .into_iter()
            .filter(|t| t.kind != TokenKind::Space)
            .map(|t| t.text)
            .collect()
    }

    /// Every fixture from `builder::tests`, regenerated from the builder itself
    /// so the corpus follows any change to the output format, plus the
    /// catalog-driven command from `params::tests::to_spec_orders_like_before`.
    fn fixtures() -> Vec<String> {
        let mut v = vec![
            builder::build_command(&[], &[], ""),
            builder::build_command(
                &env(&[("PROTON_ENABLE_WAYLAND", "1"), ("DXVK_ASYNC", "1")]),
                &[],
                "",
            ),
            builder::build_command(&[], &[Wrapper::Mangohud, Wrapper::Gamemoderun], ""),
            builder::build_command(
                &[],
                &[Wrapper::Mangohud, Wrapper::Gamescope("-W 2560 -H 1440 -f".into())],
                "",
            ),
            builder::build_command(&[], &[Wrapper::Gamescope(String::new())], ""),
            builder::build_command(
                &env(&[("PROTON_USE_NTSYNC", "1")]),
                &[
                    Wrapper::Gamemoderun,
                    Wrapper::Mangohud,
                    Wrapper::Gamescope("-f".into()),
                ],
                "--skip-launcher",
            ),
            builder::build_umu_command(
                &env(&[("PROTON_USE_NTSYNC", "1")]),
                &[],
                "/usr/share/steam/compatibilitytools.d/proton-cachyos-slr",
                "",
                None,
                "/games/Game/game.exe",
                "",
            ),
            builder::build_umu_command(
                &[],
                &[Wrapper::Mangohud, Wrapper::Gamemoderun],
                "/opt/proton",
                "umu-42",
                Some("/home/u/prefix"),
                "/games/My Game/game.exe",
                "--windowed",
            ),
            builder::build_umu_command(
                &[],
                &[Wrapper::Gamescope("-f".into()), Wrapper::Mangohud],
                "/opt/proton",
                "",
                None,
                "g.exe",
                "",
            ),
        ];

        // The params.rs pipeline output, built through the real catalog.
        let cat = Catalog::bundled();
        let mut opts = Options::from_catalog(&cat);
        for (i, w) in cat.wrappers.iter().enumerate() {
            if w.key == "mangohud" || w.key == "gamemoderun" {
                opts.wrappers[i].enabled = true;
            }
        }
        for (i, e) in cat.envs.iter().enumerate() {
            if e.key == "PROTON_ENABLE_WAYLAND" {
                opts.envs[i].enabled = true;
                opts.envs[i].value = "1".to_string();
            }
        }
        let (e, w) = params::to_spec(&cat, &opts);
        v.push(builder::build_command(&e, &w, ""));
        v
    }

    /// The load-bearing guarantee: tokens reassemble into the exact input.
    #[test]
    fn tokens_roundtrip_every_builder_fixture() {
        for cmd in fixtures() {
            let joined: String = explain(&cmd).iter().map(|t| t.text.as_str()).collect();
            assert_eq!(joined, cmd, "round-trip lost bytes for: {cmd}");
        }
    }

    #[test]
    fn tokens_roundtrip_odd_whitespace_and_quotes() {
        for cmd in [
            "",
            "   ",
            "  A=1   %command%  ",
            "\tA=1\n%command%",
            "WINEPREFIX='/home/u/my prefix' umu-run \"/games/My Game/g.exe\" --a  --b",
            "%command%",
        ] {
            let joined: String = explain(cmd).iter().map(|t| t.text.as_str()).collect();
            assert_eq!(joined, cmd, "round-trip lost bytes for: {cmd:?}");
        }
    }

    #[test]
    fn classifies_a_full_steam_command() {
        let cmd =
            "PROTON_ENABLE_WAYLAND=1 gamescope -W 2560 -f -- gamemoderun mangohud %command% --skip-launcher";
        use TokenKind::*;
        assert_eq!(
            kinds(cmd),
            vec![
                Env, Wrapper, WrapperArg, WrapperArg, WrapperArg, Separator, Wrapper, Wrapper,
                Target, GameArg,
            ]
        );
        let env_tok = explain(cmd).into_iter().find(|t| t.kind == Env).unwrap();
        assert_eq!(env_tok.key.as_deref(), Some("PROTON_ENABLE_WAYLAND"));
        let wrap_keys: Vec<String> = explain(cmd)
            .into_iter()
            .filter(|t| t.kind == Wrapper)
            .filter_map(|t| t.key)
            .collect();
        assert_eq!(wrap_keys, ["gamescope", "gamemoderun", "mangohud"]);
    }

    #[test]
    fn umu_exe_stays_one_quoted_token() {
        let cmd = "GAMEID=umu-0 PROTONPATH=/opt/proton umu-run \"/games/My Game/game.exe\" --windowed";
        use TokenKind::*;
        assert_eq!(kinds(cmd), vec![Env, Env, Target, Exe, GameArg]);
        // Quotes are preserved inside the token, and the space doesn't split it.
        assert_eq!(texts(cmd)[3], "\"/games/My Game/game.exe\"");
    }

    #[test]
    fn steam_post_target_tokens_are_all_game_args() {
        use TokenKind::*;
        assert_eq!(kinds("%command% -novid -high"), vec![Target, GameArg, GameArg]);
    }

    #[test]
    fn foreign_wrapper_is_unknown() {
        use TokenKind::*;
        assert_eq!(kinds("prime-run mangohud %command%"), vec![Unknown, Wrapper, Target]);
    }

    #[test]
    fn a_wrapper_named_by_path_is_still_a_wrapper() {
        use TokenKind::*;
        let cmd = "/usr/bin/mangohud %command%";
        assert_eq!(kinds(cmd), vec![Wrapper, Target]);
        // The reassembly guarantee holds: only the classification changed.
        assert_eq!(explain(cmd).iter().map(|t| t.text.as_str()).collect::<String>(), cmd);
        // `key` stays the catalog key, or the frontend could not resolve it.
        assert_eq!(explain(cmd)[0].key.as_deref(), Some("mangohud"));
    }

    #[test]
    fn umu_run_named_by_path_still_marks_the_target() {
        use TokenKind::*;
        let cmd = "GAMEID=umu-0 /home/u/.local/bin/umu-run /games/g.exe --windowed";
        assert_eq!(kinds(cmd), vec![Env, Target, Exe, GameArg]);
        assert_eq!(explain(cmd).iter().map(|t| t.text.as_str()).collect::<String>(), cmd);
    }
}
