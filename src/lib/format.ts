export function relativeTime(whenMs: number | null): string {
  if (whenMs === null) return "";
  const diff = Date.now() - whenMs;
  if (diff < 60_000) return "hace un momento";
  const minutes = Math.floor(diff / 60_000);
  if (minutes < 60) return `hace ${minutes} min`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `hace ${hours} h`;
  const days = Math.floor(hours / 24);
  return `hace ${days} d`;
}

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.min(units.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)));
  const value = bytes / 1024 ** i;
  return `${value >= 100 || i === 0 ? Math.round(value) : value.toFixed(1)} ${units[i]}`;
}

export function formatBps(bps: number | null): string {
  if (bps === null || !Number.isFinite(bps)) return "—";
  return `${formatBytes(bps)}/s`;
}

export function formatMbps(bps: number): string {
  return `${(bps / 1_000_000).toFixed(1)} Mbps`;
}

export function formatUptime(ms: number): string {
  const totalMinutes = Math.floor(ms / 60_000);
  const days = Math.floor(totalMinutes / 1440);
  const hours = Math.floor((totalMinutes % 1440) / 60);
  const minutes = totalMinutes % 60;
  if (days > 0) return `${days} d ${hours} h ${minutes} min`;
  if (hours > 0) return `${hours} h ${minutes} min`;
  return `${minutes} min`;
}

export function ratio(part: number, total: number): number {
  if (total <= 0) return 0;
  return Math.min(100, Math.max(0, (part / total) * 100));
}

/** Etiqueta compacta de duración en segundos (`45 s`, `5 min`, `2 h`). */
export function durationLabel(sec: number): string {
  if (sec < 60) return `${sec} s`;
  if (sec < 3600) return `${sec / 60} min`;
  return `${sec / 3600} h`;
}

/** Etiqueta de intervalo del carrusel (misma escala que `durationLabel`). */
export function intervalLabel(sec: number): string {
  return durationLabel(sec);
}

/** Contador `m:ss` / `h:mm:ss` del temporizador. */
export function countdownLabel(totalSec: number): string {
  const total = Math.max(0, Math.ceil(totalSec));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  if (h > 0) {
    return `${h}:${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
  }
  return `${m}:${s.toString().padStart(2, "0")}`;
}
