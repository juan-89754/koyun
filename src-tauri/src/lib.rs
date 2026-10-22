use std::sync::atomic::{AtomicBool, Ordering};

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;

mod commands;

/// Estado propio de visibilidad: respaldo cuando la consulta de
/// `window.is_visible()` falla. La fuente de verdad sigue siendo la ventana.
static VISIBLE: AtomicBool = AtomicBool::new(false);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Todo pánico queda registrado en %LOCALDATA%\koyun\koyun.log (rotado
    // por tamaño) además de en stderr, para poder diagnosticar fallos
    // en equipos sin depurador.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        commands::log::append("PANIC", &info.to_string());
        default_hook(info);
    }));

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        // Alt+F4 (o el botón de cierre) oculta el panel en lugar de terminar
        // la aplicación: la salida real es "Salir" en el icono de la bandeja.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
                VISIBLE.store(false, Ordering::SeqCst);
            }
        })
        .setup(|app| {
            use tauri_plugin_dialog::DialogExt;

            if let Some(window) = app.get_webview_window("main") {
                VISIBLE.store(window.is_visible().unwrap_or(false), Ordering::SeqCst);
            }

            // Carga configuración y registra atajos configurables
            if let Err(err) = commands::config::init(app.handle()) {
                app.dialog()
                    .message(format!("No se pudo cargar la configuración: {err}"))
                    .title("Koyun")
                    .show(|_| {});
            } else if let Err(err) = commands::hotkeys::apply(app.handle()) {
                app.dialog()
                    .message(format!("No se pudieron registrar los atajos globales: {err}"))
                    .title("Koyun")
                    .show(|_| {});
            }

            // Posiciona la ventana en la parte superior central de la pantalla primaria
            if let Some(window) = app.get_webview_window("main") {
                if let (Ok(Some(monitor)), Ok(outer), Ok(scale)) = (
                    window.primary_monitor(),
                    window.outer_size(),
                    window.scale_factor(),
                ) {
                    let monitor_logical_w = monitor.size().width as f64 / scale;
                    let window_logical_w = outer.width as f64 / scale;
                    let x = (monitor_logical_w - window_logical_w) / 2.0;
                    let _ = window.set_position(tauri::LogicalPosition::new(x, 8.0));
                }
            }

            // Redondeo nativo de Windows (DWM): esquinas redondeadas sin
            // transparencia, así no aparece el marco blanco que provocaba
            // `transparent: true` en el WebView2.
            if let Some(window) = app.get_webview_window("main") {
                if let Ok(hwnd) = window.hwnd() {
                    use windows::Win32::Foundation::HWND;
                    use windows::Win32::Graphics::Dwm::{
                        DWMWA_WINDOW_CORNER_PREFERENCE, DWM_WINDOW_CORNER_PREFERENCE,
                        DWMWCP_ROUND, DwmSetWindowAttribute,
                    };
                    let hwnd = HWND(hwnd.0);
                    let pref = DWMWCP_ROUND;
                    unsafe {
                        let _ = DwmSetWindowAttribute(
                            hwnd,
                            DWMWA_WINDOW_CORNER_PREFERENCE,
                            &pref as *const _ as *const core::ffi::c_void,
                            std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
                        );
                    }
                }
            }

            // Icono en la bandeja del sistema
            let show_item = MenuItem::with_id(app, "show", "Mostrar", true, None::<&str>)?;
            let hide_item = MenuItem::with_id(app, "hide", "Ocultar", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Salir", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &hide_item, &quit_item])?;
            let mut builder = TrayIconBuilder::with_id("koyun")
                .menu(&menu)
                .tooltip("Koyun")
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.unminimize();
                            let _ = window.show();
                            let _ = window.set_focus();
                            VISIBLE.store(true, Ordering::SeqCst);
                        }
                    }
                    "hide" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.hide();
                            VISIBLE.store(false, Ordering::SeqCst);
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            // Fuente de verdad: visibilidad real de la ventana.
                            let visible = window
                                .is_visible()
                                .unwrap_or_else(|_| VISIBLE.load(Ordering::SeqCst));
                            if visible {
                                let _ = window.hide();
                                VISIBLE.store(false, Ordering::SeqCst);
                            } else {
                                let _ = window.unminimize();
                                let _ = window.show();
                                let _ = window.set_focus();
                                VISIBLE.store(true, Ordering::SeqCst);
                            }
                        }
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                builder = builder.icon(icon.clone());
            }
            if let Err(err) = builder.build(app) {
                app.dialog()
                    .message(format!(
                        "No se pudo crear el icono de la bandeja del sistema: {err}\n\n\
                         Koyun seguirá abierto y el atajo Ctrl+Alt+K seguirá activo."
                    ))
                    .title("Koyun")
                    .show(|_| {});
            }

            // Isla dinámica: sin este hilo la ventana "island" nunca se muestra,
            // porque solo se creó oculta en tauri.conf.json.
            commands::island::start_island_tracker(app.handle().clone());

            // Reanuda un fondo en movimiento pendiente de la sesión anterior.
            commands::videowallpaper::restore(app.handle());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::config::config_get,
            commands::config::config_set,
            commands::theme::theme_system,
            commands::hotkeys::hotkeys_list,
            commands::hotkeys::hotkeys_apply,
            commands::window::show_main_window,
            commands::window::hide_main_window,
            commands::window::toggle_main_window,
            commands::power::power_lock,
            commands::power::power_suspend,
            commands::power::power_restart,
            commands::power::power_shutdown,
            commands::audio::controls_volume_get,
            commands::audio::controls_volume_set,
            commands::audio::controls_volume_set_mute,
            commands::audio::audio_output_devices,
            commands::audio::audio_set_output_device,
            commands::audio::audio_sessions,
            commands::audio::audio_session_set_volume,
            commands::audio::audio_session_set_mute,
            commands::audio::audio_mic_state,
            commands::audio::audio_mic_set_mute,
            commands::media::media_info,
            commands::media::media_action,
            commands::stats::stats_snapshot,
            commands::clipboard::clipboard_history,
            commands::clipboard::clipboard_restore,
            commands::clipboard::clipboard_delete,
            commands::clipboard::clipboard_clear,
            commands::wallpaper::wallpaper_list,
            commands::wallpaper::wallpaper_thumbnail,
            commands::wallpaper::wallpaper_monitors,
            commands::wallpaper::wallpaper_set_monitor,
            commands::wallpaper::wallpaper_current,
            commands::wallpaper::wallpaper_set,
            commands::wallpaper::wallpaper_import,
            commands::wallpaper::wallpaper_delete,
            commands::videowallpaper::wallpaper_video_start,
            commands::videowallpaper::wallpaper_video_stop,
            commands::videowallpaper::wallpaper_video_state,
            commands::controls::controls_network,
            commands::controls::controls_tiles_state,
            commands::controls::controls_wifi_set,
            commands::controls::controls_bluetooth_set,
            commands::controls::controls_airplane_set,
            commands::controls::controls_sticky_set,
            commands::controls::controls_saver_set,
            commands::controls::controls_dnd_set,
            commands::controls::controls_brightness_get,
            commands::controls::controls_brightness_set,
            commands::quick::quick_network_detail,
            commands::quick::quick_wifi_networks,
            commands::quick::quick_wifi_connect,
            commands::quick::quick_wifi_disconnect,
            commands::quick::quick_bluetooth_devices,
            commands::quick::quick_bluetooth_pair,
            commands::quick::quick_bluetooth_unpair,
            commands::quick::quick_battery,
            commands::quick::quick_power_plans,
            commands::quick::quick_power_plan_set,
            commands::log::log_message,
            commands::island::island_toggle_main,
            commands::notification::notification_list,
            commands::notification::notification_open_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
