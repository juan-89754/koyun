//! Modo Juego - Versión segura
//!
//! Objetivo: limitar la carga de Windows sobre procesos secundarios,
//! dar mayor prioridad al juego en foco y reducir interrupciones (Wi-Fi scan).
//! Todo se restaura obligatoriamente al desactivar o al cerrar la app.

use std::collections::HashMap;
use std::process::Command;
use std::sync::{LazyLock, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use windows::Win32::Foundation::{CloseHandle, HWND};
use windows::Win32::System::Threading::{
    GetPriorityClass, OpenProcess, SetPriorityClass, ABOVE_NORMAL_PRIORITY_CLASS,
    BELOW_NORMAL_PRIORITY_CLASS, HIGH_PRIORITY_CLASS, IDLE_PRIORITY_CLASS,
    NORMAL_PRIORITY_CLASS, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_INFORMATION,
};

use crate::commands::util::blocking;

#[derive(Default)]
struct GameModeState {
    enabled: bool,
    original_priorities: HashMap<u32, u32>,
    wifi_interface: Option<String>,
    wifi_autoconfig_was_enabled: Option<bool>,
    wallpaper_was_active: bool,
    target_pid: Option<u32>,
    target_exe: Option<String>,
}

static STATE: LazyLock<Mutex<GameModeState>> = LazyLock::new(|| Mutex::new(GameModeState::default()));

/// Handle global para poder restaurar desde el hook de pánico.
static APP: Mutex<Option<AppHandle>> = Mutex::new(None);

pub fn init(app: &AppHandle) {
    *APP.lock().unwrap_or_else(|e| e.into_inner()) = Some(app.clone());
}

/// Restaura sin necesidad de un `AppHandle` explícito (hook de pánico).
pub fn force_restore_from_static() {
    let app = APP.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if let Some(app) = app {
        force_restore_all(&app);
    } else {
        // Sin handle no se puede reanudar el vídeo, pero sí prioridades y Wi-Fi.
        force_restore_priorities_sync();
        force_restore_wifi_sync();
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GameModeStatus {
    pub enabled: bool,
    pub target_pid: Option<u32>,
    pub target_exe: Option<String>,
    pub wifi_autoconfig_was_enabled: Option<bool>,
    pub wifi_interface: Option<String>,
}

fn is_windows() -> bool {
    cfg!(windows)
}

fn priority_to_u32(p: u32) -> &'static str {
    match p {
        0x00000040 => "IDLE_PRIORITY_CLASS",
        0x00004000 => "BELOW_NORMAL_PRIORITY_CLASS",
        0x00000020 => "NORMAL_PRIORITY_CLASS",
        0x00008000 => "ABOVE_NORMAL_PRIORITY_CLASS",
        0x00000080 => "HIGH_PRIORITY_CLASS",
        0x00000100 => "REALTIME_PRIORITY_CLASS",
        _ => "UNKNOWN",
    }
}

fn u32_to_class(p: u32) -> Option<windows::Win32::System::Threading::PROCESS_CREATION_FLAGS> {
    match p {
        0x00000040 => Some(IDLE_PRIORITY_CLASS),
        0x00004000 => Some(BELOW_NORMAL_PRIORITY_CLASS),
        0x00000020 => Some(NORMAL_PRIORITY_CLASS),
        0x00008000 => Some(ABOVE_NORMAL_PRIORITY_CLASS),
        0x00000080 => Some(HIGH_PRIORITY_CLASS),
        0x00000100 => Some(HIGH_PRIORITY_CLASS), // Nunca restaurar a REALTIME, forzamos HIGH como tope seguro
        _ => None,
    }
}

unsafe fn get_foreground_pid() -> Option<u32> {
    unsafe {
        let hwnd: HWND = windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }
        let mut pid: u32 = 0;
        let _ = windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 { None } else { Some(pid) }
    }
}

unsafe fn get_exe_name(pid: u32) -> Option<String> {
    unsafe {
        let handle = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(h) => h,
            Err(_) => return None,
        };
        let mut buf = [0u16; 260];
        let res = windows::Win32::System::ProcessStatus::GetProcessImageFileNameW(handle, &mut buf);
        let _ = CloseHandle(handle);
        if res == 0 { return None; }
        let s = String::from_utf16_lossy(&buf[..res as usize]);
        // Obtener solo nombre de archivo
        s.split('\\').last().map(|x| x.to_string()).or(Some(s))
    }
}

fn elevate_game_priority(pid: u32) -> Result<(u32, String), String> {
    if !is_windows() { return Err("Solo Windows".into()); }
    unsafe {
        let handle = OpenProcess(PROCESS_SET_INFORMATION | PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
            .map_err(|e| format!("OpenProcess fallo: {e}"))?;
        let current = GetPriorityClass(handle);
        let exe = get_exe_name(pid).unwrap_or_default();
        // Elevar a HIGH_PRIORITY_CLASS si no está ya en HIGH o REALTIME
        if current != HIGH_PRIORITY_CLASS.0 && current != 0x00000100 {
            let _ = SetPriorityClass(handle, HIGH_PRIORITY_CLASS);
        }
        let _ = CloseHandle(handle);
        Ok((current, exe))
    }
}

fn restore_priority(pid: u32, orig: u32) {
    if !is_windows() { return; }
    if let Some(cls) = u32_to_class(orig) {
        unsafe {
            if let Ok(handle) = OpenProcess(PROCESS_SET_INFORMATION, false, pid) {
                let _ = SetPriorityClass(handle, cls);
                let _ = CloseHandle(handle);
            }
        }
    }
}

fn run_netsh(args: &[&str]) -> std::io::Result<std::process::Output> {
    let mut cmd = Command::new("netsh");
    cmd.args(args);
    #[cfg(windows)]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    cmd.output()
}

fn get_wifi_interface() -> Option<String> {
    if !is_windows() { return None; }
    let out = run_netsh(&["wlan", "show", "interfaces"]).ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        let l = line.trim();
        if l.to_lowercase().starts_with("name") {
            if let Some(idx) = l.find(':') {
                let name = l[idx+1..].trim().to_string();
                if !name.is_empty() { return Some(name); }
            }
        }
    }
    Some("Wi-Fi".to_string())
}

fn get_wifi_autoconfig_enabled(iface: &str) -> Option<bool> {
    if !is_windows() { return None; }
    let out = run_netsh(&["wlan", "show", "autoconfig", &format!("interface={}", iface)]).ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        let l = line.to_lowercase();
        if l.contains("enabled") {
            if l.contains("yes") { return Some(true); }
            if l.contains("no") { return Some(false); }
        }
    }
    None
}

fn set_wifi_autoconfig(iface: &str, enabled: bool) -> bool {
    if !is_windows() { return false; }
    let val = if enabled { "yes" } else { "no" };
    match run_netsh(&["wlan", "set", "autoconfig", &format!("enabled={}", val), &format!("interface={}", iface)]) {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}

fn force_restore_wifi_sync() {
    let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if let (Some(iface), Some(was)) = (st.wifi_interface.clone(), st.wifi_autoconfig_was_enabled) {
        if was {
            // Restaurar síncronamente
            let _ = run_netsh(&["wlan", "set", "autoconfig", "enabled=yes", &format!("interface={}", iface)])
                .map(|o| o.status.success());
        }
    }
    st.wifi_autoconfig_was_enabled = None;
    st.wifi_interface = None;
}

fn force_restore_priorities_sync() {
    let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let to_restore: Vec<(u32, u32)> = st.original_priorities.drain().collect();
    drop(st);
    for (pid, orig) in to_restore {
        restore_priority(pid, orig);
    }
}

#[tauri::command]
pub fn game_mode_status() -> GameModeStatus {
    let st = STATE.lock().unwrap_or_else(|e| e.into_inner());
    GameModeStatus {
        enabled: st.enabled,
        target_pid: st.target_pid,
        target_exe: st.target_exe.clone(),
        wifi_autoconfig_was_enabled: st.wifi_autoconfig_was_enabled,
        wifi_interface: st.wifi_interface.clone(),
    }
}

#[tauri::command]
pub async fn game_mode_use_foreground(app: AppHandle) -> Result<GameModeStatus, String> {
    blocking(move || {
        if !is_windows() { return Err("Solo Windows".into()); }
        let pid = unsafe { get_foreground_pid() }.ok_or("No hay ventana en primer plano")?;
        let exe = unsafe { get_exe_name(pid) }.unwrap_or_default();
        let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
        st.target_pid = Some(pid);
        st.target_exe = if exe.is_empty() { None } else { Some(exe) };
        let _ = app.emit("koyun:game-mode:target", st.target_exe.clone().unwrap_or_default());
        Ok(GameModeStatus {
            enabled: st.enabled,
            target_pid: st.target_pid,
            target_exe: st.target_exe.clone(),
            wifi_autoconfig_was_enabled: st.wifi_autoconfig_was_enabled,
            wifi_interface: st.wifi_interface.clone(),
        })
    }).await
}

#[tauri::command]
pub async fn game_mode_enable(app: AppHandle, limit_wifi: bool) -> Result<GameModeStatus, String> {
    blocking(move || {
        if !is_windows() { return Err("Solo Windows".into()); }
        let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
        if st.enabled {
            return Ok(GameModeStatus {
                enabled: true,
                target_pid: st.target_pid,
                target_exe: st.target_exe.clone(),
                wifi_autoconfig_was_enabled: st.wifi_autoconfig_was_enabled,
                wifi_interface: st.wifi_interface.clone(),
            });
        }

        // Detectar objetivo
        if st.target_pid.is_none() {
            if let Some(pid) = unsafe { get_foreground_pid() } {
                st.target_pid = Some(pid);
                st.target_exe = unsafe { get_exe_name(pid) };
            }
        }
        let pid = st.target_pid.ok_or("Selecciona el juego en primer plano o establece objetivo")?;

        // Elevar prioridad
        match elevate_game_priority(pid) {
            Ok((orig, exe)) => {
                if st.target_exe.is_none() && !exe.is_empty() {
                    st.target_exe = Some(exe);
                }
                st.original_priorities.insert(pid, orig);
            }
            Err(e) => {
                crate::commands::log::append("GAME_MODE", &format!("No se pudo elevar prioridad: {e}"));
            }
        }

        // Wi-Fi autoconfig
        if limit_wifi {
            if st.wifi_autoconfig_was_enabled.is_none() {
                if let Some(iface) = get_wifi_interface() {
                    if let Some(enabled) = get_wifi_autoconfig_enabled(&iface) {
                        if enabled {
                            st.wifi_interface = Some(iface.clone());
                            st.wifi_autoconfig_was_enabled = Some(true);
                            let _ = set_wifi_autoconfig(&iface, false);
                            crate::commands::log::append("GAME_MODE", &format!("Wi-Fi autoconfig deshabilitado en {}", iface));
                        } else {
                            st.wifi_interface = Some(iface);
                            st.wifi_autoconfig_was_enabled = Some(false);
                        }
                    } else {
                        if let Some(iface) = get_wifi_interface() {
                            st.wifi_interface = Some(iface);
                        }
                    }
                }
            }
        }

        // Pausar vídeo de fondo si está activo
        if crate::commands::videowallpaper::is_active() {
            st.wallpaper_was_active = true;
            crate::commands::videowallpaper::pause_game(&app);
        } else {
            st.wallpaper_was_active = false;
        }

        st.enabled = true;
        drop(st);
        let _ = app.emit("koyun:game-mode", true);
        crate::commands::log::append("GAME_MODE", "Modo Juego ACTIVADO");

        let st2 = STATE.lock().unwrap_or_else(|e| e.into_inner());
        Ok(GameModeStatus {
            enabled: st2.enabled,
            target_pid: st2.target_pid,
            target_exe: st2.target_exe.clone(),
            wifi_autoconfig_was_enabled: st2.wifi_autoconfig_was_enabled,
            wifi_interface: st2.wifi_interface.clone(),
        })
    }).await
}

#[tauri::command]
pub async fn game_mode_disable(app: AppHandle) -> Result<GameModeStatus, String> {
    blocking(move || {
        if !is_windows() { return Err("Solo Windows".into()); }
        let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
        if !st.enabled {
            return Ok(GameModeStatus {
                enabled: false,
                target_pid: st.target_pid,
                target_exe: st.target_exe.clone(),
                wifi_autoconfig_was_enabled: None,
                wifi_interface: None,
            });
        }

        // Restaurar prioridades
        let to_restore: Vec<(u32, u32)> = st.original_priorities.drain().collect();
        drop(st);
        for (pid, orig) in to_restore {
            restore_priority(pid, orig);
        }

        // Restaurar Wi-Fi
        force_restore_wifi_sync();

        // Reanudar vídeo
        let st2 = STATE.lock().unwrap_or_else(|e| e.into_inner());
        if st2.wallpaper_was_active {
            crate::commands::videowallpaper::resume_game(&app);
        }
        let mut st3 = STATE.lock().unwrap_or_else(|e| e.into_inner());
        st3.enabled = false;
        st3.wallpaper_was_active = false;
        drop(st3);
        let _ = app.emit("koyun:game-mode", false);
        crate::commands::log::append("GAME_MODE", "Modo Juego DESACTIVADO");

        let st4 = STATE.lock().unwrap_or_else(|e| e.into_inner());
        Ok(GameModeStatus {
            enabled: st4.enabled,
            target_pid: st4.target_pid,
            target_exe: st4.target_exe.clone(),
            wifi_autoconfig_was_enabled: None,
            wifi_interface: None,
        })
    }).await
}

/// Llamado obligatoriamente al cerrar la app
pub fn force_restore_all(app: &AppHandle) {
    if !is_windows() { return; }
    force_restore_priorities_sync();
    force_restore_wifi_sync();
    let st = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if st.wallpaper_was_active {
        crate::commands::videowallpaper::resume_game(app);
    }
    let mut st2 = STATE.lock().unwrap_or_else(|e| e.into_inner());
    st2.enabled = false;
    st2.wallpaper_was_active = false;
    st2.original_priorities.clear();
    let _ = app.emit("koyun:game-mode", false);
    crate::commands::log::append("GAME_MODE", "Restauración forzosa al salir");
}
