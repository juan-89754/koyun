//! Historial de notificaciones del sistema (UserNotificationListener).
//!
//! Windows expone los avisos activos mediante `UserNotificationListener`.
//! Se solicita acceso en la primera lectura; si el usuario lo rechaza o no está
//! disponible, las consultas devuelven una lista vacía en lugar de fallar.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use windows::Foundation::DateTime;
use windows::UI::Notifications::Management::UserNotificationListener;
use windows::UI::Notifications::{NotificationKinds, UserNotification};

use crate::commands::util::{block_on, blocking};

#[derive(Serialize, Clone)]
pub struct NotificationItem {
    pub id: u32,
    pub app: String,
    pub title: String,
    pub body: String,
    pub time_ms: u64,
}

static LAST_FETCH: Mutex<Option<Instant>> = Mutex::new(None);

fn ticks_to_unix_ms(ticks: i64) -> u64 {
    // Windows DateTime: 100-nanosecond intervals since 1601-01-01.
    const EPOCH_DIFF_TICKS: i64 = 116444736000000000;
    if ticks <= EPOCH_DIFF_TICKS {
        return 0;
    }
    ((ticks - EPOCH_DIFF_TICKS) / 10_000) as u64
}

fn hstring_to_string(h: &windows::core::HSTRING) -> String {
    h.to_string()
}

fn text_of(notification: &UserNotification) -> (String, String) {
    let Ok(n) = notification.Notification() else {
        return (String::new(), String::new());
    };
    let Ok(visual) = n.Visual() else {
        return (String::new(), String::new());
    };

    // `ToastGeneric` usa bindings con textos; hay plantillas con otros nombres.
    let name = windows::core::HSTRING::from("ToastGeneric");
    let bindings = if let Ok(b) = visual.GetBinding(&name) {
        b
    } else if let Ok(_list) = visual.Bindings() {
        return (String::new(), String::new());
    } else {
        return (String::new(), String::new());
    };

    let Ok(elements) = bindings.GetTextElements() else {
        return (String::new(), String::new());
    };

    let mut texts: Vec<String> = Vec::new();
    let Ok(enumerator) = elements.First() else {
        return (String::new(), String::new());
    };
    loop {
        if !enumerator.HasCurrent().unwrap_or(false) {
            break;
        }
        if let Ok(elem) = enumerator.Current() {
            if let Ok(text) = elem.Text() {
                let s = hstring_to_string(&text);
                if !s.is_empty() {
                    texts.push(s);
                }
            }
        }
        if !enumerator.MoveNext().unwrap_or(false) {
            break;
        }
    }

    let title = texts.first().cloned().unwrap_or_default();
    let body = texts.get(1).cloned().unwrap_or_default();
    (title, body)
}

fn app_name(notification: &UserNotification) -> String {
    if let Ok(info) = notification.AppInfo() {
        if let Ok(display) = info.DisplayInfo() {
            if let Ok(name) = display.DisplayName() {
                let s = hstring_to_string(&name);
                if !s.is_empty() {
                    return s;
                }
            }
        }
    }
    "Sistema".to_string()
}

fn list_inner() -> Result<Vec<NotificationItem>, String> {
    let listener = UserNotificationListener::Current()
        .map_err(|e| format!("UserNotificationListener no disponible: {e}"))?;

    // `RequestAccessAsync` devuelve un estado; si ya se concedió devuelve «Allowed».
    let op = listener
        .RequestAccessAsync()
        .map_err(|e| format!("No se pudo solicitar acceso a notificaciones: {e}"))?;
    let status = block_on(async move { op.await })
        .map_err(|e| format!("Acceso a notificaciones: {e}"))?;

    // Allowed = 0, Denied = 1, Unspecified = 2
    if status.0 != 0 {
        return Ok(Vec::new());
    }

    let kinds = NotificationKinds::Toast;
    let op = listener
        .GetNotificationsAsync(kinds)
        .map_err(|e| format!("No se pudieron leer las notificaciones: {e}"))?;
    let list = block_on(async move { op.await })
        .map_err(|e| format!("Notificaciones: {e}"))?;

    let mut items: Vec<NotificationItem> = Vec::new();
    let Ok(enumerator) = list.First() else {
        return Ok(items);
    };
    loop {
        if !enumerator.HasCurrent().unwrap_or(false) {
            break;
        }
        if let Ok(notification) = enumerator.Current() {
            if let Ok(id) = notification.Id() {
                let app = app_name(&notification);
                let (title, body) = text_of(&notification);
                let time_ms = notification
                    .CreationTime()
                    .ok()
                    .map(|dt: DateTime| ticks_to_unix_ms(dt.UniversalTime))
                    .unwrap_or(0);
                if !title.is_empty() || !body.is_empty() {
                    items.push(NotificationItem {
                        id,
                        app,
                        title,
                        body,
                        time_ms,
                    });
                }
            }
        }
        if !enumerator.MoveNext().unwrap_or(false) {
            break;
        }
    }

    // Más recientes primero.
    items.sort_by(|a, b| b.time_ms.cmp(&a.time_ms));
    Ok(items)
}

#[tauri::command]
pub async fn notification_list() -> Result<Vec<NotificationItem>, String> {
    // Evita lecturas demasiado frecuentes para no molestar al sistema.
    {
        let mut last = LAST_FETCH.lock().unwrap();
        if let Some(t) = *last {
            if t.elapsed() < Duration::from_millis(750) {
                return Ok(Vec::new());
            }
        }
        *last = Some(Instant::now());
    }
    blocking(list_inner).await
}

#[tauri::command]
pub async fn notification_open_settings() -> Result<(), String> {
    blocking(|| {
        let _com = crate::commands::util::ComGuard::new();
        let _ = std::process::Command::new("ms-settings:notifications").spawn();
        Ok(())
    })
    .await
}
