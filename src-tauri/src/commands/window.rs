//! Utilidades sobre las ventanas de Koyun (panel principal, isla, bandeja).
//!
//! Centraliza el «mostrar / ocultar» del panel para que los atajos globales,
//! la isla y la bandeja del sistema se comporten igual.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

/// Estado propio de visibilidad + anti-rebote: evita alternancias erráticas si
/// se pulsa el atajo justo durante una transición. Es solo un respaldo: la
/// fuente de verdad es `window.is_visible()`.
static VISIBLE: AtomicBool = AtomicBool::new(false);
static LAST_TOGGLE: Mutex<Option<Instant>> = Mutex::new(None);

pub fn set_visible(visible: bool) {
    VISIBLE.store(visible, Ordering::SeqCst);
}

pub fn is_visible() -> bool {
    VISIBLE.load(Ordering::SeqCst)
}

/// Muestra el panel y le da el foco.
pub fn show_panel(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        set_visible(true);
    }
}

/// Oculta el panel.
pub fn hide_panel(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
        set_visible(false);
    }
}

/// Alterna la visibilidad del panel. Ignora pulsaciones repetidas dentro de
/// 250 ms para que mantener la tecla pulsada no lo abra y cierre sin parar.
pub fn toggle_panel(app: &AppHandle) {
    {
        let mut last = LAST_TOGGLE.lock().unwrap();
        let now = Instant::now();
        if last.is_some_and(|t| now.duration_since(t) < Duration::from_millis(250)) {
            return;
        }
        *last = Some(now);
    }

    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    // Fuente de verdad: visibilidad real de la ventana (cubre los ocultados
    // hechos desde el frontend). El estado propio solo sirve de respaldo.
    let currently_visible = window.is_visible().unwrap_or_else(|_| is_visible());
    if currently_visible {
        hide_panel(app);
    } else {
        show_panel(app);
    }
}

// ---------------------------------------------------------------------------
// Comandos
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn show_main_window(app: AppHandle) {
    show_panel(&app);
}

#[tauri::command]
pub fn hide_main_window(app: AppHandle) {
    hide_panel(&app);
}

#[tauri::command]
pub fn toggle_main_window(app: AppHandle) {
    toggle_panel(&app);
}
