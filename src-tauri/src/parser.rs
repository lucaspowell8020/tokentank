//! Incremental JSONL transcript parser. Same semantics as the CLI
//! dashboard: per-event API-equivalent cost (cache reads at the read
//! multiplier, cache writes at the write multiplier) and limit-hit
//! detection via `<synthetic>` "limit reached" messages.

use crate::config::Config;
use chrono::DateTime;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy)]
pub struct Event {
    /// Epoch seconds.
    pub ts: i64,
    /// API-equivalent USD for this event (0 for non-usage events).
    pub cost: f64,
    /// True when this event is a usage-limit notice.
    pub limit: bool,
}

/// Tracks how far into each file we've read, so runtime refreshes only
/// parse appended lines.
#[derive(Default)]
pub struct Offsets(HashMap<PathBuf, u64>);

fn epoch(st: SystemTime) -> i64 {
    st.duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn tier_for<'a>(cfg: &'a Config, model: &str) -> Option<&'a crate::config::TierPrice> {
    let m = model.to_lowercase();
    for (tier, price) in &cfg.pricing {
        if m.contains(tier.as_str()) {
            return Some(price);
        }
    }
    cfg.pricing.get(&cfg.default_tier)
}

fn text_of(msg: &Value) -> String {
    match msg.get("content") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(blocks)) => blocks
            .iter()
            .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}

fn parse_line(cfg: &Config, line: &str) -> Option<Event> {
    let v: Value = serde_json::from_str(line).ok()?;
    let ts_str = v.get("timestamp")?.as_str()?;
    let ts = DateTime::parse_from_rfc3339(ts_str).ok()?.timestamp();

    let mut cost = 0.0;
    let mut limit = false;

    if let Some(msg) = v.get("message") {
        let model = msg.get("model").and_then(|m| m.as_str()).unwrap_or("unknown");
        if model == "<synthetic>" && text_of(msg).to_lowercase().contains("limit reached") {
            limit = true;
        }
        if let Some(usage) = msg.get("usage") {
            let g = |k: &str| usage.get(k).and_then(|x| x.as_u64()).unwrap_or(0) as f64;
            if let Some(price) = tier_for(cfg, model) {
                let inp = price.input / 1_000_000.0;
                let out = price.output / 1_000_000.0;
                cost = g("input_tokens") * inp
                    + g("cache_creation_input_tokens") * inp * cfg.cache_write_mult
                    + g("cache_read_input_tokens") * inp * cfg.cache_read_mult
                    + g("output_tokens") * out;
            }
        }
    }

    if cost == 0.0 && !limit {
        return None;
    }
    Some(Event { ts, cost, limit })
}

fn jsonl_files(base: &Path, mtime_cutoff: i64) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![base.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "jsonl") {
                let recent = entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .map(|t| epoch(t) >= mtime_cutoff)
                    .unwrap_or(true);
                if recent {
                    out.push(path);
                }
            }
        }
    }
    out
}

/// Read new content from `path` starting at the stored offset. Only whole
/// lines are consumed; a partial trailing line stays unread until the next
/// refresh.
fn read_new_lines(offsets: &mut Offsets, path: &Path) -> Vec<String> {
    let Ok(mut f) = std::fs::File::open(path) else { return Vec::new() };
    let start = *offsets.0.get(path).unwrap_or(&0);
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    if len <= start {
        // File shrank (rotated) — reset.
        if len < start {
            offsets.0.insert(path.to_path_buf(), 0);
        }
        return Vec::new();
    }
    if f.seek(SeekFrom::Start(start)).is_err() {
        return Vec::new();
    }
    let mut buf = Vec::with_capacity((len - start) as usize);
    if f.read_to_end(&mut buf).is_err() {
        return Vec::new();
    }
    let last_newline = match buf.iter().rposition(|&b| b == b'\n') {
        Some(i) => i + 1,
        None => return Vec::new(),
    };
    offsets.0.insert(path.to_path_buf(), start + last_newline as u64);
    String::from_utf8_lossy(&buf[..last_newline])
        .lines()
        .map(|s| s.to_string())
        .collect()
}

/// Scan the transcripts root and return newly parsed events since the last
/// call. On the first call (empty offsets) this parses the last ~31 days.
pub fn refresh(cfg: &Config, offsets: &mut Offsets, base: &Path, now: i64) -> Vec<Event> {
    let cutoff = now - 31 * 24 * 3600;
    let mut events = Vec::new();
    for path in jsonl_files(base, cutoff) {
        for line in read_new_lines(offsets, &path) {
            if let Some(ev) = parse_line(cfg, &line) {
                if ev.ts >= cutoff {
                    events.push(ev);
                }
            }
        }
    }
    events
}
