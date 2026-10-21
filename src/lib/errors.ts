import { useCallback, useState } from "react";
import { logError } from "./log";

export function formatError(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  try {
    const text = JSON.stringify(error);
    if (typeof text === "string" && text !== undefined) return text;
  } catch {
    // no serializable
  }
  return String(error);
}

/**
 * Estado de error compartido por todas las pestañas (M5): un único punto de
 * entrada que además deja constancia en el registro de la aplicación.
 * `setError` se mantiene disponible para mensajes informativos controlados.
 */
export function useErrorState(scope: string) {
  const [error, setError] = useState<string | null>(null);

  const reportError = useCallback(
    (e: unknown) => {
      logError(scope, e);
      setError(formatError(e));
    },
    [scope],
  );

  return { error, setError, reportError };
}
