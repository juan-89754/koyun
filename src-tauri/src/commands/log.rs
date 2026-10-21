use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// Rotación básica: por encima de este tamaño se renombra a `koyun.log.1`.
const MAX_LOG_BYTES: u64 = 512 * 1024;

fn log_path() -> &'static PathBuf {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        let base = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let dir = base.join("koyun");
        let _ = std::fs::create_dir_all(&dir);
        dir.join("koyun.log")
    })
}

/// Convierte marcas de época UTC a `YYYY-MM-DD HH:MM:SS` (algoritmo civil,
/// días desde 1970-01-01) sin depender de cron.
fn format_timestamp(unix_secs: i64) -> String {
    let days = unix_secs.div_euclid(86_400);
    let secs_of_day = unix_secs.rem_euclid(86_400);

    // Algoritmo "civil_from_days" (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };

    format!(
        "{year:04}-{m:02}-{d:02} {:02}:{:02}:{:02}",
        secs_of_day / 3600,
        (secs_of_day % 3600) / 60,
        secs_of_day % 60,
    )
}

/// Una línea por entrada: los saltos de línea del mensaje se neutralizan.
fn sanitize(message: &str) -> String {
    message
        .replace('\r', " ")
        .replace('\n', " | ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn rotate_if_needed(path: &Path) {
    let Ok(meta) = std::fs::metadata(path) else {
        return;
    };
    if meta.len() <= MAX_LOG_BYTES {
        return;
    }
    let backup = path.with_extension("log.1");
    let _ = std::fs::remove_file(&backup);
    let _ = std::fs::rename(path, backup);
}

pub fn append(level: &str, message: &str) {
    let path = log_path();
    rotate_if_needed(path);

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let line = format!(
        "{} [{level}] {}\n",
        format_timestamp(now),
        sanitize(message)
    );

    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = file.write_all(line.as_bytes());
    }
}

#[tauri::command]
pub fn log_message(level: String, message: String) -> Result<(), String> {
    let level = match level.as_str() {
        "error" => "ERROR",
        "warn" => "WARN",
        "info" => "INFO",
        other => return Err(format!("Nivel de registro desconocido: {other}")),
    };
    append(level, &message);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_epoch_is_1970() {
        assert_eq!(format_timestamp(0), "1970-01-01 00:00:00");
    }

    #[test]
    fn timestamp_known_moment() {
        // 2026-10-20 13:00:00 UTC
        assert_eq!(format_timestamp(1_792_501_200), "2026-10-20 13:00:00");
    }

    #[test]
    fn sanitize_keeps_one_line() {
        assert_eq!(sanitize("uno\ndos\r\n tres "), "uno | dos | tres");
    }
}
