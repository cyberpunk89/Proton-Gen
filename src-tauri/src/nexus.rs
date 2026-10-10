//! Nexus, the user's own game launcher (`~/Documents/Projects/Fitgirl`):
//! read-only discovery of its library, `~/.local/share/nexus/games.json`.
//!
//! Nexus installs repacks and then mirrors each one into Steam (a non-Steam
//! shortcut pointing straight at the exe) and Heroic (a sideload). Without
//! this module protongen saw every repack twice — as that shortcut and as that
//! sideload, each with its own `game_memory` slot — and knew nothing of the
//! repack's real prefix (`<game>/pfx`) or the Proton build Nexus pinned for it.
//! So a copied umu command ran the game in umu's default prefix.
//!
//! [`absorb`] folds those mirrors into one [`GameSource::Nexus`] entry. Its
//! `app_id` is the Steam shortcut's appid when there is one, else the Heroic
//! hash (both are what Nexus's own `protongen_id` resolves to), so existing
//! tuning, favourites and Nexus's "Tune in protongen" id all keep working. The
//! frontend carries any tuning saved under an absorbed mirror over to that id.
//!
//! Nexus launches games; protongen never does. Writing a tuned config back
//! goes through `nexus-cli --set-launch` (see [`crate::ipc::apply_to_nexus`]),
//! never by editing `games.json` directly.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::games::{Game, GameSource, heroic_app_id};

/// Nexus's data directory. Fixed under `$HOME` (not `$XDG_DATA_HOME`), the
/// same way `nexus/paths.py` computes it.
pub fn data_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share/nexus"))
}

pub(crate) fn registry_path() -> Option<PathBuf> {
    data_dir().map(|d| d.join("games.json"))
}

/// What protongen needs to know about a Nexus game beyond [`Game`]'s fields.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NexusInfo {
    /// Nexus's key for the game — what `nexus-cli --set-launch` takes.
    pub slug: String,
    /// The game's own Wine prefix (`<game>/pfx`).
    pub wine_prefix: Option<String>,
    /// The Proton build Nexus runs it with, by folder name: the per-game
    /// profile's pin, else the build recorded at install.
    pub proton: Option<String>,
    /// The ids of the mirrors this entry absorbed, best source of saved
    /// tuning first (the one Nexus last imported from protongen, per its
    /// profile's `source`).
    pub alias_ids: Vec<u32>,
    /// The Heroic sideload Nexus keeps for it, whose playtime adds to Nexus's.
    pub heroic_app_name: Option<String>,
    /// Unix seconds; Nexus records it as a local ISO timestamp.
    pub last_played: Option<u64>,
    pub playtime_minutes: Option<u32>,
}

/// One `games.json` entry, read leniently: every field optional, unknown
/// fields ignored, so a newer Nexus adding keys never hides the library.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Entry {
    slug: Option<String>,
    display_name: Option<String>,
    exe: Option<String>,
    wine_prefix: Option<String>,
    /// Nexus's Steam shortcut appid — the same u32 protongen reads from
    /// `shortcuts.vdf`. A JSON number, so read wide and range-checked.
    steam_appid: Option<u64>,
    heroic_app_name: Option<String>,
    proton: Option<String>,
    artwork_dir: Option<String>,
    last_played: Option<String>,
    playtime_seconds: Option<u64>,
    launch: Option<Launch>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Launch {
    proton: Option<String>,
    source: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Registry {
    games: std::collections::BTreeMap<String, serde_json::Value>,
}

/// A deterministic id for a repack with neither a Steam shortcut nor a Heroic
/// entry. Same FNV scheme and high bit as Heroic ids, over a prefixed key so
/// it can't land on a Heroic game's id.
fn slug_app_id(slug: &str) -> u32 {
    heroic_app_id(&format!("nexus:{slug}"))
}

fn non_empty(s: Option<String>) -> Option<String> {
    s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// `2026-10-02T13:29:42` (Nexus's `datetime.now().isoformat()`, local time,
/// no zone) to Unix seconds. Read as UTC: off by the zone offset at most,
/// which only nudges the library's "recent" sort.
fn parse_iso_local(s: &str) -> Option<u64> {
    let (date, time) = s.trim().split_once('T')?;
    let mut d = date.splitn(3, '-').map(|p| p.parse::<i64>().ok());
    let (y, m, day) = (d.next()??, d.next()??, d.next()??);
    let mut t = time.get(..8)?.splitn(3, ':').map(|p| p.parse::<i64>().ok());
    let (hh, mm, ss) = (t.next()??, t.next()??, t.next()??);
    if !(1..=12).contains(&m) || !(1..=31).contains(&day) || hh > 23 || mm > 59 || ss > 60 {
        return None;
    }
    // Days from civil (Howard Hinnant's algorithm).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(days * 86_400 + hh * 3600 + mm * 60 + ss).ok()
}

/// Parse a `games.json` document into protongen games. Entries without a
/// slug or exe are skipped (nothing to tune or launch); a malformed entry
/// never hides the rest.
fn parse_registry(text: &str) -> Vec<Game> {
    let Ok(reg) = serde_json::from_str::<Registry>(text) else {
        return Vec::new();
    };
    reg.games
        .into_iter()
        .filter_map(|(key, v)| {
            let e: Entry = serde_json::from_value(v).ok()?;
            let slug = non_empty(e.slug).unwrap_or(key);
            let exe = non_empty(e.exe)?;
            let shortcut = e.steam_appid.and_then(|id| u32::try_from(id).ok()).filter(|&id| id != 0);
            let heroic_app_name = non_empty(e.heroic_app_name);
            let heroic = heroic_app_name.as_deref().map(heroic_app_id);
            let app_id = shortcut.or(heroic).unwrap_or_else(|| slug_app_id(&slug));

            // Prefer the mirror Nexus says it last imported from.
            let from_heroic = e
                .launch
                .as_ref()
                .and_then(|l| l.source.as_deref())
                .is_some_and(|s| s.contains("Heroic"));
            let mut alias_ids: Vec<u32> = if from_heroic {
                vec![heroic, shortcut]
            } else {
                vec![shortcut, heroic]
            }
            .into_iter()
            .flatten()
            .filter(|&id| id != app_id)
            .collect();
            alias_ids.dedup();

            let proton = e.launch.as_ref().and_then(|l| non_empty(l.proton.clone())).or(non_empty(e.proton));
            let exe_path = PathBuf::from(&exe);
            Some(Game {
                app_id,
                name: non_empty(e.display_name).unwrap_or_else(|| slug.clone()),
                source: GameSource::Nexus,
                install_dir: exe_path.parent().map(Path::to_path_buf),
                executable: Some(exe),
                installed: true,
                heroic_id: None,
                art_url: non_empty(e.artwork_dir),
                nexus: Some(NexusInfo {
                    slug,
                    wine_prefix: non_empty(e.wine_prefix),
                    proton,
                    alias_ids,
                    heroic_app_name,
                    last_played: e.last_played.as_deref().and_then(parse_iso_local),
                    playtime_minutes: e
                        .playtime_seconds
                        .map(|s| u32::try_from(s / 60).unwrap_or(u32::MAX)),
                }),
            })
        })
        .collect()
}

/// Nexus's games, or nothing when Nexus isn't installed or its registry
/// can't be read.
pub fn list() -> Vec<Game> {
    registry_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|t| parse_registry(&t))
        .unwrap_or_default()
}

/// Replace the Steam-shortcut and Heroic mirrors of each Nexus game with the
/// Nexus entry itself. A mirror is matched by id (the shortcut appid / Heroic
/// hash Nexus recorded) or, failing that, by pointing at the same exe — a
/// shortcut re-added by hand gets a new appid but the same target.
pub fn absorb(mut games: Vec<Game>, nexus: Vec<Game>) -> Vec<Game> {
    if nexus.is_empty() {
        return games;
    }
    let mirrored = |g: &Game| {
        nexus.iter().any(|n| {
            let info = n.nexus.as_ref();
            g.app_id == n.app_id
                || info.is_some_and(|i| i.alias_ids.contains(&g.app_id))
                || (g.source != GameSource::Steam
                    && g.executable.is_some()
                    && g.executable == n.executable)
        })
    };
    games.retain(|g| !mirrored(g));
    games.extend(nexus.into_iter().map(crate::games::checked));
    games
}

/// `nexus-cli`, Nexus's backend: on `$PATH`, else where Nexus's installer
/// links it (`~/.local/bin`, which a desktop-launched protongen's `$PATH`
/// often lacks).
pub fn cli_path() -> Option<PathBuf> {
    crate::which::find("nexus-cli").or_else(|| {
        let p = PathBuf::from(std::env::var_os("HOME")?).join(".local/bin/nexus-cli");
        p.is_file().then_some(p)
    })
}

/// Longest a `--set-launch` may run. It rewrites launch.sh, the desktop entry
/// and the Steam/Heroic entries; seconds at most, so a minute means stuck.
const CLI_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// Whether `code` is shaped like a protongen share code: the prefix, then
/// base64 only, within the size protongen's own decoder accepts.
pub fn is_share_code(code: &str) -> bool {
    const PREFIX: &str = "protongen:v1:";
    code.len() <= 32 * 1024
        && code.strip_prefix(PREFIX).is_some_and(|b64| {
            !b64.is_empty() && b64.bytes().all(|b| b.is_ascii_alphanumeric() || b"+/=".contains(&b))
        })
}

/// Hand a tuned config to Nexus: `nexus-cli --set-launch <slug> <code>`,
/// which saves it as the game's launch profile and re-applies it to every
/// launcher Nexus keeps for it (launch.sh, desktop entry, Steam shortcut,
/// Heroic entry). Nexus owns those files; protongen only asks. Arguments go
/// straight to `execve`, never through a shell. Returns Nexus's own report.
pub fn set_launch(cli: &Path, slug: &str, code: &str) -> Result<String, String> {
    use std::io::Read;
    use std::process::{Command, Stdio};

    let mut child = Command::new(cli)
        .args(["--set-launch", slug, code])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("couldn't run {}: {e}", cli.display()))?;

    // Drain both pipes on threads so a chatty child can't block on a full
    // pipe while we wait for it to exit.
    let drain = |r: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut s = String::new();
            if let Some(mut r) = r {
                let _ = r.read_to_string(&mut s);
            }
            s
        })
    };
    let out = drain(child.stdout.take().map(|r| Box::new(r) as Box<dyn Read + Send>));
    let err = drain(child.stderr.take().map(|r| Box::new(r) as Box<dyn Read + Send>));

    let started = std::time::Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() > CLI_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("nexus-cli didn't finish within a minute and was stopped".into());
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(50)),
            Err(e) => return Err(format!("couldn't wait for nexus-cli: {e}")),
        }
    };
    let (out, err) = (out.join().unwrap_or_default(), err.join().unwrap_or_default());
    if status.success() {
        Ok(out.trim().to_string())
    } else {
        let msg = if err.trim().is_empty() { out } else { err };
        Err(format!("nexus-cli failed: {}", msg.trim()))
    }
}

/// Box art for a Nexus game, from its artwork folder:
/// `<dir>/{cover,hero,wide}.{png,jpg}`.
pub fn art_file(artwork_dir: &str, kind: &str) -> Option<PathBuf> {
    let stem = match kind {
        "portrait" => "cover",
        "hero" => "hero",
        _ => "wide",
    };
    ["png", "jpg", "jpeg", "webp"]
        .iter()
        .map(|ext| Path::new(artwork_dir).join(format!("{stem}.{ext}")))
        .find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"version": 1, "games": {
      "007-first-light": {"slug": "007-first-light", "display_name": "007 First Light",
        "exe": "/g/007/Retail/007FirstLight.exe", "wine_prefix": "/g/007/pfx",
        "steam_appid": 3632060096, "heroic_app_name": "g5-RoqR1AZ3roKLPv7o3AF",
        "proton": "proton-cachyos-slr", "artwork_dir": "/a/007",
        "launch": {"proton": "GE-Proton10-3", "env": [], "source": "protongen (Steam entry)"}},
      "dawnwalker": {"slug": "dawnwalker", "display_name": "The Blood of Dawnwalker",
        "exe": "/g/dw/Dawnwalker.exe", "steam_appid": 2515201647,
        "heroic_app_name": "R1P-IKBw0vP4MpQ4c5W54r",
        "launch": {"source": "protongen (Heroic entry)"}},
      "witcher": {"slug": "witcher", "display_name": "The Witcher 3", "exe": "/g/w3/witcher3.exe",
        "steam_appid": null, "heroic_app_name": "2b3qYQvYY8FjBs72DuHSLP",
        "proton": "proton-cachyos-slr", "last_played": "2026-10-02T13:29:42", "playtime_seconds": 7260},
      "loner": {"slug": "loner", "exe": "/g/loner/l.exe"},
      "broken": {"slug": "broken", "exe": 42},
      "no-exe": {"slug": "no-exe", "display_name": "Nothing to run"}
    }}"#;

    fn by_slug<'a>(games: &'a [Game], slug: &str) -> &'a Game {
        games.iter().find(|g| g.nexus.as_ref().unwrap().slug == slug).unwrap()
    }

    #[test]
    fn ids_follow_nexus_own_protongen_id() {
        let games = parse_registry(SAMPLE);
        assert_eq!(games.len(), 4, "the bad-exe and no-exe entries are skipped");
        // Steam shortcut appid first, then the Heroic hash, then a slug hash.
        assert_eq!(by_slug(&games, "007-first-light").app_id, 3_632_060_096);
        assert_eq!(by_slug(&games, "witcher").app_id, heroic_app_id("2b3qYQvYY8FjBs72DuHSLP"));
        let loner = by_slug(&games, "loner");
        assert_eq!(loner.app_id, slug_app_id("loner"));
        assert!(loner.app_id & 0x8000_0000 != 0);
        assert_eq!(loner.name, "loner");
    }

    #[test]
    fn aliases_put_the_mirror_nexus_imported_from_first() {
        let games = parse_registry(SAMPLE);
        let dw = by_slug(&games, "dawnwalker");
        assert_eq!(dw.app_id, 2_515_201_647);
        assert_eq!(dw.nexus.as_ref().unwrap().alias_ids, vec![heroic_app_id("R1P-IKBw0vP4MpQ4c5W54r")]);
        let w = by_slug(&games, "witcher");
        assert!(w.nexus.as_ref().unwrap().alias_ids.is_empty(), "its only mirror is its own id");
    }

    #[test]
    fn carries_prefix_pinned_proton_and_play_stats() {
        let games = parse_registry(SAMPLE);
        let bond = by_slug(&games, "007-first-light").nexus.clone().unwrap();
        assert_eq!(bond.wine_prefix.as_deref(), Some("/g/007/pfx"));
        assert_eq!(bond.proton.as_deref(), Some("GE-Proton10-3"), "the profile's pin wins");
        let w = by_slug(&games, "witcher").nexus.clone().unwrap();
        assert_eq!(w.proton.as_deref(), Some("proton-cachyos-slr"));
        assert_eq!(w.playtime_minutes, Some(121));
        assert_eq!(w.last_played, Some(1_790_947_782));
        assert_eq!(by_slug(&games, "007-first-light").art_url.as_deref(), Some("/a/007"));
    }

    #[test]
    fn garbage_is_an_empty_library_not_a_panic() {
        assert!(parse_registry("").is_empty());
        assert!(parse_registry("[]").is_empty());
        assert!(parse_registry(r#"{"games": 3}"#).is_empty());
    }

    #[test]
    fn iso_parse() {
        assert_eq!(parse_iso_local("1970-01-01T00:00:00"), Some(0));
        assert_eq!(parse_iso_local("2000-03-01T12:00:00.123"), Some(951_912_000));
        assert_eq!(parse_iso_local("2026-13-01T00:00:00"), None);
        assert_eq!(parse_iso_local("yesterday"), None);
    }

    fn plain(app_id: u32, source: GameSource, exe: Option<&str>) -> Game {
        Game {
            app_id,
            name: format!("g{app_id}"),
            source,
            executable: exe.map(str::to_string),
            installed: true,
            heroic_id: None,
            install_dir: None,
            art_url: None,
            nexus: None,
        }
    }

    #[test]
    fn absorb_folds_mirrors_by_id_or_exe_and_keeps_the_rest() {
        let nexus = parse_registry(SAMPLE);
        let dw_heroic = heroic_app_id("R1P-IKBw0vP4MpQ4c5W54r");
        let games = vec![
            plain(3_632_060_096, GameSource::NonSteam, Some("/g/007/Retail/007FirstLight.exe")),
            plain(dw_heroic, GameSource::Heroic, Some("/g/dw/Dawnwalker.exe")),
            // A shortcut re-added by hand: new appid, same target.
            plain(123, GameSource::NonSteam, Some("/g/w3/witcher3.exe")),
            plain(1_245_620, GameSource::Steam, None),
            plain(456, GameSource::NonSteam, Some("/other/game.exe")),
        ];
        let out = absorb(games, nexus);
        let ids: Vec<u32> = out.iter().filter(|g| g.source != GameSource::Nexus).map(|g| g.app_id).collect();
        assert_eq!(ids, vec![1_245_620, 456]);
        assert_eq!(out.iter().filter(|g| g.source == GameSource::Nexus).count(), 4);
    }

    #[test]
    fn share_code_shape() {
        assert!(is_share_code("protongen:v1:eyJuYW1lIjoieCJ9"));
        assert!(!is_share_code("protongen:v1:"));
        assert!(!is_share_code("--help"));
        assert!(!is_share_code("protongen:v1:abc def"));
        assert!(!is_share_code("protongen:v1:abc;rm"));
        assert!(!is_share_code(&format!("protongen:v1:{}", "A".repeat(33 * 1024))));
    }

    /// A stand-in nexus-cli: a shell script that echoes its argv, or fails.
    fn fake_cli(name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let p = std::env::temp_dir().join(format!("protongen-fake-nexus-{}-{name}", std::process::id()));
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    #[test]
    fn set_launch_passes_argv_verbatim_and_reports_output() {
        let cli = fake_cli("ok", r#"printf '%s|' "$@""#);
        let out = set_launch(&cli, "my game; rm -rf ~", "protongen:v1:QQ==").unwrap();
        assert_eq!(out, "--set-launch|my game; rm -rf ~|protongen:v1:QQ==|");
        std::fs::remove_file(cli).ok();
    }

    #[test]
    fn set_launch_surfaces_nexus_errors() {
        let cli = fake_cli("fail", "echo 'Error: no game named x' >&2; exit 1");
        let err = set_launch(&cli, "x", "protongen:v1:QQ==").unwrap_err();
        assert_eq!(err, "nexus-cli failed: Error: no game named x");
        std::fs::remove_file(cli).ok();
    }

    #[test]
    fn art_file_maps_kinds_to_nexus_names() {
        let dir = std::env::temp_dir().join(format!("protongen-nexus-art-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("cover.png"), b"x").unwrap();
        std::fs::write(dir.join("wide.jpg"), b"x").unwrap();
        let d = dir.to_str().unwrap();
        assert_eq!(art_file(d, "portrait"), Some(dir.join("cover.png")));
        assert_eq!(art_file(d, "header"), Some(dir.join("wide.jpg")));
        assert_eq!(art_file(d, "hero"), None);
        std::fs::remove_dir_all(&dir).ok();
    }
}
