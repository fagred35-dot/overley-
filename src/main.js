import "./styles.css";
import "./phase2.css";
import presetCatalog from "./presets.json";

const PRESETS = presetCatalog.presets;
const presetById = new Map(PRESETS.map((preset) => [preset.id, preset]));
const CATEGORIES = [
  ["all", "Все"],
  ["classic", "Кресты"],
  ["dot", "Точки"],
  ["circle", "Круги"],
  ["tl", "T / L"],
  ["chevron", "Шевроны"],
  ["sniper", "Снайперские"],
  ["dynamic", "Динамические"],
  ["pixel", "Пиксельные"],
  ["minimal", "Минимализм"],
  ["hybrid", "Гибриды"],
  ["animated", "Анимация"],
];
const CATEGORY_LABELS = new Map(CATEGORIES);
const tauri = window.__TAURI__;
const tauriInvoke = tauri?.core?.invoke?.bind(tauri.core);
const defaultPreset = presetById.get("classic");
const DEFAULT_CONFIG = {
  enabled: true,
  presetId: "classic",
  color: defaultPreset.color,
  size: defaultPreset.size,
  thickness: defaultPreset.thickness,
  gap: defaultPreset.gap,
  outlineEnabled: defaultPreset.outline.enabled,
  outlineColor: defaultPreset.outline.color,
  outlineThickness: defaultPreset.outline.thickness,
  shadowEnabled: defaultPreset.shadow.enabled,
  shadowColor: defaultPreset.shadow.color,
  shadowBlur: defaultPreset.shadow.blur,
  opacity: defaultPreset.opacity,
  rotation: defaultPreset.rotation,
  warning: null,
};

let appState = { ...DEFAULT_CONFIG };
let toastTimer = 0;
let saveTimer = 0;
let svgSerial = 0;
let activeCategory = "all";
let searchTerm = "";

function svgMarkupForLayer(layer, useGap, config, maskId) {
  const markup = layer.markup ?? "";
  if (!useGap || !layer.gapMask || Number(config.gap) <= 0) return markup;
  return `<g mask="url(#${maskId})">${markup}</g>`;
}

function svgFor(preset, config = appState, className = "", animate = false) {
  const id = `nova-gap-${++svgSerial}`;
  const hasGapLayer = (preset.layers ?? []).some((layer) => layer.gapMask);
  const defs = hasGapLayer && Number(config.gap) > 0
    ? `<defs><mask id="${id}" maskUnits="userSpaceOnUse" x="0" y="0" width="100" height="100"><rect width="100" height="100" fill="white"/><circle cx="50" cy="50" r="${Number(config.gap)}" fill="black"/></mask></defs>`
    : "";
  const markup = (preset.layers ?? []).map((layer) => svgMarkupForLayer(layer, true, config, id)).join("");
  const animation = animate && preset.animation !== "none" ? `preview-reticle-animation-${preset.animation}` : "";
  return `<svg class="crosshair-svg ${className} ${animation}" viewBox="0 0 100 100" role="img" aria-label="${preset.name}">${defs}${markup}</svg>`;
}

function renderPresetCards() {
  const list = document.querySelector("#preset-list");
  list.innerHTML = PRESETS.map((preset) => `
    <button class="preset-card" type="button" role="listitem" data-preset-id="${preset.id}" data-category="${preset.category}"
      aria-pressed="false" title="${preset.description}">
      <span class="preset-art" aria-hidden="true">${svgFor(preset, DEFAULT_CONFIG, "", false)}</span>
      <strong>${preset.name}</strong>
      <small>${preset.subtitle}</small>
    </button>
  `).join("");
  list.addEventListener("click", (event) => {
    const button = event.target.closest("[data-preset-id]");
    if (button) choosePreset(button.dataset.presetId);
  });
}

function renderCategoryFilters() {
  const container = document.querySelector("#category-filters");
  container.innerHTML = CATEGORIES.map(([id, label]) => {
    const count = id === "all" ? PRESETS.length : PRESETS.filter((preset) => preset.category === id).length;
    return `<button class="category-chip${id === activeCategory ? " active" : ""}" type="button" data-category-filter="${id}" aria-pressed="${id === activeCategory}">${label}<span class="chip-count">${count}</span></button>`;
  }).join("");
  container.addEventListener("click", (event) => {
    const button = event.target.closest("[data-category-filter]");
    if (!button) return;
    activeCategory = button.dataset.categoryFilter;
    container.querySelectorAll(".category-chip").forEach((chip) => {
      const selected = chip.dataset.categoryFilter === activeCategory;
      chip.classList.toggle("active", selected);
      chip.setAttribute("aria-pressed", String(selected));
    });
    filterPresets();
  });
}

function filterPresets() {
  const words = searchTerm.trim().toLocaleLowerCase("ru").split(/\s+/).filter(Boolean);
  let visible = 0;
  document.querySelectorAll("#preset-list [data-preset-id]").forEach((button) => {
    const preset = presetById.get(button.dataset.presetId);
    const categoryMatches = activeCategory === "all" || preset.category === activeCategory;
    const searchContent = `${preset.name} ${preset.subtitle} ${preset.description} ${CATEGORY_LABELS.get(preset.category) ?? ""}`.toLocaleLowerCase("ru");
    const textMatches = words.every((word) => searchContent.includes(word));
    const show = categoryMatches && textMatches;
    button.hidden = !show;
    if (show) visible += 1;
  });
  document.querySelector("#results-count").textContent = `${visible} ${pluralizePreset(visible)}`;
  document.querySelector("#no-results").hidden = visible !== 0;
}

function pluralizePreset(count) {
  const lastTwo = count % 100;
  const last = count % 10;
  if (lastTwo >= 11 && lastTwo <= 14) return "прицелов";
  if (last === 1) return "прицел";
  if (last >= 2 && last <= 4) return "прицела";
  return "прицелов";
}

function syncControlValues(config) {
  const mappings = [
    ["#color-control", "color"],
    ["#size-control", "size"],
    ["#thickness-control", "thickness"],
    ["#gap-control", "gap"],
    ["#opacity-control", "opacity", (value) => Math.round(Number(value) * 100)],
    ["#rotation-control", "rotation"],
    ["#outline-enabled", "outlineEnabled"],
    ["#outline-color", "outlineColor"],
    ["#outline-thickness-control", "outlineThickness"],
    ["#shadow-enabled", "shadowEnabled"],
    ["#shadow-control", "shadowBlur"],
  ];
  for (const [selector, key, convert = (value) => value] of mappings) {
    const element = document.querySelector(selector);
    if (!element || document.activeElement === element) continue;
    if (element.type === "checkbox") element.checked = Boolean(config[key]);
    else element.value = String(convert(config[key]));
  }
  document.querySelector("#outline-color").disabled = !config.outlineEnabled;
  document.querySelector("#outline-thickness-control").disabled = !config.outlineEnabled;
  document.querySelector("#shadow-control").disabled = !config.shadowEnabled;
  document.querySelector("#size-value").textContent = String(Math.round(config.size));
  document.querySelector("#thickness-value").textContent = Number(config.thickness).toFixed(1);
  document.querySelector("#gap-value").textContent = String(Math.round(config.gap));
  document.querySelector("#opacity-value").textContent = `${Math.round(Number(config.opacity) * 100)}%`;
  document.querySelector("#rotation-value").textContent = `${Math.round(config.rotation)}°`;
  document.querySelector("#outline-thickness-value").textContent = Number(config.outlineThickness).toFixed(1);
  document.querySelector("#shadow-value").textContent = String(Math.round(config.shadowBlur));
}

function render(state) {
  if (!state || typeof state !== "object") return;
  if (state.config && typeof state.config === "object") state = { ...state.config, warning: state.warning };
  appState = {
    ...appState,
    ...state,
    presetId: state.presetId ?? state.preset_id ?? appState.presetId,
  };
  const preset = presetById.get(appState.presetId) ?? defaultPreset;
  appState.presetId = preset.id;
  const enabled = Boolean(appState.enabled);
  const warning = appState.warning || null;

  const rootStyle = document.documentElement.style;
  rootStyle.setProperty("--crosshair-color", appState.color);
  rootStyle.setProperty("--crosshair-outline", appState.outlineEnabled ? appState.outlineColor : "transparent");
  rootStyle.setProperty("--crosshair-thickness", String(appState.thickness));
  rootStyle.setProperty("--crosshair-outline-thickness", String(appState.outlineThickness));
  rootStyle.setProperty("--crosshair-filter", appState.shadowEnabled && appState.shadowBlur > 0
    ? `drop-shadow(0 0 ${appState.shadowBlur}px ${appState.shadowColor})`
    : "none");

  document.querySelectorAll("#preset-list [data-preset-id]").forEach((button) => {
    const selected = button.dataset.presetId === preset.id;
    button.classList.toggle("selected", selected);
    button.setAttribute("aria-pressed", String(selected));
  });
  const previewReticle = document.querySelector("#preview-reticle");
  previewReticle.innerHTML = svgFor(preset, appState, "", true);
  previewReticle.style.width = `${appState.size}px`;
  previewReticle.style.height = `${appState.size}px`;
  previewReticle.style.opacity = String(appState.opacity);
  previewReticle.style.transform = `rotate(${appState.rotation}deg)`;
  document.querySelector("#selected-name").textContent = preset.name;
  document.querySelector("#selected-category").textContent = (CATEGORY_LABELS.get(preset.category) ?? preset.category).toLocaleUpperCase("ru");
  document.querySelector("#selected-description").textContent = preset.description;
  document.querySelector("#hero-status-text").textContent = enabled ? "Оверлей включён" : "Оверлей скрыт";
  document.querySelector("#hero-status-dot").classList.toggle("on", enabled);

  const toggle = document.querySelector("#overlay-toggle");
  toggle.classList.toggle("is-on", enabled);
  toggle.classList.toggle("is-off", !enabled);
  toggle.setAttribute("aria-pressed", String(enabled));
  toggle.setAttribute("aria-label", enabled ? "Скрыть оверлей" : "Показать оверлей");
  toggle.querySelector(".toggle-label").textContent = enabled ? "ВКЛ" : "ВЫКЛ";
  toggle.disabled = Boolean(warning);
  toggle.title = warning || "Показать или скрыть прозрачный оверлей";

  const liveBadge = document.querySelector(".live-badge");
  liveBadge.classList.toggle("is-off", !enabled);
  document.querySelector("#status-label").textContent = warning ? "ТРЕБУЕТСЯ НАСТРОЙКА" : enabled ? "АКТИВЕН" : "СКРЫТ";
  document.querySelector("#backend-note").textContent = warning || "Оверлей готов к работе";
  document.querySelector("#preset-count").textContent = String(PRESETS.length);
  syncControlValues(appState);
  filterPresets();
  persistLocalConfig();
}

function persistLocalConfig() {
  try {
    const { warning: _warning, ...config } = appState;
    localStorage.setItem("nova-phase2-state", JSON.stringify(config));
  } catch {
    // The native Rust config store is authoritative in the desktop application.
  }
}

function configForCommand() {
  const { warning: _warning, ...config } = appState;
  return config;
}

function showToast(message, isError = false) {
  const toast = document.querySelector("#toast");
  toast.textContent = message;
  toast.classList.toggle("error", isError);
  toast.classList.add("visible");
  window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => toast.classList.remove("visible"), 2600);
}

function setSettingsStatus(text, error = false) {
  const label = document.querySelector("#settings-status");
  label.textContent = text;
  label.closest(".settings-saved").classList.toggle("has-error", error);
}

async function invoke(command, args = {}) {
  if (tauriInvoke) return tauriInvoke(command, args);
  if (command === "get_overlay_state") {
    try {
      const stored = JSON.parse(localStorage.getItem("nova-phase2-state"));
      return stored ? { ...DEFAULT_CONFIG, ...stored } : appState;
    } catch {
      return appState;
    }
  }
  if (command === "set_overlay_enabled") {
    const next = { ...appState, enabled: Boolean(args.enabled) };
    render(next);
    return next;
  }
  if (command === "set_preset") {
    const preset = presetById.get(args.presetId);
    if (!preset) throw new Error("Неизвестный пресет");
    const next = { ...appState, presetId: preset.id };
    render(next);
    return next;
  }
  if (command === "update_overlay_settings") {
    const next = { ...appState, ...(args.config ?? {}) };
    render(next);
    return next;
  }
  if (command === "hide_main_window" || command === "minimize_main_window") {
    showToast("Эта команда работает в настольном приложении.");
    return null;
  }
  throw new Error(`Unknown command: ${command}`);
}

async function choosePreset(presetId) {
  if (!presetById.has(presetId)) return;
  appState = { ...appState, presetId };
  render(appState);
  setSettingsStatus("Обновляем оверлей…");
  try {
    const state = await invoke("set_preset", { presetId });
    if (state) render(state);
    setSettingsStatus("Изменения сохранены");
  } catch (error) {
    setSettingsStatus("Не удалось применить", true);
    showToast(String(error), true);
  }
}

async function toggleOverlay() {
  if (appState.warning) return;
  try {
    const state = await invoke("set_overlay_enabled", { enabled: !appState.enabled });
    if (state) render(state);
  } catch (error) {
    showToast(String(error), true);
  }
}

function scheduleSettingsSave() {
  setSettingsStatus("Сохраняем настройки…");
  window.clearTimeout(saveTimer);
  saveTimer = window.setTimeout(async () => {
    try {
      const state = await invoke("update_overlay_settings", { config: configForCommand() });
      if (state) render(state);
      setSettingsStatus("Настройки сохранены");
    } catch (error) {
      setSettingsStatus("Не удалось сохранить", true);
      showToast(String(error), true);
    }
  }, 90);
}

function readControls() {
  return {
    color: document.querySelector("#color-control").value,
    size: Number(document.querySelector("#size-control").value),
    thickness: Number(document.querySelector("#thickness-control").value),
    gap: Number(document.querySelector("#gap-control").value),
    opacity: Number(document.querySelector("#opacity-control").value) / 100,
    rotation: Number(document.querySelector("#rotation-control").value),
    outlineEnabled: document.querySelector("#outline-enabled").checked,
    outlineColor: document.querySelector("#outline-color").value,
    outlineThickness: Number(document.querySelector("#outline-thickness-control").value),
    shadowEnabled: document.querySelector("#shadow-enabled").checked,
    shadowColor: appState.shadowColor ?? "rgba(83, 246, 207, 0.22)",
    shadowBlur: Number(document.querySelector("#shadow-control").value),
  };
}

function bindSettingsControls() {
  const inputIds = [
    "color-control", "size-control", "thickness-control", "gap-control", "opacity-control",
    "rotation-control", "outline-enabled", "outline-color", "outline-thickness-control", "shadow-enabled", "shadow-control",
  ];
  for (const id of inputIds) {
    const control = document.getElementById(id);
    control.addEventListener("input", () => {
      appState = { ...appState, ...readControls() };
      render(appState);
      scheduleSettingsSave();
    });
    if (control.type === "checkbox" || control.type === "color") {
      control.addEventListener("change", () => {
        appState = { ...appState, ...readControls() };
        render(appState);
        scheduleSettingsSave();
      });
    }
  }
  document.querySelector("#reset-settings").addEventListener("click", () => {
    const preset = presetById.get(appState.presetId) ?? defaultPreset;
    appState = {
      ...appState,
      color: preset.color,
      size: preset.size,
      thickness: preset.thickness,
      gap: preset.gap,
      outlineEnabled: preset.outline.enabled,
      outlineColor: preset.outline.color,
      outlineThickness: preset.outline.thickness,
      shadowEnabled: preset.shadow.enabled,
      shadowColor: preset.shadow.color,
      shadowBlur: preset.shadow.blur,
      opacity: preset.opacity,
      rotation: preset.rotation,
    };
    render(appState);
    scheduleSettingsSave();
  });
}

renderPresetCards();
renderCategoryFilters();
bindSettingsControls();
document.querySelector("#preset-search").addEventListener("input", (event) => {
  searchTerm = event.currentTarget.value;
  filterPresets();
});
document.addEventListener("keydown", (event) => {
  if (event.key === "/" && !["INPUT", "TEXTAREA"].includes(document.activeElement?.tagName)) {
    event.preventDefault();
    document.querySelector("#preset-search").focus();
  }
  if (event.key === "Escape" && document.activeElement?.id === "preset-search") {
    document.querySelector("#preset-search").value = "";
    searchTerm = "";
    filterPresets();
  }
});
document.querySelector("#overlay-toggle").addEventListener("click", toggleOverlay);
document.querySelector("#hide-window").addEventListener("click", () => {
  invoke("hide_main_window").catch((error) => showToast(String(error), true));
});
document.querySelector("#minimize-window").addEventListener("click", () => {
  invoke("minimize_main_window").catch((error) => showToast(String(error), true));
});

if (tauri?.event?.listen) tauri.event.listen("overlay-state", (event) => render(event.payload));
invoke("get_overlay_state")
  .then(render)
  .catch((error) => showToast(`Не удалось получить состояние: ${String(error)}`, true));
