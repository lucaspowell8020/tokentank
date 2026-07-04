//! Reads the shared claude_usage.config.json (same file the CLI dashboard
//! uses) plus gauge-specific ceiling overrides.

use serde::Deserialize;
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
}

#[derive(Debug, Clone)]
pub struct Config {
    pub plan: Option<String>,
    pub plan_prices: HashMap<String, f64>,
    pub pricing: HashMap<String, TierPrice>,
    pub default_tier: String,
    pub cache_write_mult: f64,
    pub cache_read_mult: f64,
    /// Estimated API-equivalent-$ ceilings (5-hour window, rolling week).
    pub five_h_ceiling: f64,
    pub weekly_ceiling: f64,
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

/// Estimated ceilings per plan, API-equivalent dollars. These are rough
/// community-style estimates, clearly labelled in the UI; observed limit
/// events override them (see state::Calibration).
fn default_ceilings(plan: Option<&str>) -> (f64, f64) {
    match plan {
        Some("pro") => (15.0, 100.0),
        Some("max_5x") => (75.0, 500.0),
        Some("max_20x") => (300.0, 2000.0),
        Some("api") => (f64::INFINITY, f64::INFINITY),
        _ => (75.0, 500.0), // unknown: assume mid tier
    }
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

    Config {
        plan,
        plan_prices,
        pricing,
        default_tier: raw.default_model_tier.unwrap_or_else(|| "sonnet".into()),
        cache_write_mult: raw.cache_write_mult.unwrap_or(1.25),
        cache_read_mult: raw.cache_read_mult.unwrap_or(0.10),
        five_h_ceiling: five_h,
        weekly_ceiling: weekly,
    }
}
