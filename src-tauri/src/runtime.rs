//! Discover installed Proton compatibility tools (read-only).
//!
//! Two sources:
//!   1. Custom tools in `compatibilitytools.d` (system + user), each described
//!      by a `compatibilitytool.vdf` giving an internal name + display name.
//!   2. Valve-bundled Proton under `steamapps/common/Proton*`, labelled from
//!      its `version` file.

use std::path::{Path, PathBuf};

use steamlocate::SteamDir;

use crate::params::ConfigWarning;
use crate::steam;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeKind {
    System,
    User,
    Bundled,
    /// From a directory the user added under Settings → Paths. Distinct from
    /// `System`/`User` so the picker can show where it came from — which is how
    /// the user confirms their configured dir actually worked.
    Custom,
}

impl RuntimeKind {
    pub fn label(&self) -> &'static str {
        match self {
            RuntimeKind::System => "system",
            RuntimeKind::User => "user",
            RuntimeKind::Bundled => "valve",
            RuntimeKind::Custom => "custom",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Runtime {
    /// Internal tool name used in Steam's CompatToolMapping (best-effort for bundled).
    pub internal_name: String,
    /// Human-friendly name as shown in Steam's compatibility dropdown.
    pub display_name: String,
    pub kind: RuntimeKind,
    /// Install path on disk; used as PROTONPATH in umu mode.
    pub path: PathBuf,
}

/// System-wide compat tools directory (shipped by distro packages).
const SYSTEM_COMPAT_DIR: &str = "/usr/share/steam/compatibilitytools.d";

/// Discover all runtimes available to the given Steam install.
///
/// `extra_dirs` (from Settings) are additional `compatibilitytools.d`-shaped
/// directories — one sub-folder per Proton build, each with a
/// `compatibilitytool.vdf`. Getting that layout wrong is the usual mistake, so a
/// configured dir that yields nothing warns rather than going quiet.
pub fn discover(
    dir: &SteamDir,
    extra_dirs: &[String],
    warn: &mut Vec<ConfigWarning>,
) -> Vec<Runtime> {
    let mut runtimes = Vec::new();

    scan_compat_dir(Path::new(SYSTEM_COMPAT_DIR), RuntimeKind::System, &mut runtimes);
    scan_compat_dir(
        &steam::user_compat_tools_dir(dir),
        RuntimeKind::User,
        &mut runtimes,
    );
    scan_bundled(dir, &mut runtimes);

    for raw in crate::store::Paths::clean(extra_dirs) {
        let path = Path::new(raw);
        if !path.is_dir() {
            warn.push(ConfigWarning::path(
                "Proton directory",
                path.display(),
                "not a directory",
            ));
            continue;
        }
        let before = runtimes.len();
        scan_compat_dir(path, RuntimeKind::Custom, &mut runtimes);
        if runtimes.len() == before {
            warn.push(ConfigWarning::path(
                "Proton directory",
                path.display(),
                "no Proton builds here — expected one sub-folder per build, \
                 each containing a compatibilitytool.vdf",
            ));
        }
    }

    runtimes.sort_by(|a, b| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()));
    runtimes
}

/// First run of `n` consecutive ASCII digits in `s`, if any.
fn first_digit_run(s: &str, n: usize) -> Option<String> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if i - start >= n {
                return Some(s[start..start + n].to_string());
            }
        } else {
            i += 1;
        }
    }
    None
}

/// The installed proton-cachyos build date (`YYYYMMDD`), extracted from the
/// proton-cachyos runtime's display name, if present.
pub fn installed_cachyos_build(runtimes: &[Runtime]) -> Option<String> {
    runtimes
        .iter()
        .find(|r| r.display_name.to_lowercase().contains("cachyos"))
        .and_then(|r| first_digit_run(&r.display_name, 8))
}

/// Scan one `compatibilitytools.d` directory, parsing each tool's vdf.
fn scan_compat_dir(dir: &Path, kind: RuntimeKind, out: &mut Vec<Runtime>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let tool_dir = entry.path();
        if !tool_dir.is_dir() {
            continue;
        }
        let vdf = tool_dir.join("compatibilitytool.vdf");
        if let Ok(text) = std::fs::read_to_string(&vdf) {
            if let Some((internal, display)) = parse_compat_vdf(&text) {
                out.push(Runtime {
                    internal_name: internal,
                    display_name: display,
                    kind,
                    path: tool_dir,
                });
            }
        }
    }
}

/// Parse `compatibilitytools.compat_tools.<internal>.display_name` out of a
/// compatibilitytool.vdf. Tolerant of `//` comments (handled by the parser).
fn parse_compat_vdf(text: &str) -> Option<(String, String)> {
    let vdf = keyvalues_parser::parse(text).ok()?;
    let top = vdf.value.get_obj()?; // contents of "compatibilitytools"
    let compat_tools = top
        .get("compat_tools")
        .and_then(|v| v.first())
        .and_then(|v| v.get_obj())?;
    // There is exactly one tool entry; take the first.
    let (internal, vals) = compat_tools.iter().next()?;
    let tool_obj = vals.first().and_then(|v| v.get_obj());
    let display = tool_obj
        .and_then(|o| o.get("display_name"))
        .and_then(|v| v.first())
        .and_then(|v| v.get_str())
        .map(str::to_string)
        .unwrap_or_else(|| internal.to_string());
    Some((internal.to_string(), display))
}

/// Scan Valve-bundled Proton directories under steamapps/common.
fn scan_bundled(dir: &SteamDir, out: &mut Vec<Runtime>) {
    let Ok(common) = steam::common_dir(dir) else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(&common) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        // Valve Proton folders are named "Proton 9.0", "Proton - Experimental", etc.
        if !p.is_dir() || !name.starts_with("Proton") {
            continue;
        }
        // A real Proton tool has a toolmanifest.vdf.
        if !p.join("toolmanifest.vdf").is_file() {
            continue;
        }
        // The `version` file is "<buildid> <human-version>"; keep the readable part.
        let version = std::fs::read_to_string(p.join("version")).ok().and_then(|s| {
            s.trim()
                .split_once(char::is_whitespace)
                .map(|(_, v)| v.trim().to_string())
                .or_else(|| Some(s.trim().to_string()))
                .filter(|v| !v.is_empty())
        });
        let display = match version {
            Some(v) => format!("{name}  ({v})"),
            None => name.clone(),
        };
        out.push(Runtime {
            // Valve's internal mapping name isn't stored here; the user picks
            // bundled Proton from the dropdown directly.
            internal_name: "(select in Steam dropdown)".to_string(),
            display_name: display,
            kind: RuntimeKind::Bundled,
            path: p,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_cachyos_build() {
        assert_eq!(
            first_digit_run("proton-cachyos-11.0-20260602 (steam linux runtime)", 8).as_deref(),
            Some("20260602")
        );
        // 11, 0 are short runs; the 8-digit date is the first qualifying run.
        assert_eq!(first_digit_run("GE-Proton10-34", 8), None);

        let rts = vec![Runtime {
            internal_name: "proton-cachyos-slr".into(),
            display_name: "proton-cachyos-11.0-20260602 (steam linux runtime)".into(),
            kind: RuntimeKind::System,
            path: PathBuf::new(),
        }];
        assert_eq!(installed_cachyos_build(&rts).as_deref(), Some("20260602"));
    }


    /// A `compatibilitytools.d`-shaped tree with one build in it.
    fn temp_compat_dir(tag: &str) -> PathBuf {
        let root = std::env::temp_dir()
            .join(format!("protongen-compat-{}-{}", tag, std::process::id()));
        let tool = root.join("GE-Proton10-34");
        std::fs::create_dir_all(&tool).unwrap();
        std::fs::write(
            tool.join("compatibilitytool.vdf"),
            r#"
"compatibilitytools"
{
  "compat_tools"
  {
    "GE-Proton10-34"
    {
      "install_path" "."
      "display_name" "GE-Proton10-34"
      "from_oslist"  "windows"
      "to_oslist"    "linux"
    }
  }
}
"#,
        )
        .unwrap();
        root
    }

    #[test]
    fn an_extra_compat_dir_contributes_custom_runtimes() {
        // Driven through `scan_compat_dir` directly: `discover` needs a SteamDir,
        // and the configured-dir behaviour is entirely this call.
        let root = temp_compat_dir("ok");
        let mut out = Vec::new();
        scan_compat_dir(&root, RuntimeKind::Custom, &mut out);

        assert_eq!(out.len(), 1);
        assert_eq!(out[0].internal_name, "GE-Proton10-34");
        // The kind is how the picker shows the user their dir actually worked.
        assert_eq!(out[0].kind, RuntimeKind::Custom);
        assert_eq!(out[0].kind.label(), "custom");

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_configured_dir_with_no_tools_finds_nothing_to_warn_about() {
        // The empty case `discover` turns into a warning: the usual mistake is
        // pointing at a single Proton build rather than at its parent.
        let root = std::env::temp_dir()
            .join(format!("protongen-compat-empty-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let mut out = Vec::new();
        scan_compat_dir(&root, RuntimeKind::Custom, &mut out);
        assert!(out.is_empty());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn parses_cachyos_slr_vdf() {
        let text = r#"
"compatibilitytools"
{
  "compat_tools"
  {
    "proton-cachyos-slr"
    {
      "install_path" "."
      "display_name" "proton-cachyos-11.0-20260601 (steam linux runtime)"
      "from_oslist"  "windows"
      "to_oslist"    "linux"
    }
  }
}
"#;
        let (internal, display) = parse_compat_vdf(text).expect("should parse");
        assert_eq!(internal, "proton-cachyos-slr");
        assert_eq!(display, "proton-cachyos-11.0-20260601 (steam linux runtime)");
    }

    #[test]
    fn parses_vdf_with_comments() {
        // GE-Proton's template vdf contains // comments.
        let text = r#"
"compatibilitytools"
{
  "compat_tools"
  {
    "GE-Proton10-34" // Internal name of this tool
    {
      "install_path" "." // a comment
      "display_name" "GE-Proton10-34"
      "from_oslist"  "windows"
      "to_oslist"    "linux"
    }
  }
}
"#;
        let (internal, display) = parse_compat_vdf(text).expect("should parse");
        assert_eq!(internal, "GE-Proton10-34");
        assert_eq!(display, "GE-Proton10-34");
    }
}
