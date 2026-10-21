use std::path::{Path, PathBuf};

use serde::Serialize;
use windows::Win32::UI::Shell::{DesktopWallpaper, IDesktopWallpaper};
use windows::Win32::UI::WindowsAndMessaging::{
    SPI_GETDESKWALLPAPER, SPI_SETDESKWALLPAPER, SPIF_SENDCHANGE, SPIF_UPDATEINIFILE,
    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
};

use crate::commands::util::blocking;

#[derive(Serialize, Clone)]
pub struct Wallpaper {
    pub path: String,
    pub name: String,
    pub source: String,
    pub kind: String,
    pub ext: String,
}

const IMAGE_EXTS: [&str; 7] = ["jpg", "jpeg", "png", "bmp", "webp", "tif", "tiff"];
const VIDEO_EXTS: [&str; 4] = ["mp4", "webm", "mkv", "avi"];
const MAX_ITEMS: usize = 400;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn is_image_ext(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| IMAGE_EXTS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn is_video_ext(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| VIDEO_EXTS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn ext_lower(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default()
}

fn push_wallpaper(path: PathBuf, source: &str, out: &mut Vec<Wallpaper>) {
    if out.len() >= MAX_ITEMS {
        return;
    }
    let kind = if is_video_ext(&path) { "video" } else { "image" };
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Fondo")
        .to_string();
    out.push(Wallpaper {
        path: path.to_string_lossy().into_owned(),
        name,
        source: source.to_string(),
        kind: kind.to_string(),
        ext: ext_lower(&path),
    });
}

fn collect_dir(dir: &Path, source: &str, depth: u8, out: &mut Vec<Wallpaper>) {
    if out.len() >= MAX_ITEMS || depth == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if out.len() >= MAX_ITEMS {
            return;
        }
        let path = entry.path();
        if path.is_dir() {
            collect_dir(&path, source, depth - 1, out);
        } else if is_image_ext(&path) || is_video_ext(&path) {
            push_wallpaper(path, source, out);
        }
    }
}

#[tauri::command]
pub async fn wallpaper_list() -> Result<Vec<Wallpaper>, String> {
    blocking(list_inner).await
}

/// Devuelve la imagen como `data:` URL lista para un `<img src>`.
///
/// El protocolo `asset:` de Tauri depende de que WebView2 resuelva
/// `http://asset.localhost/...`, que en la práctica falla en varias
/// configuraciones y deja las miniaturas en blanco. Leer el archivo y
/// codificarlo aquí es más lento pero siempre funciona.
#[tauri::command]
pub async fn wallpaper_thumbnail(path: String, max: Option<u32>) -> Result<String, String> {
    blocking(move || thumbnail_inner(&path, max.unwrap_or(480))).await
}

fn thumbnail_inner(path: &str, max: u32) -> Result<String, String> {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;
    use image::ImageReader;
    use std::io::Cursor;

    let source = PathBuf::from(path);
    if !source.is_file() {
        return Err("El archivo no existe".to_string());
    }
    let bytes = std::fs::read(&source).map_err(|e| format!("No se pudo leer la imagen: {e}"))?;

    let reader = ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|e| format!("Formato no reconocido: {e}"))?;
    let image = reader
        .decode()
        .map_err(|e| format!("No se pudo decodificar la imagen: {e}"))?;
    let thumb = image.thumbnail(max, max);

    let mut png = Vec::new();
    thumb
        .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| format!("No se pudo generar la miniatura: {e}"))?;
    Ok(format!("data:image/png;base64,{}", STANDARD.encode(png)))
}

fn list_inner() -> Result<Vec<Wallpaper>, String> {
    // Solo fondos propios: ni los de Windows ni los de Spotlight (ya no se
    // muestran en la interfaz, así que escanearlos solo costaba tiempo de E/S).
    let mut out = Vec::new();
    collect_dir(&custom_dir(), "propios", 3, &mut out);
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out.dedup_by(|a, b| a.path == b.path);
    Ok(out)
}

#[tauri::command]
pub async fn wallpaper_current() -> Result<Option<String>, String> {
    let mut buf = [0u16; 520];
    unsafe {
        SystemParametersInfoW(
            SPI_GETDESKWALLPAPER,
            buf.len() as u32,
            Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .map_err(|e| format!("Fondo de pantalla: {e}"))?;
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(0);
    if len == 0 {
        return Ok(None);
    }
    Ok(Some(String::from_utf16_lossy(&buf[..len])))
}

#[tauri::command]
pub async fn wallpaper_set(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let resultado = blocking(move || set_inner(&path)).await;
    // Una imagen estática reemplaza a un fondo en movimiento si lo había.
    if resultado.is_ok() && crate::commands::videowallpaper::is_active() {
        crate::commands::videowallpaper::wallpaper_video_stop(app).await?;
    }
    resultado
}

fn set_inner(path: &str) -> Result<(), String> {
    let path_buf = PathBuf::from(path);
    if !path_buf.is_file() {
        return Err("El archivo de fondo no existe".to_string());
    }
    let wide_path = wide(path);
    unsafe {
        SystemParametersInfoW(
            SPI_SETDESKWALLPAPER,
            0,
            Some(wide_path.as_ptr() as *mut core::ffi::c_void),
            SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
        )
        .map_err(|e| format!("Fondo de pantalla: {e}"))?;
    }
    Ok(())
}

fn custom_dir() -> PathBuf {
    let base = std::env::var("APPDATA")
        .map(PathBuf::from)
        .or_else(|_| std::env::var("USERPROFILE").map(|p| PathBuf::from(p).join("AppData").join("Roaming")))
        .unwrap_or_else(|_| PathBuf::from("."));
    base.join("koyun").join("fondos")
}

// ---------------------------------------------------------------------------
// Fondos por monitor
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone)]
pub struct MonitorWallpaper {
    pub id: String,
    pub index: u32,
    pub primary: bool,
    pub width: i32,
    pub height: i32,
    pub device: Option<String>,
    pub current: Option<String>,
}

/// Extrae un nombre legible del modelo a partir del id de dispositivo, que
/// tiene la forma `\\?\DISPLAY#AOC2243#4&...#{guid}`. Se devuelven también las
/// pulgadas si el id las incluye (algunos drivers añaden `#2560x1440`).
fn monitor_device_name(id: &str) -> Option<String> {
    let mut parts = id.split('#');
    parts.next()?; // prefijo \\?\DISPLAY
    let model = parts.next()?.trim();
    if model.is_empty() {
        return None;
    }
    Some(model.to_string())
}

/// Devuelve la interfaz COM `IDesktopWallpaper`, que permite fijar un fondo
/// distinto en cada monitor.
fn desktop_wallpaper() -> Result<IDesktopWallpaper, String> {
    use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
    unsafe {
        CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL)
            .map_err(|e| format!("Escritorio: {e}"))
    }
}

fn monitors_inner() -> Result<Vec<MonitorWallpaper>, String> {
    let _com = crate::commands::util::ComGuard::new();
    let dw = desktop_wallpaper()?;
    let count = unsafe { dw.GetMonitorDevicePathCount() }.map_err(|e| format!("Monitores: {e}"))?;

    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count {
        let id_ptr = unsafe { dw.GetMonitorDevicePathAt(i) }
            .map_err(|e| format!("Monitor {i}: {e}"))?;
        let id = unsafe { id_ptr.to_string() }.unwrap_or_default();
        unsafe {
            windows::Win32::System::Com::CoTaskMemFree(Some(id_ptr.0 as *const core::ffi::c_void));
        }
        if id.is_empty() {
            continue;
        }

        let id_wide = wide(&id);
        let id_pcwstr = windows::core::PCWSTR(id_wide.as_ptr());

        let (width, height) = match unsafe { dw.GetMonitorRECT(id_pcwstr) } {
            Ok(r) => (r.right - r.left, r.bottom - r.top),
            Err(_) => (0, 0),
        };

        let current = unsafe { dw.GetWallpaper(id_pcwstr) }
            .ok()
            .and_then(|p| {
                let text = unsafe { p.to_string() }.ok();
                unsafe {
                    windows::Win32::System::Com::CoTaskMemFree(Some(p.0 as *const core::ffi::c_void));
                }
                text
            })
            .filter(|s| !s.is_empty());

        out.push(MonitorWallpaper {
            id: id.clone(),
            index: i,
            // `IDesktopWallpaper` enumera en el mismo orden que Windows: el
            // índice 0 es siempre el monitor principal.
            primary: i == 0,
            width,
            height,
            device: monitor_device_name(&id),
            current,
        });
    }
    Ok(out)
}

#[tauri::command]
pub async fn wallpaper_monitors() -> Result<Vec<MonitorWallpaper>, String> {
    blocking(monitors_inner).await
}

#[tauri::command]
pub async fn wallpaper_set_monitor(monitor_id: String, path: String) -> Result<(), String> {
    blocking(move || {
        let _com = crate::commands::util::ComGuard::new();
        if !PathBuf::from(&path).is_file() {
            return Err("El archivo de fondo no existe".to_string());
        }
        let dw = desktop_wallpaper()?;
        let id_wide = wide(&monitor_id);
        let path_wide = wide(&path);
        // `SetWallpaper` mantiene la posición actual; se conserva el modo de
        // ajuste que ya tuviera el escritorio.
        unsafe {
            dw.SetWallpaper(
                windows::core::PCWSTR(id_wide.as_ptr()),
                windows::core::PCWSTR(path_wide.as_ptr()),
            )
        }
        .map_err(|e| format!("Fondo del monitor: {e}"))
    })
    .await
}

fn unique_dest(dir: &Path, name: &str) -> PathBuf {
    let original = Path::new(name);
    let stem = original
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("fondo");
    let ext = original.extension().and_then(|e| e.to_str()).unwrap_or("jpg");
    let mut candidate = dir.join(format!("{stem}.{ext}"));
    let mut n = 1;
    while candidate.exists() {
        candidate = dir.join(format!("{stem} ({n}).{ext}"));
        n += 1;
    }
    candidate
}

#[tauri::command]
pub async fn wallpaper_import(paths: Vec<String>) -> Result<Vec<Wallpaper>, String> {
    blocking(move || import_inner(paths)).await
}

fn import_inner(paths: Vec<String>) -> Result<Vec<Wallpaper>, String> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let dir = custom_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("No se pudo preparar la carpeta: {e}"))?;

    let mut imported = Vec::new();
    for src in paths {
        let src_path = PathBuf::from(&src);
        if !src_path.is_file() || !is_image_ext(&src_path) && !is_video_ext(&src_path) {
            continue;
        }
        let name = src_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("fondo.jpg");
        let dest = unique_dest(&dir, name);
        if std::fs::copy(&src_path, &dest).is_err() {
            continue;
        }
        push_wallpaper(dest, "propios", &mut imported);
    }
    Ok(imported)
}

#[tauri::command]
pub async fn wallpaper_delete(path: String) -> Result<(), String> {
    blocking(move || delete_inner(&path)).await
}

fn delete_inner(path: &str) -> Result<(), String> {
    let target = PathBuf::from(path);
    let dir = custom_dir();
    let canonical_target = target
        .canonicalize()
        .map_err(|_| "El archivo de fondo no existe".to_string())?;
    let canonical_dir = dir
        .canonicalize()
        .map_err(|_| "No se pudo localizar la carpeta de fondos propios".to_string())?;
    if !canonical_target.starts_with(&canonical_dir) {
        return Err("Solo se pueden borrar fondos propios".to_string());
    }
    std::fs::remove_file(&canonical_target).map_err(|e| format!("No se pudo borrar el fondo: {e}"))?;
    Ok(())
}
