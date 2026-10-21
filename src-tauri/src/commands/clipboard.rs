use serde::Serialize;
use windows::ApplicationModel::DataTransfer::{
    Clipboard, ClipboardHistoryItemsResultStatus, DataPackageView, SetHistoryItemAsContentStatus,
};
use windows::Storage::Streams::IRandomAccessStreamReference;
use windows::core::Interface;

use crate::commands::util::{block_on, sta_async, stream_reference_data_url, ticks_to_ms};

#[derive(Serialize)]
pub struct ClipItem {
    pub id: String,
    pub when_ms: Option<u64>,
    pub kind: String,
    pub text: Option<String>,
    pub image: Option<String>,
}

#[derive(Serialize)]
pub struct ClipState {
    pub available: bool,
    pub reason: Option<String>,
    pub items: Vec<ClipItem>,
}

const MAX_PREVIEW_CHARS: usize = 4_000;
const MAX_IMAGE_BYTES: u64 = 1_000_000;
const MAX_IMAGE_ITEMS: usize = 10;

fn truncate_chars(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    format!("{}…", value.chars().take(max).collect::<String>())
}

fn read_view(view: &DataPackageView) -> (String, Option<String>, Option<String>) {
    if let Ok(op) = view.GetTextAsync() {
        if let Ok(text) = block_on(async move { op.await }) {
            let text = text.to_string();
            if !text.trim().is_empty() {
                return ("text".to_string(), Some(text), None);
            }
        }
    }
    if let Ok(op) = view.GetBitmapAsync() {
        if let Ok(stream_ref) = block_on(async move { op.await }) {
            if let Ok(reference) = stream_ref.cast::<IRandomAccessStreamReference>() {
                return (
                    "image".to_string(),
                    None,
                    stream_reference_data_url(&reference, MAX_IMAGE_BYTES),
                );
            }
        }
    }
    (String::new(), None, None)
}

#[tauri::command]
pub async fn clipboard_history() -> Result<ClipState, String> {
    sta_async(history_inner).await
}

#[tauri::command]
pub async fn clipboard_restore(id: String) -> Result<(), String> {
    sta_async(move || restore_inner(&id)).await
}

#[tauri::command]
pub async fn clipboard_delete(id: String) -> Result<(), String> {
    sta_async(move || delete_inner(&id)).await
}

#[tauri::command]
pub async fn clipboard_clear() -> Result<(), String> {
    sta_async(clear_inner).await
}

fn history_inner() -> Result<ClipState, String> {
    let op = Clipboard::GetHistoryItemsAsync().map_err(|e| format!("Portapapeles: {e}"))?;
    let result = block_on(async move { op.await }).map_err(|e| format!("Portapapeles: {e}"))?;
    let status = result.Status().map_err(|e| format!("Portapapeles: {e}"))?;
    if status == ClipboardHistoryItemsResultStatus::ClipboardHistoryDisabled {
        return Ok(ClipState {
            available: false,
            reason: Some("disabled".to_string()),
            items: Vec::new(),
        });
    }
    if status != ClipboardHistoryItemsResultStatus::Success {
        return Ok(ClipState {
            available: false,
            reason: Some("denied".to_string()),
            items: Vec::new(),
        });
    }

    let list = result.Items().map_err(|e| format!("Portapapeles: {e}"))?;
    let n = list.Size().map_err(|e| format!("Portapapeles: {e}"))?;
    let mut items = Vec::new();
    let mut images = 0usize;
    for i in 0..n {
        let Ok(item) = list.GetAt(i) else {
            continue;
        };
        let id = item.Id().map(|h| h.to_string()).unwrap_or_default();
        let when_ms = item
            .Timestamp()
            .ok()
            .and_then(|t| ticks_to_ms(t.UniversalTime));
        let Ok(view) = item.Content() else {
            continue;
        };
        let (kind, text, image) = read_view(&view);
        if kind.is_empty() {
            continue;
        }
        // El historial completo viaja por IPC cada 3 s: sin límites, diez
        // imágenes de varios MB y entradas de texto gigantes apilaban
        // peticiones y congelaban la interfaz.
        let text = text.map(|value| truncate_chars(&value, MAX_PREVIEW_CHARS));
        let mut image = image;
        if image.is_some() {
            if images >= MAX_IMAGE_ITEMS {
                image = None;
            } else {
                images += 1;
            }
        }
        items.push(ClipItem {
            id,
            when_ms,
            kind,
            text,
            image,
        });
    }
    Ok(ClipState {
        available: true,
        reason: None,
        items,
    })
}

fn restore_inner(id: &str) -> Result<(), String> {
    let op = Clipboard::GetHistoryItemsAsync().map_err(|e| format!("Portapapeles: {e}"))?;
    let result = block_on(async move { op.await }).map_err(|e| format!("Portapapeles: {e}"))?;
    let list = result.Items().map_err(|e| format!("Portapapeles: {e}"))?;
    let n = list.Size().map_err(|e| format!("Portapapeles: {e}"))?;
    for i in 0..n {
        let Ok(item) = list.GetAt(i) else {
            continue;
        };
        if item.Id().map(|h| h.to_string()).as_deref() == Ok(id) {
            let status = Clipboard::SetHistoryItemAsContent(&item)
                .map_err(|e| format!("Portapapeles: {e}"))?;
            if status == SetHistoryItemAsContentStatus::Success {
                return Ok(());
            }
            return Err("No se pudo restaurar el elemento".to_string());
        }
    }
    Err("El elemento ya no está en el historial".to_string())
}

fn delete_inner(id: &str) -> Result<(), String> {
    let op = Clipboard::GetHistoryItemsAsync().map_err(|e| format!("Portapapeles: {e}"))?;
    let result = block_on(async move { op.await }).map_err(|e| format!("Portapapeles: {e}"))?;
    let list = result.Items().map_err(|e| format!("Portapapeles: {e}"))?;
    let n = list.Size().map_err(|e| format!("Portapapeles: {e}"))?;
    for i in 0..n {
        let Ok(item) = list.GetAt(i) else {
            continue;
        };
        if item.Id().map(|h| h.to_string()).as_deref() == Ok(id) {
            let removed = Clipboard::DeleteItemFromHistory(&item)
                .map_err(|e| format!("Portapapeles: {e}"))?;
            return if removed {
                Ok(())
            } else {
                Err("No se pudo eliminar el elemento".to_string())
            };
        }
    }
    Err("El elemento ya no está en el historial".to_string())
}

fn clear_inner() -> Result<(), String> {
    Clipboard::ClearHistory().map_err(|e| format!("Portapapeles: {e}"))?;
    Ok(())
}
