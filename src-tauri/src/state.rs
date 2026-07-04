//! Rolling-window consumption math, plan verdict inputs, and ceiling
//! calibration from observed limit events.

use crate::config::Config;
use crate::parser::{self, Event, Offsets};
use chrono::{Local, TimeZone};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const FIVE_H: i64 = 5 * 3600;
const WEEK: i64 = 7 * 24 * 3600;

#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub five_h_cost: f64,
    pub five_h_ceiling: f64,
    pub weekly_cost: f64,
    pub weekly_ceiling: f64,
    pub burn_per_hour: f64,
    pub today_cost: f64,
    pub month_cost: f64,
    pub plan: Option<String>,
    pub plan_price: f64,
    pub plan_multiple: f64,
    pub calibrated: bool,
    /// Remaining fraction of the tighter gauge, for the tray icon.
    pub remaining: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Calibration {
    pub five_h: Option<f64>,
    pub weekly: Option<f64>,
}

fn calibration_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("claude-gauge").join("calibration.json"))
}

impl Calibration {
    fn load() -> Self {
        calibration_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    fn save(&self) {
        if let Some(path) = calibration_path() {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Ok(text) = serde_json::to_string_pretty(self) {
                let _ = std::fs::write(path, text);
            }
        }
    }
}

pub struct Gauge {
    cfg: Config,
    base: PathBuf,
    offsets: Offsets,
    events: Vec<Event>,
    calibration: Calibration,
}

impl Gauge {
    pub fn new(cfg: Config, base: PathBuf) -> Self {
        Self {
            cfg,
            base,
            offsets: Offsets::default(),
            events: Vec::new(),
            calibration: Calibration::load(),
        }
    }

    /// Pull new transcript lines, learn ceilings from any limit events,
    /// prune old data.
    pub fn refresh(&mut self, now: i64) {
        let new_events = parser::refresh(&self.cfg, &mut self.offsets, &self.base, now);
        if !new_events.is_empty() {
            self.events.extend(new_events.iter().copied());
            self.events.sort_by_key(|e| e.ts);
            let mut changed = false;
            for ev in new_events.iter().filter(|e| e.limit) {
                let spent_5h = self.cost_between(ev.ts - FIVE_H, ev.ts);
                let spent_wk = self.cost_between(ev.ts - WEEK, ev.ts);
                // A limit event tells us the real ceiling is at least what
                // was consumed leading up to it. Keep the largest observed.
                if spent_5h > self.calibration.five_h.unwrap_or(0.0) && spent_5h > 1.0 {
                    self.calibration.five_h = Some(spent_5h);
                    changed = true;
                }
                if spent_wk > self.calibration.weekly.unwrap_or(0.0) && spent_wk > 1.0 {
                    self.calibration.weekly = Some(spent_wk);
                    changed = true;
                }
            }
            if changed {
                self.calibration.save();
            }
        }
        let cutoff = now - 31 * 24 * 3600;
        self.events.retain(|e| e.ts >= cutoff);
    }

    fn cost_between(&self, from: i64, to: i64) -> f64 {
        self.events
            .iter()
            .filter(|e| e.ts > from && e.ts <= to)
            .map(|e| e.cost)
            .sum()
    }

    pub fn snapshot(&self, now: i64) -> Snapshot {
        let five_h_cost = self.cost_between(now - FIVE_H, now);
        let weekly_cost = self.cost_between(now - WEEK, now);
        let burn_per_hour = self.cost_between(now - 3600, now);
        let month_cost = self.cost_between(now - 30 * 24 * 3600, now);

        let midnight = Local::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .map(|dt| Local.from_local_datetime(&dt).single())
            .flatten()
            .map(|dt| dt.timestamp())
            .unwrap_or(now - 24 * 3600);
        let today_cost = self.cost_between(midnight, now);

        let five_h_ceiling = self.calibration.five_h.unwrap_or(self.cfg.five_h_ceiling);
        let weekly_ceiling = self.calibration.weekly.unwrap_or(self.cfg.weekly_ceiling);

        let plan_price = self
            .cfg
            .plan
            .as_deref()
            .and_then(|p| self.cfg.plan_prices.get(p))
            .copied()
            .unwrap_or(0.0);
        let plan_multiple = if plan_price > 0.0 { month_cost / plan_price } else { 0.0 };

        let rem = |used: f64, ceiling: f64| -> f64 {
            if !ceiling.is_finite() || ceiling <= 0.0 {
                1.0
            } else {
                (1.0 - used / ceiling).clamp(0.0, 1.0)
            }
        };
        let remaining = rem(five_h_cost, five_h_ceiling).min(rem(weekly_cost, weekly_ceiling));

        Snapshot {
            five_h_cost,
            five_h_ceiling,
            weekly_cost,
            weekly_ceiling,
            burn_per_hour,
            today_cost,
            month_cost,
            plan: self.cfg.plan.clone(),
            plan_price,
            plan_multiple,
            calibrated: self.calibration.five_h.is_some() || self.calibration.weekly.is_some(),
            remaining,
        }
    }
}
