use std::sync::Mutex;

use serde::Serialize;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession,
    GlobalSystemMediaTransportControlsSessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus,
};
use windows::Storage::Streams::IRandomAccessStreamReference;
use windows::core::Result as WinResult;

use crate::commands::util::{ComGuard, block_on, blocking, stream_reference_data_url};

#[derive(Serialize)]
pub struct MediaInfo {
    pub active: bool,
    pub title: String,
    pub artist: String,
    pub album: String,
    /// Identidad estable del arte actual (título + álbum).
    pub artwork_token: Option<String>,
    /// `Some` solo cuando cambia respecto al sondeo anterior (`artwork_changed`):
    /// si se envía en cada sondeo se retransmiten ~8 MB por IPC cada 2,5 s.
    pub artwork: Option<String>,
    pub artwork_changed: bool,
    pub playing: bool,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub can_next: bool,
    pub can_prev: bool,
}

fn inactive() -> MediaInfo {
    MediaInfo {
        active: false,
        title: String::new(),
        artist: String::new(),
        album: String::new(),
        artwork_token: None,
        artwork: None,
        artwork_changed: true,
        playing: false,
        position_ms: 0,
        duration_ms: 0,
        can_next: false,
        can_prev: false,
    }
}

fn current_session() -> WinResult<GlobalSystemMediaTransportControlsSession> {
    let manager_op = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()?;
    let manager = block_on(async move { manager_op.await })?;
    manager.GetCurrentSession()
}

fn artwork_data_url(stream_ref: &IRandomAccessStreamReference) -> Option<String> {
    stream_reference_data_url(stream_ref, 6_000_000)
}

/// Clave identitaria del arte: título + álbum (constante durante una misma
/// reproducción).
fn artwork_key(title: &str, album: &str) -> Option<String> {
    if title.is_empty() && album.is_empty() {
        None
    } else {
        Some(format!("{title}\u{1}{album}"))
    }
}

static ARTWORK: Mutex<Option<(String, String)>> = Mutex::new(None);

/// Devuelve el arte **solo cuando cambia** respecto al sondeo anterior. En los
/// sondeos siguientes devuelve `(None, false)` y la tarjeta conserva la imagen,
/// evitando re-leer y re-enviar hasta 6 MB cada 2,5 s.
fn cached_artwork(
    token: &str,
    thumb: &IRandomAccessStreamReference,
) -> (Option<String>, bool) {
    let mut cache = ARTWORK.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((prev, url)) = cache.as_ref()
        && prev == token
        && !url.is_empty()
    {
        return (None, false);
    }
    match artwork_data_url(thumb) {
        Some(url) => {
            *cache = Some((token.to_string(), url.clone()));
            (Some(url), true)
        }
        // Fallo transitorio: no se cachea, se reintentará en el próximo sondeo.
        None => (None, true),
    }
}

fn media_info_impl() -> Result<MediaInfo, String> {
    let _com = ComGuard::new();
    let session = match current_session() {
        Ok(session) => session,
        Err(_) => return Ok(inactive()),
    };

    let playback = session.GetPlaybackInfo().ok();
    let playing = playback
        .as_ref()
        .and_then(|p| p.PlaybackStatus().ok())
        .map(|s| s == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing)
        .unwrap_or(false);
    let controls = playback.as_ref().and_then(|p| p.Controls().ok());
    let can_next = controls
        .as_ref()
        .and_then(|c| c.IsNextEnabled().ok())
        .unwrap_or(false);
    let can_prev = controls
        .as_ref()
        .and_then(|c| c.IsPreviousEnabled().ok())
        .unwrap_or(false);

    let mut title = String::new();
    let mut artist = String::new();
    let mut album = String::new();
    let mut artwork_token = None;
    let mut artwork = None;
    let mut artwork_changed = true;
    if let Ok(props_op) = session.TryGetMediaPropertiesAsync() {
        if let Ok(props) = block_on(async move { props_op.await }) {
            title = props.Title().ok().map(|h| h.to_string()).unwrap_or_default();
            artist = props.Artist().ok().map(|h| h.to_string()).unwrap_or_default();
            album = props
                .AlbumTitle()
                .ok()
                .map(|h| h.to_string())
                .unwrap_or_default();
            if let Ok(thumb) = props.Thumbnail() {
                artwork_token = artwork_key(&title, &album);
                if let Some(token) = artwork_token.as_deref() {
                    (artwork, artwork_changed) = cached_artwork(token, &thumb);
                }
            }
        }
    }

    let mut position_ms = 0u64;
    let mut duration_ms = 0u64;
    if let Ok(timeline) = session.GetTimelineProperties() {
        if let Ok(pos) = timeline.Position() {
            position_ms = (pos.Duration.max(0) as u64) / 10_000;
        }
        if let Ok(end) = timeline.EndTime() {
            duration_ms = (end.Duration.max(0) as u64) / 10_000;
        }
    }

    Ok(MediaInfo {
        active: true,
        title,
        artist,
        album,
        artwork_token,
        artwork,
        artwork_changed,
        playing,
        position_ms,
        duration_ms,
        can_next,
        can_prev,
    })
}

#[tauri::command]
pub async fn media_info() -> Result<MediaInfo, String> {
    blocking(media_info_impl).await
}

#[tauri::command]
pub async fn media_action(action: String) -> Result<(), String> {
    blocking(move || media_action_impl(&action)).await
}

/// Versión síncrona para usar desde atajos globales (sin pasar por el runtime async).
pub fn media_action_sync(action: &str) -> Result<(), String> {
    media_action_impl(action)
}

fn media_action_impl(action: &str) -> Result<(), String> {
    let _com = ComGuard::new();
    let session =
        current_session().map_err(|_| "No hay ninguna reproducción activa".to_string())?;
    let op = match action {
        "prev" => session.TrySkipPreviousAsync(),
        "next" => session.TrySkipNextAsync(),
        "play_pause" => session.TryTogglePlayPauseAsync(),
        "play" => session.TryPlayAsync(),
        "pause" => session.TryPauseAsync(),
        other => return Err(format!("Acción desconocida: {other}")),
    }
    .map_err(|e| format!("Multimedia: {e}"))?;
    let accepted = block_on(async move { op.await }).map_err(|e| format!("Multimedia: {e}"))?;
    if accepted {
        Ok(())
    } else {
        Err("La aplicación rechazó la acción".to_string())
    }
}
