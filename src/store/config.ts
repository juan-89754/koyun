//! Configuración de la aplicación, respaldida por el backend.
//!
//! El archivo `%APPDATA%\koyun\config.json` es la fuente de verdad: el
//! frontend lo lee con `config_get` y lo actualiza con `config_set`, que hace
//! un *merge* profundo en el servidor y vuelve a registrar los atajos.

import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import {
  applyTheme,
  loadSystemTheme,
  type SystemTheme,
  type ThemeConfig,
} from "../lib/theme";

export interface HotkeyConfig {
  toggle_panel: string;
  mute_mic: string;
  media_play_pause: string;
  media_next: string;
  media_prev: string;
  toggle_dnd: string;
  show_clipboard: string;
  lock_screen: string;
}

export interface VideoConfig {
  pause_on_fullscreen: boolean;
  pause_on_battery: boolean;
  always_muted: boolean;
}

export interface IslandConfig {
  widget: string;
  stats_threshold: number;
}

export interface NotificationsConfig {
  enabled: boolean;
  keep: number;
}

export interface UpdaterConfig {
  auto_check: boolean;
  notify: boolean;
}

export interface AppConfig {
  theme: ThemeConfig;
  hotkeys: HotkeyConfig;
  video: VideoConfig;
  island: IslandConfig;
  notifications: NotificationsConfig;
  updater: UpdaterConfig;
  extra: Record<string, unknown>;
}

export interface HotkeyBinding {
  action: string;
  label: string;
  accelerator: string;
  registered: boolean;
  error: string | null;
}

interface ConfigState {
  config: AppConfig | null;
  systemTheme: SystemTheme | null;
  hotkeys: HotkeyBinding[];
  loading: boolean;
  error: string | null;
  load: () => Promise<void>;
  patch: (patch: Record<string, unknown>) => Promise<void>;
  refreshHotkeys: () => Promise<void>;
}

const EMPTY: AppConfig = {
  theme: {
    mode: "auto",
    accentMode: "system",
    accentColor: "#2dd4bf",
    preset: "",
  },
  hotkeys: {
    toggle_panel: "Ctrl+Alt+K",
    mute_mic: "Ctrl+Alt+M",
    media_play_pause: "Ctrl+Alt+P",
    media_next: "Ctrl+Alt+Right",
    media_prev: "Ctrl+Alt+Left",
    toggle_dnd: "",
    show_clipboard: "Ctrl+Alt+V",
    lock_screen: "",
  },
  video: {
    pause_on_fullscreen: true,
    pause_on_battery: true,
    always_muted: true,
  },
  island: { widget: "auto", stats_threshold: 80 },
  notifications: { enabled: true, keep: 50 },
  updater: { auto_check: true, notify: true },
  extra: {},
};

export const useConfigStore = create<ConfigState>()((set, get) => ({
  config: null,
  systemTheme: null,
  hotkeys: [],
  loading: true,
  error: null,

  load: async () => {
    try {
      const [config, systemTheme] = await Promise.all([
        invoke<AppConfig>("config_get"),
        loadSystemTheme(),
      ]);
      set({ config, systemTheme, loading: false, error: null });
      applyTheme(config.theme, systemTheme);
      await get().refreshHotkeys();
    } catch (e) {
      set({ loading: false, error: String(e) });
    }
  },

  patch: async (patch) => {
    try {
      const config = await invoke<AppConfig>("config_set", { patch });
      set({ config, error: null });
      applyTheme(config.theme, get().systemTheme);
      await get().refreshHotkeys();
    } catch (e) {
      set({ error: String(e) });
    }
  },

  refreshHotkeys: async () => {
    try {
      const hotkeys = await invoke<HotkeyBinding[]>("hotkeys_list");
      set({ hotkeys });
    } catch {
      // Si la lista falla, los ajustes de atajos simplemente no se muestran.
    }
  },
}));

export { EMPTY as DEFAULT_CONFIG };
