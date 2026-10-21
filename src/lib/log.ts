import { invoke } from "@tauri-apps/api/core";

export type LogLevel = "info" | "warn" | "error";

function describe(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.stack ?? error.message;
  try {
    const text = JSON.stringify(error);
    if (typeof text === "string" && text !== undefined) return text;
  } catch {
    // Circulares o no serializables: cae al texto genérico.
  }
  return String(error);
}

function toText(message: unknown): string {
  return typeof message === "string" ? message : describe(message);
}

/**
 * Registro persistente: consola de desarrollo + `%LOCALDATA%\koyun\koyun.log`
 * vía el comando `log_message`. Nunca lanza: si el IPC falla, queda solo la
 * consola (evita bucles de error al reportar errores).
 */
export function log(level: LogLevel, message: unknown): void {
  const text = toText(message);
  if (level === "error") console.error(`[koyun] ${text}`);
  else if (level === "warn") console.warn(`[koyun] ${text}`);
  else console.log(`[koyun] ${text}`);

  void invoke("log_message", { level, message: text }).catch(() => {});
}

export function logError(scope: string, error: unknown): void {
  log("error", `${scope}: ${describe(error)}`);
}

/** Errores no controlados (promesas rechazadas, excepciones globales). */
export function logUnexpected(scope: string, error: unknown): void {
  log("error", `${scope}: ${describe(error)}`);
}
