//! Reads the shared claude_usage.config.json (same file the CLI dashboard
//! uses) plus gauge-specific ceiling overrides.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct TierPrice {
    pub input: f64,
    pub output: f64,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RawConfig {
    pub plan: Option<String>,
    #[serde(default)]
    pub plan_prices: HashMap<String, f64>,
    #[serde(default)]
    pub pricing: HashMap<String, TierPrice>,
    pub default_model_tier: Option<String>,
    pub cache_write_mult: Option<f64>,
    pub cache_read_mult: Option<f64>,
    /// Optional gauge overrides: {"five_h": 300.0, "weekly": 2000.0}
    #[serde(default)]
    pub gauge_ceilings: HashMap<String, f64>,
    /// Weekly quota anchor, e.g. "wed 05:59" (local time). Copy it from the
    /// Claude app's Settings -> Usage panel ("Resets Wed 5:59 AM").
    pub weekly_reset: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub plan: Option<String>,
    pub plan_prices: HashMap<String, f64>,
    pub pricing: HashMap<String, TierPrice>,
    pub default_tier: String,
    pub cache_write_mult: f64,
    pub cache_read_mult: f64,
    /// Estimated API-equivalent-$ ceilings (5-hour window, weekly quota).
    pub five_h_ceiling: f64,
    pub weekly_ceiling: f64,
    /// Weekly quota anchor: (weekday, hour, minute) in local time.
    pub weekly_reset: Option<(chrono::Weekday, u32, u32)>,
}

fn default_pricing() -> HashMap<String, TierPrice> {
    // Anthropic list prices, USD per 1M tokens (2026-06). Config overrides.
    let mut m = HashMap::new();
    m.insert("fable".into(), TierPrice { input: 10.0, output: 50.0 });
    m.insert("opus".into(), TierPrice { input: 5.0, output: 25.0 });
    m.insert("sonnet".into(), TierPrice { input: 3.0, output: 15.0 });
    m.insert("haiku".into(), TierPrice { input: 1.0, output: 5.0 });
    m
}

/// Estimated ceilings per plan, API-equivalent dollars (5-hour, weekly).
/// Anchored on a real Max 5x observation (2026-07: 17% of the session used
/// at ~$45, 19% of the week at ~$420) and scaled by tier multiple. Clearly
/// labelled as estimates in the UI; observed limit events and the
/// gauge_ceilings config key override them.
fn default_ceilings(plan: Option<&str>) -> (f64, f64) {
    match plan {
        Some("pro") => (50.0, 450.0),
        Some("max_5x") => (250.0, 2200.0),
        Some("max_20x") => (1000.0, 8800.0),
        Some("api") => (f64::INFINITY, f64::INFINITY),
        _ => (250.0, 2200.0), // unknown: assume mid tier
    }
}

/// Settings written by the in-app setup wizard. Layered over the shared
/// claude_usage.config.json (wizard wins), so the CLI tool's config is
/// never modified by the app.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GaugeSettings {
    pub plan: Option<String>,
    pub weekly_reset: Option<String>,
    pub five_h_ceiling: Option<f64>,
    pub weekly_ceiling: Option<f64>,
}

fn settings_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("tokentank").join("settings.json"))
}

pub fn load_settings() -> GaugeSettings {
    settings_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(t.trim_start_matches('\u{feff}')).ok())
        .unwrap_or_default()
}

pub fn save_settings(s: &GaugeSettings) {
    if let Some(path) = settings_path() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(text) = serde_json::to_string_pretty(s) {
            let _ = std::fs::write(path, text);
        }
    }
}

/// Estimated ceilings for a plan key — public so the wizard can reset to
/// tier defaults when the user changes plan without giving percentages.
pub fn plan_default_ceilings(plan: Option<&str>) -> (f64, f64) {
    default_ceilings(plan)
}

/// Parse "wed 05:59" (case-insensitive, local time) into (weekday, h, m).
pub fn parse_weekly_reset(s: &str) -> Option<(chrono::Weekday, u32, u32)> {
    let mut parts = s.split_whitespace();
    let day = match parts.next()?.to_lowercase().get(..3)? {
        "mon" => chrono::Weekday::Mon,
        "tue" => chrono::Weekday::Tue,
        "wed" => chrono::Weekday::Wed,
        "thu" => chrono::Weekday::Thu,
        "fri" => chrono::Weekday::Fri,
        "sat" => chrono::Weekday::Sat,
        "sun" => chrono::Weekday::Sun,
        _ => return None,
    };
    let mut hm = parts.next()?.split(':');
    let h: u32 = hm.next()?.parse().ok()?;
    let m: u32 = hm.next()?.parse().ok()?;
    (h < 24 && m < 60).then_some((day, h, m))
}

pub fn load() -> Config {
    let mut raw = RawConfig::default();
    let candidates: Vec<PathBuf> = vec![
        dirs::home_dir()
            .map(|h| h.join(".claude").join("claude_usage.config.json"))
            .unwrap_or_default(),
    ];
    for path in candidates {
        if let Ok(text) = std::fs::read_to_string(&path) {
            // Tolerate a UTF-8 BOM — Notepad and PowerShell add one.
            let text = text.trim_start_matches('\u{feff}');
            if let Ok(parsed) = serde_json::from_str::<RawConfig>(text) {
                raw = parsed;
                break;
            }
        }
    }

    let plan = raw
        .plan
        .as_deref()
        .map(|p| p.trim().to_lowercase())
        .filter(|p| ["pro", "max_5x", "max_20x", "api"].contains(&p.as_str()));

    let mut plan_prices: HashMap<String, f64> = HashMap::new();
    plan_prices.insert("pro".into(), 20.0);
    plan_prices.insert("max_5x".into(), 100.0);
    plan_prices.insert("max_20x".into(), 200.0);
    plan_prices.extend(raw.plan_prices);

    let (mut five_h, mut weekly) = default_ceilings(plan.as_deref());
    if let Some(v) = raw.gauge_ceilings.get("five_h") {
        five_h = *v;
    }
    if let Some(v) = raw.gauge_ceilings.get("weekly") {
        weekly = *v;
    }

    let mut pricing = default_pricing();
    pricing.extend(raw.pricing);

    let mut cfg = Config {
        plan,
        plan_prices,
        pricing,
        default_tier: raw.default_model_tier.unwrap_or_else(|| "sonnet".into()),
        cache_write_mult: raw.cache_write_mult.unwrap_or(1.25),
        cache_read_mult: raw.cache_read_mult.unwrap_or(0.10),
        five_h_ceiling: five_h,
        weekly_ceiling: weekly,
        weekly_reset: raw.weekly_reset.as_deref().and_then(parse_weekly_reset),
    };

    // Wizard settings win over the shared config file.
    let settings = load_settings();
    if let Some(p) = settings.plan.as_deref() {
        if ["pro", "max_5x", "max_20x", "api"].contains(&p) {
            let (f, w) = default_ceilings(Some(p));
            cfg.plan = Some(p.to_string());
            cfg.five_h_ceiling = f;
            cfg.weekly_ceiling = w;
        }
    }
    if let Some(wr) = settings.weekly_reset.as_deref().and_then(parse_weekly_reset) {
        cfg.weekly_reset = Some(wr);
    }
    if let Some(v) = settings.five_h_ceiling {
        cfg.five_h_ceiling = v;
    }
    if let Some(v) = settings.weekly_ceiling {
        cfg.weekly_ceiling = v;
    }
    cfg
}
