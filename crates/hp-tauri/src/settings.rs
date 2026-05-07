// settings.rs — persistent user settings backed by tauri-plugin-store.
//
// One JSON file at <app_data>/settings.json, single top-level key
// "settings", whose value is a JSON object.
//
// Defaults live in the FRONTEND (Elm `init`). The store only persists
// fields the user has explicitly changed — every Settings field is an
// Option, with `skip_serializing_if = "Option::is_none"` so a freshly
// installed app sees an empty object on `load_settings` and Elm keeps
// its own defaults via `Maybe.withDefault`. Trying to centralise
// defaults in Rust caused a regression on PR #106 (CI run #25499029090):
// Rust's `show_group_overlay: false` overrode Elm's `True`, and the
// per-pixel screenshot test diffed.
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    /// Schema version — emitted only on first save (set in `save_settings`)
    /// so existing-without-version stores continue to round-trip cleanly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,

    /// Absolute path of the last AI file the user opened.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_import_path: Option<String>,

    // ---- toggles
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_outlines: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_grid: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_numbers: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_lights: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_group_overlay: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_wave_overlay: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_only_blueprint: Option<bool>,

    // ---- colors (HSL hue, 0..360)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid_hue: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline_hue: Option<f64>,

    // ---- right sidebar width (--tools-width CSS var, in vw)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools_width_vw: Option<f64>,
}

/// Read the persisted settings. Missing file or unreadable contents
/// return an empty Settings (all fields None). The frontend layers
/// its own defaults on top via `Maybe.withDefault`.
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
/// by an older binary. Stamps `version` on every write so the file
/// can be migrated explicitly later.
#[tauri::command]
pub fn save_settings(app: AppHandle, partial: Value) -> Result<(), String> {
    let store = app.store(STORE_FILE).map_err(|e| e.to_string())?;
    let current = store
        .get(SETTINGS_KEY)
        .unwrap_or_else(|| Value::Object(serde_json::Map::new()));
    let mut current_obj = match current {
        Value::Object(o) => o,
        _ => serde_json::Map::new(),
    };
    if let Value::Object(updates) = partial {
        for (k, v) in updates {
            current_obj.insert(k, v);
        }
    }
    current_obj.insert("version".to_string(), Value::Number(SCHEMA_VERSION.into()));
    store.set(SETTINGS_KEY, Value::Object(current_obj));
    store.save().map_err(|e| e.to_string())
}
