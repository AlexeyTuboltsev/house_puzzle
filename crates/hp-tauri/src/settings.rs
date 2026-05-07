// settings.rs — persistent user settings backed by tauri-plugin-store.
//
// One JSON file at <app_data>/settings.json, single top-level key
// "settings", whose value is a JSON object matching the `Settings`
// struct below. Defaults applied per-field via `serde(default = ...)`
// so adding a new field is forward-compatible: old stored JSON simply
// fills in the default for any missing key.
//
// `save_settings` accepts a partial JSON object and merges it into the
// stored value. The frontend doesn't have to send the whole thing on
// every change.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

const STORE_FILE: &str = "settings.json";
const SETTINGS_KEY: &str = "settings";
const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_version")]
    pub version: u32,

    /// Absolute path of the last AI file the user opened. `None` until
    /// they pick something through the file picker.
    #[serde(default)]
    pub last_import_path: Option<String>,

    // ---- toggles (Elm Model.show*)
    #[serde(default = "default_true")]
    pub show_outlines: bool,
    #[serde(default)]
    pub show_grid: bool,
    #[serde(default)]
    pub show_numbers: bool,
    #[serde(default)]
    pub show_lights: bool,
    #[serde(default)]
    pub show_group_overlay: bool,
    #[serde(default)]
    pub show_wave_overlay: bool,
    #[serde(default)]
    pub show_only_blueprint: bool,

    // ---- colors (HSL hue, 0..360)
    #[serde(default)]
    pub grid_hue: f64,
    #[serde(default)]
    pub outline_hue: f64,

    // ---- right sidebar width (--tools-width CSS var, in vw)
    #[serde(default = "default_tools_width")]
    pub tools_width_vw: f64,
}

fn default_version() -> u32 {
    SCHEMA_VERSION
}
fn default_true() -> bool {
    true
}
fn default_tools_width() -> f64 {
    40.0
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            version: SCHEMA_VERSION,
            last_import_path: None,
            show_outlines: true,
            show_grid: false,
            show_numbers: false,
            show_lights: false,
            show_group_overlay: false,
            show_wave_overlay: false,
            show_only_blueprint: false,
            grid_hue: 0.0,
            outline_hue: 0.0,
            tools_width_vw: 40.0,
        }
    }
}

/// Read the persisted settings. Missing file or unreadable contents
/// return the defaults — the call never errors at this layer; the
/// frontend always gets a usable Settings.
#[tauri::command]
pub fn load_settings(app: AppHandle) -> Settings {
    let store = match app.store(STORE_FILE) {
        Ok(s) => s,
        Err(_) => return Settings::default(),
    };
    let raw = match store.get(SETTINGS_KEY) {
        Some(v) => v,
        None => return Settings::default(),
    };
    serde_json::from_value::<Settings>(raw).unwrap_or_default()
}

/// Merge `partial` (any subset of Settings keys) into the stored
/// JSON object and flush to disk. Unknown keys are passed through
/// unchanged so a future-version frontend won't lose data when read
/// by an older binary.
#[tauri::command]
pub fn save_settings(app: AppHandle, partial: Value) -> Result<(), String> {
    let store = app.store(STORE_FILE).map_err(|e| e.to_string())?;
    let current = store
        .get(SETTINGS_KEY)
        .unwrap_or_else(|| serde_json::to_value(Settings::default()).unwrap());
    let mut current_obj = match current {
        Value::Object(o) => o,
        _ => serde_json::Map::new(),
    };
    if let Value::Object(updates) = partial {
        for (k, v) in updates {
            current_obj.insert(k, v);
        }
    }
    store.set(SETTINGS_KEY, Value::Object(current_obj));
    store.save().map_err(|e| e.to_string())
}
