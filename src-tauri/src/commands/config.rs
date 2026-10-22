//! Configuración persistente de Koyun.
//!
//! Antes los ajustes vivían dispersos: `localStorage` en el frontend y un JSON
//! suelto (`fondo-animado.json`) para el fondo en movimiento. Aquí se
//! concentran en un único archivo `%APPDATA%\koyun\config.json` que se lee al
//! arrancar y se guarda en cuanto cambia algo.
//!
//! El archivo se **fusiona** sobre los valores por defecto: así una versión
//! nueva que añade campos no invalida la configuración existente.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

fn config_path() -> Option<PathBuf> {
    let base = std::env::var("APPDATA").ok().map(PathBuf::from)?;
    Some(base.join("koyun").join("config.json"))
}

// ---------------------------------------------------------------------------
// Estructuras
// ---------------------------------------------------------------------------

/// Aspecto visual del panel.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ThemeConfig {
    /// `auto` | `light` | `dark`
    pub mode: String,
    /// `system` (color de acento de Windows) | `custom`
    pub accent_mode: String,
    /// Color de acento personalizado en formato `#rrggbb`.
    pub accent_color: String,
    /// Paleta predefinida: `""` | `midnight` | `cyberpunk` | `minimal` | `ocean` | `sunset`.
    pub preset: String,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            mode: "auto".into(),
            accent_mode: "system".into(),
            accent_color: "#2dd4bf".into(),
            preset: String::new(),
        }
    }
}

/// Atajos globales. La cadena es un acelerador entendido por
/// `tauri-plugin-global-shortcut` (`Ctrl+Alt+K`); vacío = desasignado.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct HotkeyConfig {
    pub toggle_panel: String,
    pub mute_mic: String,
    pub media_play_pause: String,
    pub media_next: String,
    pub media_prev: String,
    pub toggle_dnd: String,
    pub show_clipboard: String,
    pub lock_screen: String,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            toggle_panel: "Ctrl+Alt+K".into(),
            mute_mic: "Ctrl+Alt+M".into(),
            media_play_pause: "Ctrl+Alt+P".into(),
            media_next: "Ctrl+Alt+Right".into(),
            media_prev: "Ctrl+Alt+Left".into(),
            toggle_dnd: String::new(),
            show_clipboard: "Ctrl+Alt+V".into(),
            lock_screen: String::new(),
        }
    }
}

/// Ajustes del fondo en movimiento.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct VideoConfig {
    /// Pausar el vídeo cuando otra aplicación ocupa toda la pantalla.
    pub pause_on_fullscreen: bool,
    /// Pausar el vídeo cuando el equipo funciona con batería.
    pub pause_on_battery: bool,
    /// Garantizar que el vídeo nunca reproduce audio.
    pub always_muted: bool,
}

impl Default for VideoConfig {
    fn default() -> Self {
        Self {
            pause_on_fullscreen: true,
            pause_on_battery: true,
            always_muted: true,
        }
    }
}

/// Contenido de la isla dinámica.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct IslandConfig {
    /// `logo` | `media` | `stats` | `auto`
    pub widget: String,
    /// Umbral (porcentaje) a partir del cual la isla muestra CPU/RAM.
    pub stats_threshold: u8,
}

impl Default for IslandConfig {
    fn default() -> Self {
        Self {
            widget: "auto".into(),
            stats_threshold: 80,
        }
    }
}

/// Captura de notificaciones del sistema.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct NotificationsConfig {
    /// Capturar los avisos (toasts) de otras aplicaciones.
    pub enabled: bool,
    /// Número máximo de avisos guardados en el cajón.
    pub keep: usize,
}

impl Default for NotificationsConfig {
    fn default() -> Self {
        Self { enabled: true, keep: 50 }
    }
}

/// Actualizaciones automáticas.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct UpdaterConfig {
    /// Buscar actualizaciones al arrancar.
    pub auto_check: bool,
    /// Avisar en la bandeja cuando hay una versión nueva.
    pub notify: bool,
}

impl Default for UpdaterConfig {
    fn default() -> Self {
        Self {
            auto_check: true,
            notify: true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    pub theme: ThemeConfig,
    pub hotkeys: HotkeyConfig,
    pub video: VideoConfig,
    pub island: IslandConfig,
    pub notifications: NotificationsConfig,
    pub updater: UpdaterConfig,
    /// Ajustes libres que no encajan en ninguna sección (compatibilidad futura).
    pub extra: BTreeMap<String, serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Carga / guardado
// ---------------------------------------------------------------------------

fn load() -> AppConfig {
    let Some(path) = config_path() else {
        return AppConfig::default();
    };
    let Ok(text) = fs::read_to_string(&path) else {
        return AppConfig::default();
    };
    match serde_json::from_str::<AppConfig>(&text) {
        Ok(cfg) => cfg,
        Err(err) => {
            crate::commands::log::append(
                "CONFIG",
                &format!("No se pudo leer la configuración ({err}); se usan los valores por defecto"),
            );
            AppConfig::default()
        }
    }
}

static STORE: OnceLock<Mutex<AppConfig>> = OnceLock::new();

fn store() -> &'static Mutex<AppConfig> {
    STORE.get_or_init(|| Mutex::new(load()))
}

/// Configuración actual (copia).
pub fn current() -> AppConfig {
    store()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

/// Carga la configuración al arrancar. Devuelve `Err` solo si el archivo
/// existe pero es ilegible; si no existe se usan los valores por defecto.
pub fn init(_app: &tauri::AppHandle) -> Result<(), String> {
    let cfg = current();
    crate::commands::log::append(
        "CONFIG",
        &format!("Configuración cargada (tema: {})", cfg.theme.mode),
    );
    Ok(())
}

/// Guarda la configuración en disco. Devuelve `false` si no se pudo escribir.
pub fn save(cfg: &AppConfig) -> bool {
    let Some(path) = config_path() else {
        return false;
    };
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    match serde_json::to_string_pretty(cfg) {
        Ok(text) => fs::write(&path, text).is_ok(),
        Err(err) => {
            crate::commands::log::append("CONFIG", &format!("No se pudo serializar: {err}"));
            false
        }
    }
}

/// Fusiona recursivamente dos objetos JSON (los valores de `patch` ganan).
fn merge(base: &mut serde_json::Value, patch: &serde_json::Value) {
    match (base, patch) {
        (serde_json::Value::Object(base_map), serde_json::Value::Object(patch_map)) => {
            for (key, value) in patch_map {
                match base_map.get_mut(key) {
                    Some(existing) => merge(existing, value),
                    None => {
                        base_map.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        (base, patch) => {
            *base = patch.clone();
        }
    }
}

// ---------------------------------------------------------------------------
// Comandos
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn config_get() -> AppConfig {
    current()
}

/// Aplica un parche de configuración (merge profundo) y lo guarda.
#[tauri::command]
pub fn config_set(app: tauri::AppHandle, patch: serde_json::Value) -> AppConfig {
    let mut guard = store().lock().unwrap_or_else(|e| e.into_inner());
    let mut raw = serde_json::to_value(&*guard).unwrap_or_else(|_| serde_json::json!({}));
    merge(&mut raw, &patch);

    // Se reconstruye desde el JSON fusionado para respetar `#[serde(default)]`.
    let updated: AppConfig = match serde_json::from_value(raw) {
        Ok(cfg) => cfg,
        Err(err) => {
            crate::commands::log::append("CONFIG", &format!("Parche inválido: {err}"));
            return guard.clone();
        }
    };

    *guard = updated.clone();
    drop(guard);

    if !save(&updated) {
        crate::commands::log::append("CONFIG", "No se pudo guardar la configuración en disco");
    }

    // Los atajos son estado del proceso: hay que volver a registrarlos.
    let _ = crate::commands::hotkeys::apply_with_config(&app, &updated.hotkeys);

    updated
}
