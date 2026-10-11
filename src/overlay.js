import "./overlay.css";
import presetCatalog from "./presets.json";

const presets = new Map(presetCatalog.presets.map((preset) => [preset.id, preset]));
const root = document.querySelector("#overlay-root");
const tauri = window.__TAURI__;

function render(state = {}) {
  const presetId = state.presetId ?? state.preset_id ?? "classic";
  const preset = presets.get(presetId) ?? presets.get("classic");
  root.replaceChildren();
  if (!state.enabled || !preset) return;

  const color = state.color ?? preset.color ?? "#69f4cf";
  const outlineEnabled = state.outlineEnabled ?? preset.outline.enabled;
  const outlineColor = outlineEnabled ? (state.outlineColor ?? preset.outline.color) : "transparent";
  const thickness = state.thickness ?? preset.thickness;
  const outlineThickness = state.outlineThickness ?? preset.outline.thickness;
  const shadowEnabled = state.shadowEnabled ?? preset.shadow.enabled;
  const shadowBlur = state.shadowBlur ?? preset.shadow.blur;
  root.style.setProperty("--crosshair-color", color);
  root.style.setProperty("--crosshair-outline", outlineColor);
  root.style.setProperty("--crosshair-thickness", String(thickness));
  root.style.setProperty("--crosshair-outline-thickness", String(outlineThickness));
  root.style.setProperty("--crosshair-filter", shadowEnabled && shadowBlur > 0
    ? `drop-shadow(0 0 ${shadowBlur}px ${state.shadowColor ?? preset.shadow.color})`
    : "none");

  const holder = document.createElement("div");
  holder.className = "reticle";
  const size = state.size ?? preset.size ?? 72;
  const opacity = state.opacity ?? preset.opacity ?? 1;
  const rotation = state.rotation ?? preset.rotation ?? 0;
  holder.style.width = `${size}px`;
  holder.style.height = `${size}px`;
  holder.style.opacity = String(opacity);
  holder.style.transform = `rotate(${rotation}deg)`;
  const gapId = "nova-overlay-gap-mask";
  const gap = Number(state.gap ?? preset.gap ?? 8);
  const hasGap = (preset.layers ?? []).some((layer) => layer.gapMask) && gap > 0;
  const defs = hasGap
    ? `<defs><mask id="${gapId}" maskUnits="userSpaceOnUse" x="0" y="0" width="100" height="100"><rect width="100" height="100" fill="white"/><circle cx="50" cy="50" r="${gap}" fill="black"/></mask></defs>`
    : "";
  const markup = (preset.layers ?? []).map((layer) => {
    const body = layer.markup ?? "";
    return layer.gapMask && hasGap ? `<g mask="url(#${gapId})">${body}</g>` : body;
  }).join("");
  const animation = preset.animation && preset.animation !== "none" ? `reticle-animation-${preset.animation}` : "";
  holder.innerHTML = `<svg class="crosshair-svg ${animation}" viewBox="0 0 100 100" focusable="false">${defs}${markup}</svg>`;
  root.append(holder);
}

if (tauri?.event?.listen) {
  tauri.event.listen("overlay-state", ({ payload }) => render(payload));
}

if (tauri?.core?.invoke) {
  tauri.core.invoke("get_overlay_state")
    .then(render)
    .catch(() => render({ enabled: true }));
} else {
  render({ enabled: true });
}
