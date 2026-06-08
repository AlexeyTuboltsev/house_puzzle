// settings.rs — persistent user settings backed by tauri-plugin-store.
//
// One JSON file at <app_data>/settings.json, single top-level key
// "settings", whose value is a JSON object matching the `Settings`
// struct below.
//
// Rust is the single source of truth for defaults. Values in
// `Settings::default()` are what every fresh install starts with;
// the frontend never has its own defaults. Elm gates the first
// render on the `load_settings` response (`AppState.Loading ->
// Running` transition in Main.elm), so the user never sees Elm's
// literal init values — they're just type-required placeholders.
//
// `#[serde(default)]` on the struct fills missing fields from the
// Default impl during deserialisation, so a partially-populated
// stored JSON still loads cleanly.
//
// `save_settings` accepts a partial JSON object and merges it into
// the stored value. The frontend only sends fields that actually
// changed.
//
// SCHEMA_VERSION must stay in lock-step with `settingsSchemaVersion`
// in `elm/src/Main.elm`. The Elm side rejects (transitions to
// `BootstrapError`) any stored settings whose `version` doesn't
// match the constant it was compiled with — keeps stale or
// future-generated stores from landing on a half-broken UI.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

const STORE_FILE: &str = "settings.json";
const SETTINGS_KEY: &str = "settings";

/// Bump in lock-step with `settingsSchemaVersion` in `elm/src/Main.elm`.
const SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub version: u32,

    /// Absolute path of the last AI file the user opened. Empty
    /// string means "never picked anything".
    pub last_import_path: String,

    // ---- toggles (mirror Elm Model.show*)
    pub show_outlines: bool,
    pub show_grid: bool,
    pub show_numbers: bool,
    pub show_lights: bool,
    pub show_group_overlay: bool,
    pub show_wave_overlay: bool,
    pub show_only_blueprint: bool,

    // ---- colors (HSL hue, 0..360)
    pub grid_hue: f64,
    pub outline_hue: f64,

    // ---- right sidebar width (--tools-width CSS var, in vw)
    pub tools_width_vw: f64,

    // ---- export panel inputs (stored as strings so partially-typed
    //      intermediate values like "12." round-trip cleanly)
    pub export_dpi: String,
    pub export_location: String,
    pub export_house_name: String,
    pub export_position: String,
    pub export_spacing: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            version: SCHEMA_VERSION,
            last_import_path: String::new(),
            show_outlines: true,
            show_grid: false,
            show_numbers: false,
            show_lights: false,
            show_group_overlay: true,
            show_wave_overlay: true,
            show_only_blueprint: false,
            grid_hue: 35.0,
            outline_hue: 210.0,
            tools_width_vw: 40.0,
            export_dpi: "300".into(),
            export_location: "Rome".into(),
            export_house_name: "NewHouse".into(),
            export_position: "0".into(),
            export_spacing: "12.0".into(),
        }
    }
}

/// Read the persisted settings, falling back to defaults for any
/// missing fields and for the missing-file / unreadable-store cases.
/// The frontend always gets a fully-populated Settings.
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
