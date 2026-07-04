import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface GaugeSnapshot {
  five_h_cost: number;
  five_h_ceiling: number;
  weekly_cost: number;
  weekly_ceiling: number;
  burn_per_hour: number;
  today_cost: number;
  month_cost: number;
  plan: string | null;
  plan_price: number;
  plan_multiple: number;
  calibrated: boolean;
}

const CIRCUMFERENCE = 2 * Math.PI * 50;

function money(n: number): string {
  if (n >= 1000) return "$" + Math.round(n).toLocaleString();
  return "$" + n.toFixed(2);
}

function setGauge(arcId: string, pctId: string, used: number, ceiling: number) {
  const remaining = Math.max(0, Math.min(1, 1 - used / Math.max(ceiling, 0.01)));
  const arc = document.getElementById(arcId) as unknown as SVGCircleElement;
  const pct = document.getElementById(pctId)!;
  arc.style.strokeDashoffset = String(CIRCUMFERENCE * (1 - remaining));
  arc.classList.toggle("low", remaining < 0.25);
  arc.classList.toggle("warn", remaining >= 0.25 && remaining < 0.5);
  pct.textContent = Math.round(remaining * 100) + "%";
}

function render(s: GaugeSnapshot) {
  setGauge("arc-5h", "pct-5h", s.five_h_cost, s.five_h_ceiling);
  setGauge("arc-wk", "pct-wk", s.weekly_cost, s.weekly_ceiling);

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
}

async function init() {
  // Outside Tauri (plain browser dev), render sample data so the popover
  // can be designed and reviewed without the Rust side.
  if (!("__TAURI_INTERNALS__" in window)) {
    render({
      five_h_cost: 9.7,
      five_h_ceiling: 300,
      weekly_cost: 932,
      weekly_ceiling: 2000,
      burn_per_hour: 9.7,
      today_cost: 41.2,
      month_cost: 5991,
      plan: "max_20x",
      plan_price: 200,
      plan_multiple: 29.9,
      calibrated: false,
    });
    return;
  }
  try {
    const snapshot = await invoke<GaugeSnapshot>("get_state");
    render(snapshot);
  } catch (e) {
    console.error("get_state failed", e);
  }
  await listen<GaugeSnapshot>("gauge://state", (event) => render(event.payload));
}

init();
