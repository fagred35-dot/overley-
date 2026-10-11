use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

const PRESETS_JSON: &str = include_str!("../../src/presets.json");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct OverlayConfig {
    pub(crate) enabled: bool,
    pub(crate) preset_id: String,
    pub(crate) color: String,
    pub(crate) size: f64,
    pub(crate) thickness: f64,
    pub(crate) gap: f64,
    pub(crate) outline_enabled: bool,
    pub(crate) outline_color: String,
    pub(crate) outline_thickness: f64,
    pub(crate) shadow_enabled: bool,
    pub(crate) shadow_color: String,
    pub(crate) shadow_blur: f64,
    pub(crate) opacity: f64,
    pub(crate) rotation: f64,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            preset_id: "classic".to_owned(),
            color: "#69f4cf".to_owned(),
            size: 72.0,
            thickness: 3.3,
            gap: 8.0,
            outline_enabled: true,
            outline_color: "#071019".to_owned(),
            outline_thickness: 7.0,
            shadow_enabled: true,
            shadow_color: "#69f4cf".to_owned(),
            shadow_blur: 4.0,
            opacity: 1.0,
            rotation: 0.0,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum NativePrimitive {
    Segments { segments: Vec<[f64; 4]> },
    Circle { cx: f64, cy: f64, radius: f64 },
    Dot { cx: f64, cy: f64, radius: f64 },
    Polyline {
        points: Vec<[f64; 2]>,
        #[serde(default)]
        closed: bool,
    },
    Arc {
        cx: f64,
        cy: f64,
        radius: f64,
        #[serde(rename = "startDeg")]
        start_deg: f64,
        #[serde(rename = "endDeg")]
        end_deg: f64,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PresetLayer {
    #[serde(default)]
    pub(crate) gap_mask: bool,
    pub(crate) native: Vec<NativePrimitive>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PresetDefinition {
    pub(crate) id: String,
    pub(crate) layers: Vec<PresetLayer>,
}

#[derive(Deserialize)]
struct PresetCatalog {
    presets: Vec<PresetDefinition>,
}

pub(crate) fn presets() -> &'static [PresetDefinition] {
    static PRESETS: OnceLock<Vec<PresetDefinition>> = OnceLock::new();
    PRESETS
        .get_or_init(|| {
            serde_json::from_str::<PresetCatalog>(PRESETS_JSON)
                .expect("src/presets.json must contain valid preset definitions")
                .presets
        })
        .as_slice()
}

pub(crate) fn find_preset(id: &str) -> Option<&'static PresetDefinition> {
    presets().iter().find(|preset| preset.id == id)
}

pub(crate) fn is_hex_color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
}

pub(crate) fn validate_config(mut config: OverlayConfig) -> Result<OverlayConfig, String> {
    if find_preset(&config.preset_id).is_none() {
        return Err(format!("Неизвестный пресет: {}", config.preset_id));
    }
    if !is_hex_color(&config.color) || !is_hex_color(&config.outline_color) {
        return Err("Цвета должны быть указаны в формате #RRGGBB".to_owned());
    }
    if !is_hex_color(&config.shadow_color) {
        config.shadow_color = config.color.clone();
    }
    for value in [
        config.size,
        config.thickness,
        config.gap,
        config.outline_thickness,
        config.shadow_blur,
        config.opacity,
        config.rotation,
    ] {
        if !value.is_finite() {
            return Err("Числовые настройки должны быть конечными значениями".to_owned());
        }
    }
    config.size = config.size.clamp(28.0, 116.0);
    config.thickness = config.thickness.clamp(0.8, 8.0);
    config.gap = config.gap.clamp(0.0, 24.0);
    config.outline_thickness = config.outline_thickness.clamp(0.0, 14.0);
    config.shadow_blur = config.shadow_blur.clamp(0.0, 12.0);
    config.opacity = config.opacity.clamp(0.2, 1.0);
    config.rotation = config.rotation.clamp(-180.0, 180.0);
    Ok(config)
}
