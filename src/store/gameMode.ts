import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";

export interface GameModeStatus {
  enabled: boolean;
  target_pid?: number | null;
  target_exe?: string | null;
  wifi_autoconfig_was_enabled?: boolean | null;
  wifi_interface?: string | null;
}

interface GameModeState {
  enabled: boolean;
  targetPid: number | null;
  targetExe: string | null;
  limitWifi: boolean;
  loading: boolean;
  error: string | null;
  status: () => Promise<void>;
  useForeground: () => Promise<void>;
  enable: () => Promise<void>;
  disable: () => Promise<void>;
}

export const useGameModeStore = create<GameModeState>((set, get) => ({
  enabled: false,
  targetPid: null,
  targetExe: null,
  limitWifi: true,
  loading: false,
  error: null,
  status: async () => {
    try {
      set({ loading: true, error: null });
      const s = await invoke<GameModeStatus>("game_mode_status");
      set({
        enabled: !!s.enabled,
        targetPid: s.target_pid ?? null,
        targetExe: s.target_exe ?? null,
        loading: false,
      });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },
  useForeground: async () => {
    try {
      set({ loading: true, error: null });
      const s = await invoke<GameModeStatus>("game_mode_use_foreground");
      set({
        targetPid: s.target_pid ?? null,
        targetExe: s.target_exe ?? null,
        loading: false,
      });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },
  enable: async () => {
    try {
      set({ loading: true, error: null });
      const s = await invoke<GameModeStatus>("game_mode_enable", { limitWifi: get().limitWifi });
      set({
        enabled: !!s.enabled,
        targetPid: s.target_pid ?? null,
        targetExe: s.target_exe ?? null,
        loading: false,
      });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },
  disable: async () => {
    try {
      set({ loading: true, error: null });
      const s = await invoke<GameModeStatus>("game_mode_disable");
      set({
        enabled: !!s.enabled,
        targetPid: s.target_pid ?? null,
        targetExe: s.target_exe ?? null,
        loading: false,
      });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },
}));
