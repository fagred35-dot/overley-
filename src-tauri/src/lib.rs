mod config;
use config::{find_preset, validate_config, OverlayConfig};
use serde::Serialize;
use std::{path::PathBuf, sync::Mutex};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};

#[cfg(target_os = "linux")]
mod linux_overlay;

struct AppState {
    config: Mutex<OverlayConfig>,
    warning: Mutex<Option<String>>,
}

impl AppState {
    fn new() -> Self {
        Self {
            config: Mutex::new(OverlayConfig::default()),
            warning: Mutex::new(None),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OverlayStatePayload {
    #[serde(flatten)]
    config: OverlayConfig,
    warning: Option<String>,
}

fn payload_from(state: &AppState) -> OverlayStatePayload {
    let config = state
        .config
        .lock()
        .map(|value| value.clone())
        .unwrap_or_default();
    let warning = state.warning.lock().ok().and_then(|value| value.clone());
    OverlayStatePayload { config, warning }
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|directory| directory.join("settings.json"))
        .map_err(|error| format!("Не удалось определить каталог настроек: {error}"))
}

fn load_config(app: &AppHandle) -> Option<OverlayConfig> {
    let content = std::fs::read(settings_path(app).ok()?).ok()?;
    let config = serde_json::from_slice::<OverlayConfig>(&content).ok()?;
    validate_config(config).ok()
}

fn persist_config(app: &AppHandle, config: &OverlayConfig) -> Result<(), String> {
    let path = settings_path(app)?;
    let directory = path
        .parent()
        .ok_or_else(|| "Каталог настроек недоступен".to_owned())?;
    std::fs::create_dir_all(directory)
        .map_err(|error| format!("Не удалось создать каталог настроек: {error}"))?;
    let bytes = serde_json::to_vec_pretty(config)
        .map_err(|error| format!("Не удалось сериализовать настройки: {error}"))?;
    std::fs::write(path, bytes).map_err(|error| format!("Не удалось сохранить настройки: {error}"))
}

fn commit_state(app: &AppHandle, state: &AppState) -> Result<OverlayStatePayload, String> {
    let payload = payload_from(state);
    let preset = find_preset(&payload.config.preset_id)
        .ok_or_else(|| format!("Неизвестный пресет: {}", payload.config.preset_id))?;

    #[cfg(target_os = "linux")]
    linux_overlay::set_state(
        app.clone(),
        payload.config.clone(),
        preset.layers.clone(),
    )
    .map_err(|error| error.to_string())?;

    #[cfg(not(target_os = "linux"))]
    {
        let overlay = app
            .get_webview_window("overlay")
            .ok_or_else(|| "Окно оверлея ещё не создано".to_owned())?;
        if payload.config.enabled {
            overlay.show().map_err(|error| error.to_string())?;
        } else {
            overlay.hide().map_err(|error| error.to_string())?;
        }
    }

    app.emit("overlay-state", payload.clone())
        .map_err(|error| error.to_string())?;
    persist_config(app, &payload.config)?;
    Ok(payload)
}

#[tauri::command]
fn get_overlay_state(state: State<'_, AppState>) -> OverlayStatePayload {
    payload_from(&state)
}

#[tauri::command]
fn set_overlay_enabled(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<OverlayStatePayload, String> {
    if enabled {
        if let Some(warning) = state.warning.lock().ok().and_then(|value| value.clone()) {
            return Err(warning);
        }
    }
    state
        .config
        .lock()
        .map_err(|_| "Не удалось обновить состояние оверлея".to_owned())?
        .enabled = enabled;
    commit_state(&app, &state)
}

#[tauri::command]
fn set_preset(
    app: AppHandle,
    state: State<'_, AppState>,
    preset_id: String,
) -> Result<OverlayStatePayload, String> {
    if find_preset(&preset_id).is_none() {
        return Err(format!("Неизвестный пресет: {preset_id}"));
    }
    state
        .config
        .lock()
        .map_err(|_| "Не удалось изменить выбранный прицел".to_owned())?
        .preset_id = preset_id;
    commit_state(&app, &state)
}

#[tauri::command]
fn update_overlay_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    config: OverlayConfig,
) -> Result<OverlayStatePayload, String> {
    let config = validate_config(config)?;
    *state
        .config
        .lock()
        .map_err(|_| "Не удалось обновить настройки прицела".to_owned())? = config;
    commit_state(&app, &state)
}

#[tauri::command]
fn hide_main_window(app: AppHandle) -> Result<(), String> {
    app.get_webview_window("main")
        .ok_or_else(|| "Окно настроек не найдено".to_owned())?
        .hide()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn minimize_main_window(app: AppHandle) -> Result<(), String> {
    app.get_webview_window("main")
        .ok_or_else(|| "Окно настроек не найдено".to_owned())?
        .minimize()
        .map_err(|error| error.to_string())
}

fn set_overlay_enabled_inner(
    app: &AppHandle,
    state: &AppState,
    enabled: bool,
) -> Result<OverlayStatePayload, String> {
    if enabled {
        if let Some(warning) = state.warning.lock().ok().and_then(|value| value.clone()) {
            return Err(warning);
        }
    }
    state
        .config
        .lock()
        .map_err(|_| "Не удалось обновить состояние оверлея".to_owned())?
        .enabled = enabled;
    commit_state(app, state)
}

fn toggle_from_tray(app: &AppHandle) {
    let state = app.state::<AppState>();
    let current = state
        .config
        .lock()
        .map(|config| config.enabled)
        .unwrap_or(false);
    if let Err(error) = set_overlay_enabled_inner(app, &state, !current) {
        eprintln!("Could not toggle overlay from tray: {error}");
    }
}

fn create_tray(app: &tauri::App) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(
        app,
        "toggle-overlay",
        "Показать / скрыть прицел",
        true,
        None::<&str>,
    )?;
    let settings = MenuItem::with_id(app, "settings", "Настройки", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = PredefinedMenuItem::quit(app, Some("Выход"))?;
    let menu = Menu::with_items(app, &[&toggle, &settings, &separator, &quit])?;
    let icon = Image::new_owned(make_tray_pixels(), 32, 32);

    let builder = TrayIconBuilder::new()
        .tooltip("NOVA Crosshair")
        .icon(icon)
        .menu(&menu);
    // Tauri's left-click tray menu option is unsupported by the Linux tray backend.
    #[cfg(not(target_os = "linux"))]
    let builder = builder.show_menu_on_left_click(true);

    builder
        .on_menu_event(|app, event| match event.id.as_ref() {
            "toggle-overlay" => toggle_from_tray(app),
            "settings" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            _ => {}
        })
        .build(app)?;
    Ok(())
}

fn make_tray_pixels() -> Vec<u8> {
    const SIDE: usize = 32;
    let mut pixels = vec![0_u8; SIDE * SIDE * 4];
    for y in 0..SIDE {
        for x in 0..SIDE {
            let horizontal = (14..=17).contains(&y) && (5..=26).contains(&x);
            let vertical = (14..=17).contains(&x) && (5..=26).contains(&y);
            let center = (13..=18).contains(&x) && (13..=18).contains(&y);
            if horizontal || vertical {
                let offset = (y * SIDE + x) * 4;
                let rgba = if center { [225, 255, 246, 255] } else { [105, 244, 207, 255] };
                pixels[offset..offset + 4].copy_from_slice(&rgba);
            }
        }
    }
    pixels
}

#[cfg(not(target_os = "linux"))]
fn create_desktop_overlay(app: &AppHandle) -> tauri::Result<()> {
    let geometry = app
        .primary_monitor()
        .ok()
        .flatten()
        .map(|monitor| {
            let scale = monitor.scale_factor();
            let position = monitor.position();
            let size = monitor.size();
            (
                position.x as f64 / scale,
                position.y as f64 / scale,
                size.width as f64 / scale,
                size.height as f64 / scale,
            )
        })
        .unwrap_or((0.0, 0.0, 1920.0, 1080.0));

    let overlay = WebviewWindowBuilder::new(app, "overlay", WebviewUrl::App("overlay.html".into()))
        .title("NOVA overlay surface")
        .inner_size(geometry.2, geometry.3)
        .position(geometry.0, geometry.1)
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .focusable(false)
        .focused(false)
        .visible(false)
        .build()?;

    overlay.set_ignore_cursor_events(true)?;
    overlay.set_always_on_top(true)?;
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .manage(AppState::new())
        .setup(|app| {
            let handle = app.handle().clone();
            let state = app.state::<AppState>();
            if let Some(config) = load_config(&handle) {
                if let Ok(mut current) = state.config.lock() {
                    *current = config;
                }
            }
            let initial = payload_from(&state);

            #[cfg(not(target_os = "linux"))]
            create_desktop_overlay(&handle)?;

            #[cfg(target_os = "linux")]
            {
                let preset = find_preset(&initial.config.preset_id).expect("validated preset exists");
                if let Err(warning) = linux_overlay::create(initial.config.clone(), preset.layers.clone()) {
                    if let Ok(mut current) = state.config.lock() {
                        current.enabled = false;
                    }
                    if let Ok(mut value) = state.warning.lock() {
                        *value = Some(warning);
                    }
                }
            }

            let _ = commit_state(&handle, &state);
            create_tray(app)?;

            if let Some(main_window) = app.get_webview_window("main") {
                let close_target = main_window.clone();
                main_window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = close_target.hide();
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_overlay_state,
            set_overlay_enabled,
            set_preset,
            update_overlay_settings,
            hide_main_window,
            minimize_main_window
        ])
        .run(tauri::generate_context!())
        .expect("NOVA Crosshair failed to start");
}
