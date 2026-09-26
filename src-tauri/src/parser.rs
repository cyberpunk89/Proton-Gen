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
    pub game_performance: bool,
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
        if self.game_performance {
            v.push(Wrapper::GamePerformance);
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

/// One shell word: its value with quoting removed, the byte range of its
/// source text, and whether it holds an *unquoted* shell operator character.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Word {
    pub text: String,
    pub start: usize,
    pub end: usize,
    /// An unquoted `; & | < > ( )` or backtick. Before the target that means
    /// the shell does not see what it looks like — `A=x;y %command%` runs `A=x`
    /// as its own statement and never exports it to the game.
    pub operator: bool,
}

/// Split a command line into words the way a POSIX shell does: whitespace
/// separates words outside quotes; `'…'` is literal; inside `"…"` a backslash
/// escapes only `$`, `` ` ``, `"`, `\` and newline; outside quotes a backslash
/// makes the next character literal. An unclosed quote runs to the end.
pub fn words(input: &str) -> Vec<Word> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut start: Option<usize> = None;
    let mut operator = false;
    let mut quote: Option<char> = None;
    let mut chars = input.char_indices().peekable();

    while let Some((i, ch)) = chars.next() {
        match quote {
            Some('\'') => {
                if ch == '\'' {
                    quote = None;
                } else {
                    cur.push(ch);
                }
            }
            Some(_) => match ch {
                '"' => quote = None,
                '\\' => match chars.peek() {
                    Some(&(_, n)) if matches!(n, '$' | '`' | '"' | '\\' | '\n') => {
                        chars.next();
                        if n != '\n' {
                            cur.push(n);
                        }
                    }
                    _ => cur.push('\\'),
                },
                c => cur.push(c),
            },
            None if ch.is_whitespace() => {
                if let Some(s) = start.take() {
                    out.push(Word { text: std::mem::take(&mut cur), start: s, end: i, operator });
                    operator = false;
                }
            }
            None => {
                start.get_or_insert(i);
                match ch {
                    '"' | '\'' => quote = Some(ch),
                    '\\' => {
                        if let Some((_, n)) = chars.next() {
                            if n != '\n' {
                                cur.push(n);
                            }
                        }
                    }
                    c => {
                        operator |= matches!(c, ';' | '&' | '|' | '<' | '>' | '(' | ')' | '`');
                        cur.push(c);
                    }
                }
            }
        }
    }
    if let Some(s) = start {
        out.push(Word { text: cur, start: s, end: input.len(), operator });
    }
    out
}

/// The words' values alone, quoting removed.
pub fn tokenize(input: &str) -> Vec<String> {
    words(input).into_iter().map(|w| w.text).collect()
}

/// The verbatim source from the start of `first` to the end of `last`.
fn raw_span<'a>(input: &'a str, first: &Word, last: &Word) -> &'a str {
    &input[first.start..last.end]
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
/// `gamescope`, collects args from `iter` up to the `--` separator, kept as
/// verbatim source so their quoting survives a round trip.
fn take_wrapper(
    input: &str,
    tok: &Word,
    iter: &mut std::iter::Peekable<std::slice::Iter<Word>>,
    p: &mut Parsed,
) -> bool {
    match basename(&tok.text) {
        "game-performance" => {
            p.game_performance = true;
            true
        }
        "gamemoderun" => {
            p.gamemoderun = true;
            true
        }
        "mangohud" => {
            p.mangohud = true;
            true
        }
        "gamescope" => {
            let mut args: Vec<&Word> = Vec::new();
            while let Some(next) = iter.peek() {
                if next.text == "--" {
                    iter.next();
                    break;
                }
                args.push(iter.next().unwrap());
            }
            p.gamescope = Some(match (args.first(), args.last()) {
                (Some(first), Some(last)) => raw_span(input, first, last).to_string(),
                _ => String::new(),
            });
            true
        }
        _ => false,
    }
}

/// Parse a Steam launch-options string or a standalone `umu-run` command.
pub fn parse(input: &str) -> Parsed {
    let tokens = words(input);
    let mut p = Parsed::default();

    // `umu-run` can be an absolute path; `%command%` is Steam's literal
    // placeholder and never is. Deriving the split index from the same lookup
    // that decides the mode keeps the two from disagreeing.
    let umu_at = tokens.iter().position(|t| basename(&t.text) == "umu-run");
    let is_umu = umu_at.is_some();
    p.umu = is_umu;

    let split = match umu_at {
        Some(i) => Some(i),
        None => tokens.iter().position(|t| t.text == "%command%"),
    };
    let (pre, post): (&[Word], &[Word]) = match split {
        Some(i) => (&tokens[..i], &tokens[i + 1..]),
        None => (&tokens[..], &[]),
    };

    let mut it = pre.iter().peekable();
    while let Some(word) = it.next() {
        if take_wrapper(input, word, &mut it, &mut p) {
            continue;
        }
        let tok = &word.text;
        // Recorded as unmodeled *as well as* parsed below: the pair keeps an
        // import lossless, while the flag stops an old unquoted `A=x;y` from
        // comparing equal to the quoted `A="x;y"` we emit — the shell never
        // exported that value, so the user has to re-paste.
        if word.operator {
            p.unknown.push(raw_span(input, word, word).to_string());
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
        if !word.operator {
            p.unknown.push(tok.clone());
        }
    }

    // Game args are the user's own shell syntax, so they are taken verbatim
    // from the source — re-joining unquoted tokens turned `-x "a b"` into
    // `-x a b` on import.
    let rest = |args: &[Word]| match (args.first(), args.last()) {
        (Some(first), Some(last)) => raw_span(input, first, last).to_string(),
        _ => String::new(),
    };
    if is_umu {
        if let Some((first, args)) = post.split_first() {
            p.umu_exe = first.text.clone();
            p.game_args = rest(args);
        }
    } else {
        p.game_args = rest(post);
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
        let rebuilt = builder::build_command(&p.env, &p.wrappers(), &p.game_args, &builder::Bins::default());
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
            &builder::Bins::default(),
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

    #[test]
    fn tokenizer_follows_posix_quoting() {
        assert_eq!(tokenize(r#"A="x\"y" B='it\s' C=a\ b D="\q""#), vec![
            "A=x\"y", "B=it\\s", "C=a b", "D=\\q"
        ]);
        assert_eq!(tokenize(r#"E="\$HOME" F="a\\b""#), vec!["E=$HOME", "F=a\\b"]);
        // An unclosed quote swallows the rest rather than inventing a split.
        assert_eq!(tokenize("A=\"x y"), vec!["A=x y"]);
    }

    #[test]
    fn built_values_round_trip_through_the_parser() {
        let tricky = [
            ("DXVK_CONFIG", "dxgi.maxFrameLatency=1;dxvk.hud=fps"),
            ("WINEDLLOVERRIDES", "mscoree=d;mshtml=d"),
            ("SPACED", "a b"),
            ("QUOTED", "say \"hi\""),
            ("BACKTICK", "a`b"),
            ("BACKSLASH", "C:\\games"),
            ("CACHE", "$HOME/.cache"),
            ("TILDE", "~/my games"),
            ("EMPTY", ""),
        ];
        let env: Vec<(String, String)> =
            tricky.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();

        let steam = builder::build_command(&env, &[], "", &builder::Bins::default());
        let p = parse(&steam);
        assert!(p.unknown.is_empty(), "nothing unmodeled in {steam}: {:?}", p.unknown);
        // `$HOME` and `~` are left for the shell to expand, so compare the rest.
        for ((k, v), (pk, pv)) in env.iter().zip(&p.env) {
            assert_eq!(k, pk);
            if !v.starts_with('$') && !v.starts_with('~') {
                assert_eq!(v, pv, "{k} in {steam}");
            }
        }

        let umu = builder::build_umu_command(
            &env,
            &[],
            "/home/u/Proton 9.0 (Beta)",
            "",
            Some("/home/u/my prefix"),
            "/games/My Game/g.exe",
            "",
            &builder::Bins::default(),
        );
        let p = parse(&umu);
        assert_eq!(p.umu_wineprefix.as_deref(), Some("/home/u/my prefix"));
        assert_eq!(p.umu_exe, "/games/My Game/g.exe");
        assert_eq!(p.env.len(), env.len());
    }

    #[test]
    fn game_args_and_gamescope_args_are_kept_verbatim() {
        let s = r#"gamescope -W 2560 --prefer-output "DP-1" -- %command% -x "a b" --path='C:\x'"#;
        let p = parse(s);
        assert_eq!(p.gamescope.as_deref(), Some(r#"-W 2560 --prefer-output "DP-1""#));
        assert_eq!(p.game_args, r#"-x "a b" --path='C:\x'"#);
        let rebuilt =
            builder::build_command(&p.env, &p.wrappers(), &p.game_args, &builder::Bins::default());
        assert_eq!(rebuilt, s);

        let u = parse(r#"umu-run "/g/My Game.exe" -x "a b""#);
        assert_eq!(u.umu_exe, "/g/My Game.exe");
        assert_eq!(u.game_args, r#"-x "a b""#);
    }

    #[test]
    fn an_unquoted_shell_operator_before_the_target_is_unmodeled() {
        // What older builds emitted. The shell ended the statement at `;`, so the
        // variable never reached the game — this must not read as in sync.
        let p = parse("DXVK_CONFIG=a;b %command%");
        assert_eq!(p.env, vec![("DXVK_CONFIG".to_string(), "a;b".to_string())]);
        assert_eq!(p.unknown, vec!["DXVK_CONFIG=a;b".to_string()]);

        // Quoted, it's just a value.
        assert!(parse("DXVK_CONFIG=\"a;b\" %command%").unknown.is_empty());
        // After the target it's the game's business.
        assert!(parse("%command% ; echo done").unknown.is_empty());
    }

    #[test]
    fn parses_game_performance_wrapper() {
        let p = parse("game-performance mangohud %command%");
        assert!(p.game_performance);
        assert!(p.mangohud);
        assert!(!p.gamemoderun);
        assert!(p.unknown.is_empty());
    }
}
