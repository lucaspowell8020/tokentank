import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface GaugeSnapshot {
  five_h_cost: number;
  five_h_ceiling: number;
  five_h_reset: number | null;
  session_active: boolean;
  weekly_cost: number;
  weekly_ceiling: number;
  weekly_reset: number | null;
  burn_per_hour: number;
  today_cost: number;
  month_cost: number;
  plan: string | null;
  plan_price: number;
  plan_multiple: number;
  calibrated: boolean;
}

let snap: GaugeSnapshot | null = null;

function money(n: number): string {
  if (n >= 1000) return "$" + Math.round(n).toLocaleString();
  return "$" + n.toFixed(2);
}

function remainingFrac(used: number, ceiling: number): number {
  if (!isFinite(ceiling) || ceiling <= 0) return 1;
  return Math.max(0, Math.min(1, 1 - used / ceiling));
}

function fmtCountdown(secs: number): string {
  secs = Math.max(0, Math.floor(secs));
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  if (d > 0) return `${d}d ${h}h ${m}m`;
  return `${h}h ${String(m).padStart(2, "0")}m ${String(s).padStart(2, "0")}s`;
}

/** Static parts: needle, bars, stats. Called on each snapshot. */
function render(s: GaugeSnapshot) {
  snap = s;

  // 5-hour dial needle: full = pointing at F (right), empty = E (left).
  const rem5 = remainingFrac(s.five_h_cost, s.five_h_ceiling);
  const needle = document.getElementById("needle")!;
  needle.setAttribute("transform", `rotate(${-(1 - rem5) * 180} 120 120)`);
  document.getElementById("pct-5h")!.textContent = Math.round(rem5 * 100) + "%";

  // Weekly tank bar
  const remWk = remainingFrac(s.weekly_cost, s.weekly_ceiling);
  const fill = document.getElementById("tank-fill")!;
  fill.style.width = (remWk * 100).toFixed(1) + "%";
  fill.classList.toggle("low", remWk < 0.25);
  fill.classList.toggle("warn", remWk >= 0.25 && remWk < 0.5);
  document.getElementById("pct-wk")!.textContent = Math.round(remWk * 100) + "% left";

  document.getElementById("burn")!.textContent = money(s.burn_per_hour) + "/h";
  document.getElementById("today")!.textContent = money(s.today_cost);
  document.getElementById("month")!.textContent = money(s.month_cost);
  document.getElementById("mult")!.textContent =
    s.plan && s.plan !== "api" && s.plan_price > 0
      ? s.plan_multiple.toFixed(1) + "× sticker"
      : "—";

  document.getElementById("plan-line")!.textContent = s.plan
    ? "plan: " + s.plan
    : "plan: not set";
  document.getElementById("calibration-note")!.textContent = s.calibrated
    ? "Ceilings calibrated from your own observed limit events."
    : "Ceilings are estimates until your first observed limit.";

  tick();
}

/** Live countdowns, every second. */
function tick() {
  if (!snap) return;
  const now = Date.now() / 1000;

  const cd5 = document.getElementById("cd-5h")!;
  if (snap.session_active && snap.five_h_reset && snap.five_h_reset > now) {
    const left = snap.five_h_reset - now;
    const cls = left < 30 * 60 ? "hot" : "";
    cd5.innerHTML = `tank refills in <span class="${cls}">${fmtCountdown(left)}</span>`;
  } else if (snap.session_active) {
    cd5.textContent = "session window just reset — tank refilling";
  } else {
    cd5.textContent = "full tank — session starts with your next message";
  }

  const cdWk = document.getElementById("cd-wk")!;
  if (snap.weekly_reset && snap.weekly_reset > now) {
    cdWk.textContent = "refills in " + fmtCountdown(snap.weekly_reset - now);
  } else {
    cdWk.textContent = "rolling 7-day estimate — set weekly_reset in config for the real window";
  }
}

/* ── Setup wizard ─────────────────────────────────────── */

let wizPlan = "max_5x";

function showWizard() {
  document.getElementById("wizard")!.hidden = false;
}

function hideWizard() {
  document.getElementById("wizard")!.hidden = true;
}

function wireWizard() {
  const btns = document.querySelectorAll<HTMLButtonElement>("#plan-btns button");
  btns.forEach((b) =>
    b.addEventListener("click", () => {
      btns.forEach((x) => x.classList.remove("on"));
      b.classList.add("on");
      wizPlan = b.dataset.plan!;
      const isApi = wizPlan === "api";
      (document.getElementById("wiz-pcts") as HTMLElement).hidden = isApi;
      (document.getElementById("wiz-api-note") as HTMLElement).hidden = !isApi;
    })
  );

  document.getElementById("recal")!.addEventListener("click", showWizard);

  document.getElementById("wiz-save")!.addEventListener("click", async () => {
    const num = (id: string): number | null => {
      const v = (document.getElementById(id) as HTMLInputElement).value.trim();
      const n = parseFloat(v);
      return v && isFinite(n) && n >= 1 && n <= 99 ? n : null;
    };
    const day = (document.getElementById("wiz-day") as HTMLSelectElement).value;
    const time = (document.getElementById("wiz-time") as HTMLInputElement).value || "06:00";

    if (!("__TAURI_INTERNALS__" in window)) {
      hideWizard();
      return;
    }
    try {
      const snapshot = await invoke<GaugeSnapshot>("save_setup", {
        plan: wizPlan,
        weeklyReset: `${day} ${time}`,
        sessionPct: num("wiz-session-pct"),
        weekPct: num("wiz-week-pct"),
      });
      render(snapshot);
      hideWizard();
    } catch (e) {
      console.error("save_setup failed", e);
    }
  });
}

async function init() {
  setInterval(tick, 1000);
  wireWizard();

  // Outside Tauri (plain browser dev), render sample data so the popover
  // can be designed and reviewed without the Rust side.
  if (!("__TAURI_INTERNALS__" in window)) {
    if (location.hash === "#wizard") showWizard();
    const now = Date.now() / 1000;
    render({
      five_h_cost: 45,
      five_h_ceiling: 265,
      five_h_reset: now + 4 * 3600 + 27 * 60,
      session_active: true,
      weekly_cost: 420,
      weekly_ceiling: 2200,
      weekly_reset: now + 4 * 86400 + 17 * 3600,
      burn_per_hour: 44.96,
      today_cost: 44.96,
      month_cost: 6059,
      plan: "max_5x",
      plan_price: 100,
      plan_multiple: 30.3,
      calibrated: false,
    });
    return;
  }

  try {
    if (await invoke<boolean>("needs_setup")) showWizard();
    const snapshot = await invoke<GaugeSnapshot>("get_state");
    render(snapshot);

    const autostart = document.getElementById("autostart-toggle") as HTMLInputElement;
    autostart.checked = await invoke<boolean>("get_autostart");
    autostart.addEventListener("change", async () => {
      autostart.checked = await invoke<boolean>("set_autostart", {
        enabled: autostart.checked,
      });
    });
  } catch (e) {
    console.error("startup failed", e);
  }
  await listen<GaugeSnapshot>("gauge://state", (event) => render(event.payload));
}

init();
