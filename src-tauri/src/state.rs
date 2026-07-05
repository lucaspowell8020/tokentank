//! Rolling-window consumption math, plan verdict inputs, and ceiling
//! calibration from observed limit events.

use crate::config::Config;
use crate::parser::{self, Event, Offsets};
use chrono::{Datelike, Duration as ChronoDuration, Local, TimeZone};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const FIVE_H: i64 = 5 * 3600;
const WEEK: i64 = 7 * 24 * 3600;

#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub five_h_cost: f64,
    pub five_h_ceiling: f64,
    /// Epoch seconds when the current 5-hour session block ends.
    /// None when no session is active (next message starts a fresh block).
    pub five_h_reset: Option<i64>,
    pub session_active: bool,
    pub weekly_cost: f64,
    pub weekly_ceiling: f64,
    /// Epoch seconds of the next weekly quota reset (needs weekly_reset in
    /// config); None means the weekly gauge is a rolling 7-day approximation.
    pub weekly_reset: Option<i64>,
    pub burn_per_hour: f64,
    pub today_cost: f64,
    pub month_cost: f64,
    pub plan: Option<String>,
    pub plan_price: f64,
    pub plan_multiple: f64,
    pub plan_detected: bool,
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
    dirs::config_dir().map(|d| d.join("tokentank").join("calibration.json"))
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

    /// The current 5-hour session block, mirroring how Claude's session
    /// windows behave: a block opens with the first message after the
    /// previous block expires and lasts five hours. Returns (start, end)
    /// when a block is active at `now`.
    fn session_block(&self, now: i64) -> Option<(i64, i64)> {
        let mut block_start: Option<i64> = None;
        for e in &self.events {
            if e.ts > now {
                break;
            }
            match block_start {
                None => block_start = Some(e.ts),
                Some(bs) if e.ts >= bs + FIVE_H => block_start = Some(e.ts),
                _ => {}
            }
        }
        block_start.and_then(|bs| (now < bs + FIVE_H).then_some((bs, bs + FIVE_H)))
    }

    /// The active weekly quota window. With a configured anchor ("wed 05:59")
    /// this is the fixed window Claude actually uses; otherwise a rolling
    /// 7-day approximation with no known reset instant.
    fn weekly_window(&self, now: i64) -> (i64, Option<i64>) {
        if let Some((weekday, h, m)) = self.cfg.weekly_reset {
            let now_local = Local
                .timestamp_opt(now, 0)
                .single()
                .unwrap_or_else(Local::now);
            for days_back in 0..8 {
                let day = now_local.date_naive() - ChronoDuration::days(days_back);
                if day.weekday() != weekday {
                    continue;
                }
                let Some(anchor) = day
                    .and_hms_opt(h, m, 0)
                    .and_then(|ndt| Local.from_local_datetime(&ndt).single())
                else {
                    continue;
                };
                let ts = anchor.timestamp();
                if ts <= now {
                    return (ts, Some(ts + WEEK));
                }
            }
        }
        (now - WEEK, None)
    }

    pub fn needs_setup(&self) -> bool {
        self.cfg.plan.is_none()
    }

    /// Apply the setup wizard's answers. Percentages come from the Claude
    /// app's Settings -> Usage panel; a ceiling derives as spend ÷ fraction
    /// when there's enough spend in the window for the math to be stable.
    /// Returns nothing; call snapshot() after.
    pub fn apply_setup(
        &mut self,
        now: i64,
        plan: &str,
        weekly_reset: Option<&str>,
        session_pct: Option<f64>,
        week_pct: Option<f64>,
    ) {
        let plan = plan.trim().to_lowercase();
        if !["pro", "max_5x", "max_20x", "api"].contains(&plan.as_str()) {
            return;
        }
        let (default_5h, default_wk) = crate::config::plan_default_ceilings(Some(&plan));
        self.cfg.plan = Some(plan.clone());
        self.cfg.five_h_ceiling = default_5h;
        self.cfg.weekly_ceiling = default_wk;

        let weekly_reset_str = weekly_reset.map(|s| s.trim().to_lowercase());
        if let Some(parsed) = weekly_reset_str
            .as_deref()
            .and_then(crate::config::parse_weekly_reset)
        {
            self.cfg.weekly_reset = Some(parsed);
        }

        // Start from what's on disk so wizard re-runs don't wipe unrelated
        // settings (e.g. the autostart choice).
        let mut settings = crate::config::load_settings();
        settings.plan = Some(plan.clone());
        settings.weekly_reset = weekly_reset_str.clone();
        settings.five_h_ceiling = None;
        settings.weekly_ceiling = None;

        if plan != "api" {
            // Session ceiling from panel percentage, if a session is active
            // and there's enough spend for the division to mean something.
            if let (Some(pct), Some((start, _))) =
                (session_pct.filter(|p| *p >= 1.0), self.session_block(now))
            {
                let spend = self.cost_between(start - 1, now);
                if spend >= 2.0 {
                    let ceiling = spend / (pct / 100.0);
                    self.cfg.five_h_ceiling = ceiling;
                    settings.five_h_ceiling = Some(ceiling);
                }
            }
            // Weekly ceiling from panel percentage, using the (possibly just
            // configured) anchor window.
            if let Some(pct) = week_pct.filter(|p| *p >= 1.0) {
                let (week_start, _) = self.weekly_window(now);
                let spend = self.cost_between(week_start, now);
                if spend >= 10.0 {
                    let ceiling = spend / (pct / 100.0);
                    self.cfg.weekly_ceiling = ceiling;
                    settings.weekly_ceiling = Some(ceiling);
                }
            }
        }

        crate::config::save_settings(&settings);
    }

    pub fn snapshot(&self, now: i64) -> Snapshot {
        let block = self.session_block(now);
        let five_h_cost = block.map_or(0.0, |(start, _)| self.cost_between(start - 1, now));
        let five_h_reset = block.map(|(_, end)| end);

        let (week_start, weekly_reset) = self.weekly_window(now);
        let weekly_cost = self.cost_between(week_start, now);
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
            five_h_reset,
            session_active: block.is_some(),
            weekly_cost,
            weekly_ceiling,
            weekly_reset,
            burn_per_hour,
            today_cost,
            month_cost,
            plan: self.cfg.plan.clone(),
            plan_price,
            plan_multiple,
            plan_detected: self.cfg.plan_detected,
            calibrated: self.calibration.five_h.is_some() || self.calibration.weekly.is_some(),
            remaining,
        }
    }
}
