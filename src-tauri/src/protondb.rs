//! Opt-in ProtonDB lookup for a Steam app id.
//!
//! Uses the official summary endpoint, which returns only compatibility stats
//! (tier / confidence / score / report count) — no launch commands. Fetched
//! synchronously via `ehttp::fetch_blocking`; the Tauri command wraps it in a
//! blocking task so it never stalls the UI.

use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Tier {
    pub tier: String,
    pub total: u64,
    pub confidence: String,
    /// Parsed from the API; surfaced in the chip hover.
    pub trending: String,
    pub best: String,
}

/// `https://www.protondb.com/app/<appid>` — the human page with community tips.
pub fn page_url(appid: u32) -> String {
    format!("https://www.protondb.com/app/{appid}")
}

/// Blocking fetch of a game's ProtonDB summary. Returns a `Tier` or an error
/// message suitable for display.
pub fn fetch_blocking(appid: u32) -> Result<Tier, String> {
    let url = format!("https://www.protondb.com/api/v1/reports/summaries/{appid}.json");
    let request = ehttp::Request::get(url);
    match ehttp::fetch_blocking(&request) {
        Ok(resp) if resp.ok => parse(resp.text().unwrap_or("")),
        Ok(resp) => Err(format!("HTTP {} {}", resp.status, resp.status_text)),
        Err(e) => Err(e),
    }
}

fn parse(body: &str) -> Result<Tier, String> {
    let v: serde_json::Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("unknown").to_string();
    if v.get("tier").is_none() {
        return Err("no ProtonDB reports for this game".to_string());
    }
    Ok(Tier {
        tier: s("tier"),
        trending: s("trendingTier"),
        best: s("bestReportedTier"),
        confidence: s("confidence"),
        total: v.get("total").and_then(|x| x.as_u64()).unwrap_or(0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_summary() {
        let t = parse(
            r#"{"tier":"platinum","total":1234,"confidence":"strong",
                "trendingTier":"gold","bestReportedTier":"platinum"}"#,
        )
        .expect("valid payload");
        assert_eq!(t.tier, "platinum");
        assert_eq!(t.total, 1234);
        assert_eq!(t.confidence, "strong");
        assert_eq!(t.trending, "gold");
        assert_eq!(t.best, "platinum");
    }

    #[test]
    fn missing_tier_is_error() {
        // The API returns a bare object for games with no reports.
        assert!(parse(r#"{"total":0}"#).is_err());
    }

    #[test]
    fn optional_fields_default_to_unknown() {
        let t = parse(r#"{"tier":"gold"}"#).expect("tier alone is enough");
        assert_eq!(t.tier, "gold");
        assert_eq!(t.trending, "unknown");
        assert_eq!(t.best, "unknown");
        assert_eq!(t.confidence, "unknown");
        assert_eq!(t.total, 0);
    }

    #[test]
    fn malformed_json_is_error() {
        assert!(parse("<html>404</html>").is_err());
    }
}
