//! Atajos globales configurables.
//!
//! Antes solo existía `Ctrl+Alt+K`, fijo en el código. Ahora el usuario puede
//! reasignar cada acción desde la pestaña de ajustes: los accelerators viven
//! en la configuración (`config.json`) y aquí se traducen a registros del
//! sistema operativo.

use std::collections::HashSet;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::commands::config::HotkeyConfig;

/// Acciones disponibles: identificador y etiqueta para la interfaz.
pub const ACTIONS: &[(&str, &str)] = &[
    ("toggle_panel", "Mostrar u ocultar el panel"),
    ("mute_mic", "Silenciar el micrófono"),
    ("media_play_pause", "Reproducir o pausar"),
    ("media_next", "Pista siguiente"),
    ("media_prev", "Pista anterior"),
    ("toggle_dnd", "Alternar «No molestar»"),
    ("show_clipboard", "Abrir el portapapeles"),
    ("lock_screen", "Bloquear la pantalla"),
];

/// Estado de una acción tal y como lo ve la interfaz.
#[derive(Serialize, Clone)]
pub struct HotkeyBinding {
    pub action: String,
    pub label: String,
    pub accelerator: String,
    /// `true` si el accelerator está registrado correctamente en el sistema.
    pub registered: bool,
    /// Mensaje de error si no se pudo registrar.
    pub error: Option<String>,
}

/// Devuelve la asignación actual de cada acción.
#[tauri::command]
pub fn hotkeys_list() -> Vec<HotkeyBinding> {
    let cfg = crate::commands::config::current();
    let bindings = bindings_of(&cfg.hotkeys);
    ACTIONS
        .iter()
        .map(|(action, label)| {
            let accelerator = bindings
                .iter()
                .find(|(a, _)| a == action)
                .map(|(_, v)| v.clone())
                .unwrap_or_default();
            let trimmed = accelerator.trim();
            // Una cadena vacía significa «sin asignar».
            let (registered, error) = if trimmed.is_empty() {
                (false, None)
            } else {
                match trimmed.parse::<Shortcut>() {
                    Ok(_) => (true, None),
                    Err(err) => (false, Some(format!("Combinación no válida: {err}"))),
                }
            };
            HotkeyBinding {
                action: (*action).to_string(),
                label: (*label).to_string(),
                accelerator,
                registered,
                error,
            }
        })
        .collect()
}

/// Extrae los accelerators del bloque de configuración.
fn bindings_of(cfg: &HotkeyConfig) -> Vec<(&'static str, String)> {
    vec![
        ("toggle_panel", cfg.toggle_panel.clone()),
        ("mute_mic", cfg.mute_mic.clone()),
        ("media_play_pause", cfg.media_play_pause.clone()),
        ("media_next", cfg.media_next.clone()),
        ("media_prev", cfg.media_prev.clone()),
        ("toggle_dnd", cfg.toggle_dnd.clone()),
        ("show_clipboard", cfg.show_clipboard.clone()),
        ("lock_screen", cfg.lock_screen.clone()),
    ]
}

/// Registra todos los atajos de la configuración, sustituyendo los anteriores.
///
/// Se llama al arrancar y cada vez que el usuario guarda un cambio.
pub fn apply(app: &AppHandle) -> Result<(), String> {
    let cfg = crate::commands::config::current();
    apply_with_config(app, &cfg.hotkeys)
}

/// Registra todos los atajos indicados, sustituyendo los anteriores.
pub fn apply_with_config(app: &AppHandle, cfg: &HotkeyConfig) -> Result<(), String> {
    let manager = app.global_shortcut();
    // `unregister_all` limpia también el atajo fijo que se registraba antes
    // desde `lib.rs`.
    let _ = manager.unregister_all();

    let mut usados: HashSet<String> = HashSet::new();
    for (action, accelerator) in bindings_of(cfg) {
        let accelerator = accelerator.trim().to_string();
        if accelerator.is_empty() {
            continue;
        }
        // Dos acciones no pueden compartir combinación: Windows no lo permite
        // y el registro fallaría con «ya registrado».
        if !usados.insert(accelerator.to_ascii_lowercase()) {
            crate::commands::log::append(
                "HOTKEY",
                &format!("La combinación «{accelerator}» está repetida; se ignora"),
            );
            continue;
        }

        let parsed = match accelerator.parse::<Shortcut>() {
            Ok(shortcut) => shortcut,
            Err(err) => {
                crate::commands::log::append(
                    "HOTKEY",
                    &format!("La combinación «{accelerator}» no es válida: {err}"),
                );
                continue;
            }
        };

        let action = action.to_string();
        let label = accelerator.clone();
        if let Err(err) = manager.on_shortcut(parsed, move |app, _shortcut, event| {
            if event.state != ShortcutState::Pressed {
                return;
            }
            dispatch(app, &action);
        }) {
            crate::commands::log::append(
                "HOTKEY",
                &format!("No se pudo registrar «{label}»: {err}"),
            );
        }
    }
    Ok(())
}

#[tauri::command]
pub fn hotkeys_apply(app: AppHandle) -> Result<(), String> {
    apply(&app)
}

/// Ejecuta la acción asociada a un atajo.
fn dispatch(app: &AppHandle, action: &str) {
    match action {
        "toggle_panel" => crate::commands::window::toggle_panel(app),
        "mute_mic" => {
            if let Err(err) = crate::commands::audio::toggle_input_mute() {
                crate::commands::log::append("HOTKEY", &format!("Micrófono: {err}"));
            }
        }
        "media_play_pause" => {
            let _ = crate::commands::media::media_action_sync("play_pause");
        }
        "media_next" => {
            let _ = crate::commands::media::media_action_sync("next");
        }
        "media_prev" => {
            let _ = crate::commands::media::media_action_sync("prev");
        }
        "toggle_dnd" => {
            let _ = crate::commands::controls::dnd_toggle();
        }
        "show_clipboard" => {
            crate::commands::window::show_panel(app);
            let _ = app.emit("koyun:tab", "clipboard");
        }
        "lock_screen" => {
            let _ = crate::commands::power::power_lock_sync();
        }
        _ => {}
    }
}
