//! Opt-in local-LLM "log coach".
//!
//! Sends a game's Proton log (plus the built launch command and the detected
//! hardware) to an OpenAI-compatible chat endpoint — LM Studio, Ollama or a
//! llama.cpp server — and returns tuning suggestions: a prose analysis, and a
//! best-effort list of concrete catalog changes the frontend can offer to apply.
//!
//! HTTP mirrors the `protondb` / `update` modules: `ehttp::fetch_blocking`,
//! wrapped by the caller in `spawn_blocking`. Nothing here is applied
//! automatically — the response is advice the user chooses to act on, so the
//! read-only contract holds (the only outputs are still an on-screen string and,
//! if the user clicks, an in-app toggle).

use serde::{Deserialize, Serialize};

/// Endpoint used when the store field is empty (fresh install, no file yet).
/// The `/v1` base is what LM Studio serves; `/chat/completions` and `/models`
/// hang off it.
pub const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:1234/v1";
/// Model used when the store field is empty. A local default the user can change
/// in Settings; see the model note in the release plan for why gpt-oss-20b.
pub const DEFAULT_MODEL: &str = "gpt-oss-20b";

/// How much of the log tail we forward. The whole 64 KB tail can crowd out a
/// small model's context, and the error lines carry the signal, so the raw tail
/// is capped tighter here.
const MAX_TAIL_CHARS: usize = 12 * 1024;

/// Context for one analysis. The catalog allow-list and hardware summary are
/// added by the IPC command from `AppState`, not sent from the frontend.
#[derive(Clone, Debug, Deserialize)]
pub struct LlmRequest {
    /// The built launch command currently on screen.
    pub command: String,
    /// The selected game's name, or "" for a generic build.
    pub game_name: String,
    /// Pre-categorized "worth a look" lines from the log viewer.
    pub error_lines: Vec<String>,
    /// The raw log tail shown in the viewer (trimmed further here).
    pub log_tail: String,
}

/// One concrete change the model recommends, restricted to a catalog key so the
/// frontend can render a one-click "Apply" chip. `kind` is a hint; the frontend
/// re-checks the key against its env/wrapper maps before applying.
#[derive(Clone, Debug, Serialize)]
pub struct LlmChange {
    pub key: String,
    pub value: String,
    /// "env" or "wrap".
    pub kind: String,
    pub reason: String,
}

/// The result of an analysis: the model's prose plus any catalog-backed changes.
#[derive(Clone, Debug, Serialize)]
pub struct LlmSuggestion {
    /// The model's markdown analysis (also the broader setup suggestions).
    pub text: String,
    /// Concrete changes parsed from the model's JSON block, filtered to keys the
    /// catalog actually has. Empty when the model returned prose only.
    pub changes: Vec<LlmChange>,
}

/// Resolve the endpoint, falling back to the default when the store field is
/// empty, and trimming a trailing slash so path joins stay clean.
fn resolve_endpoint(endpoint: &str) -> String {
    let e = endpoint.trim();
    let e = if e.is_empty() { DEFAULT_ENDPOINT } else { e };
    e.trim_end_matches('/').to_string()
}

fn resolve_model(model: &str) -> String {
    let m = model.trim();
    if m.is_empty() { DEFAULT_MODEL.to_string() } else { m.to_string() }
}

/// GET `{endpoint}/models`, returning the served model ids for the Settings
/// dropdown / connection test. Blocking; the command wraps it.
pub fn list_models_blocking(endpoint: &str) -> Result<Vec<String>, String> {
    let url = format!("{}/models", resolve_endpoint(endpoint));
    let req = ehttp::Request::get(&url);
    let resp = ehttp::fetch_blocking(&req)?;
    if !resp.ok {
        return Err(format!("HTTP {} {} from {url}", resp.status, resp.status_text));
    }
    let body = resp.text().unwrap_or("");
    let v: serde_json::Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let ids = v
        .get("data")
        .and_then(|d| d.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.get("id").and_then(|x| x.as_str()).map(String::from))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(ids)
}

/// Send the log + context to the model and return its suggestions. Blocking; the
/// command wraps it in `spawn_blocking`.
pub fn suggest_blocking(
    req: LlmRequest,
    endpoint: &str,
    model: &str,
    hardware: &str,
    catalog_keys: &[String],
) -> Result<LlmSuggestion, String> {
    let url = format!("{}/chat/completions", resolve_endpoint(endpoint));
    let body = chat_body(&resolve_model(model), &req, hardware, catalog_keys);
    let payload = serde_json::to_vec(&body).map_err(|e| e.to_string())?;

    let mut request = ehttp::Request::post(&url, payload);
    request.headers.insert("Content-Type", "application/json");
    let resp = ehttp::fetch_blocking(&request)?;
    if !resp.ok {
        return Err(format!(
            "HTTP {} {} from {url} — is the local server running with a model loaded?",
            resp.status, resp.status_text
        ));
    }
    let text = resp.text().unwrap_or("");
    let content = parse_content(text)?;
    let changes = extract_changes(&content, catalog_keys);
    Ok(LlmSuggestion { text: content, changes })
}

/// A Fix recipe the model may recommend, passed in by the command from the
/// loaded recipe set. `index` is the IPC index the frontend applies by.
#[derive(Clone, Debug, Serialize)]
pub struct RecipeRef {
    pub index: u32,
    pub name: String,
    pub symptom: String,
    pub description: String,
}

/// Context for one troubleshooting request. The symptom is the user's own
/// words; the log signal is optional (they may be diagnosing before a first
/// run). The recipe list, hardware summary and catalog allow-list are added by
/// the command.
#[derive(Clone, Debug, Deserialize)]
pub struct TroubleshootRequest {
    pub symptom: String,
    pub command: String,
    pub game_name: String,
    #[serde(default)]
    pub error_lines: Vec<String>,
    #[serde(default)]
    pub has_log: bool,
}

/// The result of a troubleshooting request: prose, recommended existing recipes
/// (by IPC index), and any extra catalog-backed changes no recipe covers.
#[derive(Clone, Debug, Serialize)]
pub struct TroubleshootResult {
    pub text: String,
    pub recipes: Vec<u32>,
    pub changes: Vec<LlmChange>,
}

/// Diagnose a free-text symptom, recommending existing Fix recipes where they
/// fit and proposing catalog changes otherwise. Blocking; the command wraps it.
pub fn troubleshoot_blocking(
    req: TroubleshootRequest,
    recipes: &[RecipeRef],
    endpoint: &str,
    model: &str,
    hardware: &str,
    catalog_keys: &[String],
) -> Result<TroubleshootResult, String> {
    let url = format!("{}/chat/completions", resolve_endpoint(endpoint));
    let body = troubleshoot_body(&resolve_model(model), &req, recipes, hardware, catalog_keys);
    let payload = serde_json::to_vec(&body).map_err(|e| e.to_string())?;

    let mut request = ehttp::Request::post(&url, payload);
    request.headers.insert("Content-Type", "application/json");
    let resp = ehttp::fetch_blocking(&request)?;
    if !resp.ok {
        return Err(format!(
            "HTTP {} {} from {url} — is the local server running with a model loaded?",
            resp.status, resp.status_text
        ));
    }
    let content = parse_content(resp.text().unwrap_or(""))?;
    let valid: Vec<u32> = recipes.iter().map(|r| r.index).collect();
    Ok(TroubleshootResult {
        recipes: extract_recipe_indices(&content, &valid),
        changes: extract_changes(&content, catalog_keys),
        text: content,
    })
}

fn troubleshoot_body(
    model: &str,
    req: &TroubleshootRequest,
    recipes: &[RecipeRef],
    hardware: &str,
    catalog_keys: &[String],
) -> serde_json::Value {
    let recipe_list = if recipes.is_empty() {
        "(none available)".to_string()
    } else {
        recipes
            .iter()
            .map(|r| format!("- [{}] {} — {} ({})", r.index, r.name, r.symptom, r.description))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let log = if !req.has_log {
        "(no log captured yet)".to_string()
    } else if req.error_lines.is_empty() {
        "(log present, no error lines flagged)".to_string()
    } else {
        req.error_lines.join("\n")
    };
    let game = if req.game_name.trim().is_empty() { "(unnamed)" } else { req.game_name.trim() };
    let keys = catalog_keys.join(", ");

    let user = format!(
        "Game: {game}\n\
         Hardware: {hardware}\n\n\
         The problem, in the user's words:\n{}\n\n\
         Current launch command:\n{}\n\n\
         Flagged log lines (if any):\n{log}\n\n\
         Available troubleshooter recipes (recommend by index when one fits):\n{recipe_list}\n\n\
         Catalog keys you may recommend directly (use ONLY these exact keys):\n{keys}",
        req.symptom, req.command
    );

    serde_json::json!({
        "model": model,
        "temperature": 0.3,
        "stream": false,
        "messages": [
            { "role": "system", "content": TROUBLESHOOT_PROMPT },
            { "role": "user", "content": user },
        ],
    })
}

const TROUBLESHOOT_PROMPT: &str = "\
You are an expert assistant for Linux gaming with Proton on CachyOS. The user \
describes a problem with a game (crash, black screen, stutter, no audio, won't \
launch, etc.). Diagnose it and guide them to a fix.

You are given a list of ready-made troubleshooter recipes (each with an index) \
and a list of individual catalog keys. Prefer recommending a recipe when one \
clearly matches the symptom; otherwise recommend individual catalog changes.

Respond in two parts:
1. A concise markdown explanation: the most likely cause and what to try, in \
plain language. Mention broader setup fixes (drivers, session, runtime) when \
relevant.
2. THEN a fenced ```json code block with this exact shape:
{\"recipes\": [<indices from the recipe list>], \"changes\": [{\"key\": \"<catalog key>\", \"value\": \"<value>\", \"kind\": \"env\"|\"wrap\", \"reason\": \"<short why>\"}]}
Use recipe indices ONLY from the provided list, and catalog keys ONLY from the \
provided list. Use empty arrays when you have nothing concrete to recommend. \
Never invent indices or keys.";

/// Best-effort parse of the recommended recipe indices, filtered to the set the
/// command actually offered (so a hallucinated index is dropped) and de-duped.
fn extract_recipe_indices(content: &str, valid: &[u32]) -> Vec<u32> {
    let Some(json) = find_json_object(content) else { return Vec::new() };
    #[derive(Deserialize)]
    struct Raw {
        #[serde(default)]
        recipes: Vec<u32>,
    }
    let Ok(raw) = serde_json::from_str::<Raw>(json) else { return Vec::new() };
    let mut seen = std::collections::HashSet::new();
    raw.recipes
        .into_iter()
        .filter(|i| valid.contains(i) && seen.insert(*i))
        .collect()
}

/// Build the OpenAI chat-completions request body.
fn chat_body(
    model: &str,
    req: &LlmRequest,
    hardware: &str,
    catalog_keys: &[String],
) -> serde_json::Value {
    let tail: String = req.log_tail.chars().rev().take(MAX_TAIL_CHARS).collect();
    let tail: String = tail.chars().rev().collect();
    let errors = if req.error_lines.is_empty() {
        "(none flagged)".to_string()
    } else {
        req.error_lines.join("\n")
    };
    let game = if req.game_name.trim().is_empty() { "(unnamed)" } else { req.game_name.trim() };
    let keys = catalog_keys.join(", ");

    let user = format!(
        "Game: {game}\n\
         Hardware: {hardware}\n\n\
         Current launch command:\n{}\n\n\
         Flagged log lines:\n{errors}\n\n\
         Log tail:\n{tail}\n\n\
         Catalog keys you may recommend (use ONLY these exact keys):\n{keys}",
        req.command
    );

    serde_json::json!({
        "model": model,
        "temperature": 0.3,
        "stream": false,
        "messages": [
            { "role": "system", "content": SYSTEM_PROMPT },
            { "role": "user", "content": user },
        ],
    })
}

const SYSTEM_PROMPT: &str = "\
You are an expert assistant for Linux gaming with Proton on CachyOS. You help the \
user make a specific game run more smoothly by reading its Proton log, the launch \
command, and their hardware.

Respond in two parts:
1. A concise markdown analysis: what the log suggests is going wrong (or that it \
looks healthy), the most likely causes, and clear recommendations. Include broader \
setup advice too (drivers, Wayland/X11 session, Proton/runtime choice, GPU \
settings) when the logs or hardware hint at an easy win — but keep it grounded in \
what you can actually see.
2. THEN a fenced ```json code block with this exact shape:
{\"changes\": [{\"key\": \"<catalog key>\", \"value\": \"<value>\", \"kind\": \"env\"|\"wrap\", \"reason\": \"<short why>\"}]}
Only include a change when you are recommending a concrete catalog key from the \
allow-list the user provides; use the exact key string. Use an empty array if you \
have no concrete catalog change to recommend. Never invent keys that are not in the \
list.";

/// Pull `choices[0].message.content` out of the chat-completions response.
fn parse_content(body: &str) -> Result<String, String> {
    let v: serde_json::Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    // Some servers surface errors as `{ "error": { "message": ... } }`.
    if let Some(msg) = v.get("error").and_then(|e| e.get("message")).and_then(|m| m.as_str()) {
        return Err(msg.to_string());
    }
    v.get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .map(str::to_string)
        .ok_or_else(|| "the server returned no message content".to_string())
}

/// Best-effort parse of the model's JSON block into catalog-backed changes.
/// Anything unparseable, or any key not in the catalog, is silently dropped —
/// the prose is always shown regardless, so a model that ignores the format
/// still produces a useful result.
fn extract_changes(content: &str, catalog_keys: &[String]) -> Vec<LlmChange> {
    let Some(json) = find_json_object(content) else { return Vec::new() };

    #[derive(Deserialize)]
    struct RawChange {
        key: String,
        #[serde(default)]
        value: String,
        #[serde(default)]
        kind: String,
        #[serde(default)]
        reason: String,
    }
    #[derive(Deserialize)]
    struct Raw {
        #[serde(default)]
        changes: Vec<RawChange>,
    }

    let Ok(raw) = serde_json::from_str::<Raw>(json) else { return Vec::new() };
    raw.changes
        .into_iter()
        .filter(|c| catalog_keys.iter().any(|k| k == &c.key))
        .map(|c| LlmChange {
            key: c.key,
            value: c.value,
            kind: if c.kind == "wrap" { "wrap".into() } else { "env".into() },
            reason: c.reason,
        })
        .collect()
}

/// Locate the model's JSON object. Prefers a fenced ```json block; falls back to
/// a brace-balanced scan from the first `{` before an anchor key, so a bare
/// object (no fence) still parses. Anchored on `"changes"`/`"recipes"` so it
/// works for both the log coach and the troubleshooter.
fn find_json_object(content: &str) -> Option<&str> {
    const ANCHORS: [&str; 2] = ["\"changes\"", "\"recipes\""];
    let has_anchor = |s: &str| ANCHORS.iter().any(|a| s.contains(a));

    // 1) Fenced ```json ... ``` block.
    if let Some(start) = content.find("```json") {
        let after = &content[start + "```json".len()..];
        if let Some(end) = after.find("```") {
            let block = after[..end].trim();
            if has_anchor(block) {
                return Some(block);
            }
        }
    }
    // 2) Balanced-brace scan around the earliest anchor key.
    let anchor = ANCHORS.iter().filter_map(|a| content.find(a)).min()?;
    let open = content[..anchor].rfind('{')?;
    let bytes = content.as_bytes();
    let mut depth = 0i32;
    for (i, &b) in bytes.iter().enumerate().skip(open) {
        match b {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&content[open..=i]);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys() -> Vec<String> {
        vec!["PROTON_FSR4_UPGRADE".into(), "DXVK_HDR".into(), "mangohud".into()]
    }

    #[test]
    fn extracts_fenced_changes_and_filters_unknown_keys() {
        let content = "Analysis here.\n\n```json\n{\"changes\":[\
            {\"key\":\"PROTON_FSR4_UPGRADE\",\"value\":\"1\",\"kind\":\"env\",\"reason\":\"sharper\"},\
            {\"key\":\"NOT_IN_CATALOG\",\"value\":\"1\",\"kind\":\"env\",\"reason\":\"nope\"}]}\n```";
        let changes = extract_changes(content, &keys());
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].key, "PROTON_FSR4_UPGRADE");
        assert_eq!(changes[0].value, "1");
    }

    #[test]
    fn extracts_unfenced_object() {
        let content =
            "prose {\"changes\": [{\"key\": \"mangohud\", \"kind\": \"wrap\", \"reason\": \"fps\"}]} trailing";
        let changes = extract_changes(content, &keys());
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].key, "mangohud");
        assert_eq!(changes[0].kind, "wrap");
    }

    #[test]
    fn no_json_block_yields_no_changes() {
        assert!(extract_changes("just prose, no block", &keys()).is_empty());
    }

    #[test]
    fn extracts_recipe_indices_and_drops_hallucinated_ones() {
        let content = "Try the low-latency fix.\n\n```json\n{\"recipes\":[0,3,99],\"changes\":[]}\n```";
        // Only 0 and 3 were offered; 99 is hallucinated and must be dropped.
        assert_eq!(extract_recipe_indices(content, &[0, 3, 5]), vec![0, 3]);
    }

    #[test]
    fn troubleshoot_json_with_only_recipes_still_parses() {
        // No "changes" key at all — the anchor generalisation must still find it.
        let content = "Analysis.\n\n```json\n{\"recipes\":[2]}\n```";
        assert_eq!(extract_recipe_indices(content, &[2]), vec![2]);
        assert!(extract_changes(content, &keys()).is_empty());
    }

    #[test]
    fn parses_chat_content() {
        let body = r#"{"choices":[{"message":{"content":"hello"}}]}"#;
        assert_eq!(parse_content(body).unwrap(), "hello");
    }

    #[test]
    fn surfaces_server_error() {
        let body = r#"{"error":{"message":"no model loaded"}}"#;
        assert!(parse_content(body).unwrap_err().contains("no model loaded"));
    }

    #[test]
    fn endpoint_falls_back_and_trims_slash() {
        assert_eq!(resolve_endpoint(""), DEFAULT_ENDPOINT);
        assert_eq!(resolve_endpoint("http://x/v1/"), "http://x/v1");
    }
}
