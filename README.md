# Claude Gauge

**A gas gauge for Claude Code, in your system tray.** Live fuel levels for your 5-hour window and weekly quota, your burn rate in API-equivalent dollars per hour, and your plan multiple — measured from your local transcripts, updated every 15 seconds.

Everything runs locally. It reads `~/.claude/projects` and nothing else. No network calls, no telemetry, no account.

## What the gauge shows

- **Tray icon** — a fuel tank that drains as you consume your 5-hour window or weekly quota (whichever is tighter). White = plenty, amber = under half, vermillion = under a quarter.
- **Click the icon** for the popover: both gauges, current burn rate ($/hour), today's total, last 30 days, and your plan multiple ("29.9× the sticker").

## How it knows the ceiling

Anthropic doesn't publish subscription quotas as numbers, so the gauge starts with clearly-labelled estimates per plan. Then it **self-calibrates**: Claude Code records a local notice every time you actually hit a limit, and the gauge learns your real ceiling from the consumption that preceded each one. The longer you run it, the more accurate it gets. Estimated ceilings can also be pinned manually in config.

## Config

Reads the same `~/.claude/claude_usage.config.json` as the [usage dashboard](https://agentshortlist.com/claude-usage):

```json
{
  "plan": "max_20x",
  "gauge_ceilings": { "five_h": 300.0, "weekly": 2000.0 }
}
```

`plan` is one of `pro`, `max_5x`, `max_20x`, `api`. `gauge_ceilings` (optional) pins the estimated ceilings in API-equivalent dollars.

## Development

Prereqs: Node 18+, Rust (MSVC toolchain on Windows).

```bash
npm install
npx tauri dev      # run with hot reload
npx tauri build    # produce the installer
```

## Status

v0.1 — Windows first. macOS build is the same codebase; packaging and notarization to follow.

## License

MIT.

---

Built by [Agent Shortlist](https://agentshortlist.com/claude-usage). Not affiliated with Anthropic. "Claude" and "Claude Code" are trademarks of Anthropic.
