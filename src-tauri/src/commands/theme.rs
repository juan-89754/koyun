//! Detección del aspecto del sistema operativo.
//!
//! Windows expone el modo claro/oscuro y el color de acento en el registro.
//! Se leen ambos para que el panel pueda seguir al sistema (opción «auto») y
//! para rellenar el color de acento cuando el usuario lo pide.

use serde::Serialize;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::core::w;

use crate::commands::util::blocking;

#[derive(Serialize, Clone, Debug)]
pub struct SystemTheme {
    /// `true` si Windows está en modo claro.
    pub light: bool,
    /// Color de acento del sistema en formato `#rrggbb`.
    pub accent: String,
    /// `true` si el valor de acento se pudo leer del registro.
    pub accent_available: bool,
}

/// Lee un `REG_DWORD` del registro del usuario actual.
fn read_dword(path: windows::core::PCWSTR, name: windows::core::PCWSTR) -> Option<u32> {
    let mut value: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            path,
            name,
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut core::ffi::c_void),
            Some(&mut size),
        )
    };
    if status == ERROR_SUCCESS {
        Some(value)
    } else {
        None
    }
}

/// Convierte el `DWORD` ABGR de Windows (formato `0x00BBGGRR`) a `#rrggbb`.
fn abgr_to_hex(raw: u32) -> String {
    let r = raw & 0xFF;
    let g = (raw >> 8) & 0xFF;
    let b = (raw >> 16) & 0xFF;
    format!("#{r:02x}{g:02x}{b:02x}")
}

const PERSONALIZE: windows::core::PCWSTR =
    w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
const DWM_ACCENT: windows::core::PCWSTR = w!("Software\\Microsoft\\Windows\\DWM");
const EXPLORER_ACCENT: windows::core::PCWSTR =
    w!("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Accent");

fn system_theme_inner() -> SystemTheme {
    // `AppsUseLightTheme` = 1 → modo claro. `SystemUsesLightTheme` cubre el
    // resto de superficies del sistema; se usa como respaldo.
    let light = read_dword(PERSONALIZE, w!("AppsUseLightTheme"))
        .or_else(|| read_dword(PERSONALIZE, w!("SystemUsesLightTheme")))
        .map(|v| v != 0)
        .unwrap_or(false);

    // `AccentColor` de DWM es el color «moderno» (Windows 10 1809+); el de
    // Explorer es el color clásico del menú de inicio.
    let raw = read_dword(DWM_ACCENT, w!("AccentColor"))
        .or_else(|| read_dword(EXPLORER_ACCENT, w!("AccentColorMenu")))
        .or_else(|| read_dword(EXPLORER_ACCENT, w!("AccentColor")));

    let (accent, accent_available) = match raw {
        Some(value) => (abgr_to_hex(value), true),
        None => ("#2dd4bf".to_string(), false),
    };

    SystemTheme {
        light,
        accent,
        accent_available,
    }
}

#[tauri::command]
pub async fn theme_system() -> Result<SystemTheme, String> {
    blocking(|| Ok(system_theme_inner())).await
}
