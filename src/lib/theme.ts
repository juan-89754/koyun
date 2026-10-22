//! Tema visual del panel.
//!
//! El backend expone la configuración (`config_get` / `config_set`) y la
//! detección del sistema (`theme_system`). Aquí se combina todo:
//!
//! - `mode` decide si se sigue al sistema (`auto`), o se fuerza claro/oscuro.
//! - `accentMode` decide si el acento es el de Windows (`system`) o uno
//!   personalizado (`custom`).
//! - `preset` aplica una paleta predefinida de acento.
//!
//! El resultado se escribe en `document.documentElement` como `data-theme`
//! (`light` | `dark`), `data-preset` y `--koyun-accent`, que son las variables
//! que consume `styles.css`.

import { invoke } from "@tauri-apps/api/core";

export type ThemeMode = "auto" | "light" | "dark";
export type AccentMode = "system" | "custom";

export interface ThemeConfig {
  mode: ThemeMode;
  accentMode: AccentMode;
  accentColor: string;
  preset: string;
}

export interface SystemTheme {
  light: boolean;
  accent: string;
  accent_available: boolean;
}

export const ACCENT_PRESETS: readonly { id: string; label: string; color: string }[] = [
  { id: "", label: "Predeterminado", color: "#2dd4bf" },
  { id: "midnight", label: "Medianoche", color: "#60a5fa" },
  { id: "cyberpunk", label: "Ciberpunk", color: "#f472b6" },
  { id: "minimal", label: "Minimal", color: "#94a3b8" },
  { id: "ocean", label: "Océano", color: "#38bdf8" },
  { id: "sunset", label: "Atardecer", color: "#fb923c" },
];

const DEFAULT_THEME: ThemeConfig = {
  mode: "auto",
  accentMode: "system",
  accentColor: "#2dd4bf",
  preset: "",
};

/** Normaliza un color hexadecimal; devuelve `null` si no es válido. */
export function normalizeHex(value: string): string | null {
  const raw = value.trim();
  const hex = raw.startsWith("#") ? raw.slice(1) : raw;
  if (!/^[0-9a-fA-F]{6}$/.test(hex)) return null;
  return `#${hex.toLowerCase()}`;
}

/**
 * Calcula el acento que se aplica realmente: la paleta predefinida manda
 * sobre el color personalizado, y el del sistema si no hay ninguno.
 */
export function resolveAccent(
  config: ThemeConfig,
  system: SystemTheme | null,
): string {
  if (config.preset) {
    const preset = ACCENT_PRESETS.find((p) => p.id === config.preset);
    if (preset) return preset.color;
  }
  if (config.accentMode === "system") {
    if (system?.accent_available && system.accent) return system.accent;
    return DEFAULT_THEME.accentColor;
  }
  return normalizeHex(config.accentColor) ?? DEFAULT_THEME.accentColor;
}

/** Aplica el tema al documento. */
export function applyTheme(config: ThemeConfig, system: SystemTheme | null): void {
  const root = document.documentElement;
  const resolved = config.mode === "auto" ? (system?.light ? "light" : "dark") : config.mode;
  root.setAttribute("data-theme", resolved);
  if (config.preset) {
    root.setAttribute("data-preset", config.preset);
  } else {
    root.removeAttribute("data-preset");
  }
  root.style.setProperty("--koyun-accent", resolveAccent(config, system));
}

/** Lee el tema del sistema operativo. */
export async function loadSystemTheme(): Promise<SystemTheme | null> {
  try {
    return await invoke<SystemTheme>("theme_system");
  } catch {
    return null;
  }
}

export { DEFAULT_THEME };
