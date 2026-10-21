use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};
use windows::Win32::Foundation::POINT;
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

static RUNNING: AtomicBool = AtomicBool::new(false);
static VISIBLE: AtomicBool = AtomicBool::new(false);
static LAST_SHOW: Mutex<Option<Instant>> = Mutex::new(None);

#[tauri::command]
pub fn island_toggle_main(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
    Ok(())
}

fn hide_island(island: &WebviewWindow) {
    let _ = island.hide();
    VISIBLE.store(false, Ordering::SeqCst);
}

fn monitor_info_from_point(px: i32, py: i32) -> Option<MONITORINFO> {
    let point = POINT { x: px, y: py };
    let hmon = unsafe { MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST) };
    if hmon.is_invalid() {
        return None;
    }
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let ok = unsafe { GetMonitorInfoW(hmon, &mut info) };
    if !ok.as_bool() {
        None
    } else {
        Some(info)
    }
}

fn cursor_pos() -> Option<(i32, i32)> {
    let mut p = POINT { x: 0, y: 0 };
    if unsafe { GetCursorPos(&mut p) }.is_ok() {
        Some((p.x, p.y))
    } else {
        None
    }
}

/// Zona dinámica "debajo de la cámara": centrada horizontalmente en el monitor,
/// en la franja superior. Usa rcWork/rcMonitor para adaptarse a DPI/escala.
fn inside_island_zone(info: &MONITORINFO, cx: i32, cy: i32) -> bool {
    let mon_top = info.rcMonitor.top;
    let mon_bottom = info.rcMonitor.bottom;
    let mon_left = info.rcMonitor.left;
    let mon_right = info.rcMonitor.right;

    let mon_w = (mon_right - mon_left).max(1);
    let mon_h = (mon_bottom - mon_top).max(1);

    let center_x = (mon_left + mon_right) / 2;
    // Más amplio y más bajo para detectar bien debajo de la cámara
    let zone_pad = ((mon_w as f64) * 0.28).max(160.0).min(320.0) as i32;
    let z_top = mon_top + ((mon_h as f64) * 0.015).max(8.0).min(40.0) as i32;
    let z_bottom = mon_top + ((mon_h as f64) * 0.12).max(80.0).min(160.0) as i32;

    cx >= center_x - zone_pad && cx <= center_x + zone_pad && cy >= z_top && cy <= z_bottom
}

fn island_pos(info: &MONITORINFO) -> (i32, i32) {
    let mon_top = info.rcMonitor.top;
    let mon_left = info.rcMonitor.left;
    let mon_right = info.rcMonitor.right;
    let mon_bottom = info.rcMonitor.bottom;

    let mon_h = (mon_bottom - mon_top).max(1);
    let center_x = (mon_left + mon_right) / 2;
    let z_bottom = mon_top + ((mon_h as f64) * 0.08).max(60.0).min(120.0) as i32;

    let px = center_x - 28;
    let py = z_bottom - 10;
    (px, py)
}

pub fn start_island_tracker(app: AppHandle) {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }

    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_millis(50));

            let Some((cx, cy)) = cursor_pos() else { continue };
            let Some(info) = monitor_info_from_point(cx, cy) else { continue };
            let inside = inside_island_zone(&info, cx, cy);

            let Some(island) = app.get_webview_window("island") else { continue };

            if inside {
                let mut last = LAST_SHOW.lock().unwrap();
                let now = Instant::now();
                if last.is_some_and(|t| now.duration_since(t) < Duration::from_millis(160)) {
                    continue;
                }
                *last = Some(now);

                if !VISIBLE.load(Ordering::SeqCst) {
                    let (px, py) = island_pos(&info);
                    let _ = island.set_position(PhysicalPosition::new(px, py));
                    let _ = island.set_size(PhysicalSize::new(56, 56));
                    let _ = island.show();
                    VISIBLE.store(true, Ordering::SeqCst);
                }
            } else if VISIBLE.load(Ordering::SeqCst) {
                hide_island(&island);
            }
        }
    });
}
