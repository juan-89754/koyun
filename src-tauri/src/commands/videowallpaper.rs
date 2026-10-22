use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager};
use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowExW, FindWindowW, GetClientRect, GetParent, SendMessageTimeoutW,
    SetParent, SetWindowPos, SMTO_NORMAL, SWP_NOACTIVATE, SWP_NOZORDER, WNDENUMPROC,
};
use windows::core::{w, BOOL};

use crate::commands::util::blocking;

/// Fondo en movimiento activo (`None` = imagen estática).
static CURRENT: Mutex<Option<String>> = Mutex::new(None);

fn state_file() -> Option<PathBuf> {
    let base = std::env::var("APPDATA").ok().map(PathBuf::from)?;
    Some(base.join("koyun").join("fondo-animado.json"))
}

fn persist(path: Option<&str>) {
    if let Some(file) = state_file() {
        if let Some(dir) = file.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let payload = serde_json::json!({ "path": path });
        let _ = fs::write(file, payload.to_string());
    }
}

fn persisted_path() -> Option<String> {
    let file = state_file()?;
    let text = fs::read_to_string(file).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    value.get("path")?.as_str().map(str::to_string)
}

/// Encuentra la ventana `WorkerW` que queda **detrás** de los iconos del
/// escritorio. Requiere el mensaje 0x052C a Progman para que Windows cree esa
/// superficie (truco clásico de los reproductores de fondo).
fn find_workerw() -> Result<HWND, String> {
    unsafe {
        let progman = FindWindowW(w!("Progman"), None).map_err(|e| format!("Progman: {e}"))?;
        let _ = SendMessageTimeoutW(
            progman,
            0x052C,
            WPARAM(0),
            LPARAM(0),
            SMTO_NORMAL,
            1000,
            None,
        );

        let mut holder = HWND::default();
        let raw: *mut HWND = &mut holder;
        let callback: WNDENUMPROC = Some(enum_windows_proc);
        let _ = EnumWindows(callback, LPARAM(raw as isize));

        if !holder.is_invalid() {
            if let Ok(worker) = FindWindowExW(None, Some(holder), w!("WorkerW"), None) {
                return Ok(worker);
            }
        }
        Err("Windows no expuso la superficie del escritorio (WorkerW)".to_string())
    }
}

unsafe extern "system" fn enum_windows_proc(top: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
        let holder = &mut *(lparam.0 as *mut HWND);
        if holder.is_invalid()
            && FindWindowExW(Some(top), None, w!("SHELLDLL_DefView"), None).is_ok()
        {
            *holder = top;
            return BOOL(0);
        }
        BOOL(1)
    }
}

fn attach(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("wallpaper")
        .ok_or_else(|| "Ventana de fondo no disponible".to_string())?;
    let hwnd = window.hwnd().map_err(|e| format!("Ventana de fondo: {e}"))?;
    let workerw = find_workerw()?;

    // SetParent devuelve el padre anterior (nulo para una ventana de nivel
    // superior) y por eso puede fallar en apariencia aunque vaya bien: se
    // comprueba el padre real en lugar de fiarse del resultado.
    let _ = unsafe { SetParent(hwnd, Some(workerw)) };
    let parent = unsafe { GetParent(hwnd) }.map_err(|e| format!("No se pudo insertar el fondo: {e}"))?;
    if parent != workerw {
        return Err("No se pudo colocar el vídeo detrás de los iconos".to_string());
    }

    let mut rect = RECT::default();
    unsafe { GetClientRect(workerw, &mut rect).map_err(|e| format!("Escritorio: {e}"))? };
    let width = (rect.right - rect.left).max(1);
    let height = (rect.bottom - rect.top).max(1);
    unsafe {
        SetWindowPos(
            hwnd,
            None,
            0,
            0,
            width,
            height,
            SWP_NOZORDER | SWP_NOACTIVATE,
        )
        .map_err(|e| format!("Tamaño del fondo: {e}"))?;
    }

    // Nunca captura el ratón: aunque algo falle, el escritorio sigue siendo usable.
    let _ = window.set_ignore_cursor_events(true);
    let _ = window.show();
    Ok(())
}

fn detach(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("wallpaper") {
        let _ = window.hide();
        if let Ok(hwnd) = window.hwnd() {
            let _ = unsafe { SetParent(hwnd, None) };
        }
    }
}

fn start_inner(app: AppHandle, path: String) -> Result<(), String> {
    let file = PathBuf::from(&path);
    if !file.is_file() {
        return Err("El vídeo del fondo no existe".to_string());
    }
    *CURRENT.lock().unwrap_or_else(|e| e.into_inner()) = Some(path.clone());
    persist(Some(&path));

    attach(&app)?;

    // Aviso a la página del fondo (si ya estaba cargada; si no, la página pide
    // el estado con `wallpaper_video_state` al arrancar).
    if let Some(window) = app.get_webview_window("wallpaper") {
        let _ = window.emit("wallpaper:video", Some(path));
    }
    Ok(())
}

fn stop_inner(app: AppHandle) -> Result<(), String> {
    *CURRENT.lock().unwrap_or_else(|e| e.into_inner()) = None;
    persist(None);
    detach(&app);
    if let Some(window) = app.get_webview_window("wallpaper") {
        let _ = window.emit("wallpaper:video", Option::<String>::None);
    }
    Ok(())
}

#[tauri::command]
pub async fn wallpaper_video_start(app: AppHandle, path: String) -> Result<(), String> {
    blocking(move || start_inner(app, path)).await
}

#[tauri::command]
pub async fn wallpaper_video_stop(app: AppHandle) -> Result<(), String> {
    blocking(move || stop_inner(app)).await
}

#[tauri::command]
pub async fn wallpaper_video_state() -> Result<Option<String>, String> {
    Ok(CURRENT
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone())
}

/// `true` si ahora mismo hay un vídeo cubriendo el escritorio.
pub fn is_active() -> bool {
    CURRENT
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .is_some()
}

/// Restaura un fondo en movimiento pendiente de la sesión anterior.
pub fn restore(app: &AppHandle) {
    let Some(path) = persisted_path() else {
        return;
    };
    if !PathBuf::from(&path).is_file() {
        persist(None);
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        if let Err(err) = start_inner(app, path) {
            crate::commands::log::append("WALLPAPER", &format!("Fondo en movimiento: {err}"));
            persist(None);
        }
    });
}

#[tauri::command]
pub async fn wallpaper_video_pause_game(app: AppHandle) -> Result<(), String> {
    blocking(move || {
        if let Some(window) = app.get_webview_window("wallpaper") {
            let _ = window.emit("wallpaper:game-pause", ());
        }
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn wallpaper_video_resume_game(app: AppHandle) -> Result<(), String> {
    blocking(move || {
        if let Some(window) = app.get_webview_window("wallpaper") {
            let _ = window.emit("wallpaper:game-resume", ());
        }
        Ok(())
    })
    .await
}

pub fn pause_game(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("wallpaper") {
        let _ = window.emit("wallpaper:game-pause", ());
    }
}

pub fn resume_game(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("wallpaper") {
        let _ = window.emit("wallpaper:game-resume", ());
    }
}

/// Detiene el vídeo al cerrar la aplicación para no dejar la ventana huérfana.
pub fn shutdown(app: &AppHandle) {
    if CURRENT.lock().unwrap_or_else(|e| e.into_inner()).is_some() {
        detach(app);
    }
}
