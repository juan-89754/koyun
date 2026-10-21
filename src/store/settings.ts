import { create } from "zustand";
import { persist } from "zustand/middleware";

export interface CarouselConfig {
  enabled: boolean;
  intervalSec: number;
  source: string;
}

export interface TimerConfig {
  durationSec: number;
}

/** Estado en ejecución del temporizador (cuenta atrás). Vive en la store para
 * que no se pierda al cambiar de pestaña (`App` recrea el panel con `key`). */
export interface TimerRun {
  remainingSec: number;
  running: boolean;
  finished: boolean;
}

interface SettingsState {
  carousel: CarouselConfig;
  timer: TimerConfig;
  timerRun: TimerRun;
  setCarousel: (patch: Partial<CarouselConfig>) => void;
  setTimer: (patch: Partial<TimerConfig>) => void;
  setTimerRun: (patch: Partial<TimerRun> | ((prev: TimerRun) => Partial<TimerRun>)) => void;
}

export const CAROUSEL_INTERVALS = [15, 30, 60, 300, 900, 1800, 3600] as const;
export const TIMER_PRESETS = [60, 300, 600, 900, 1800, 3600] as const;

export const useSettingsStore = create<SettingsState>()(
  persist(
    (set) => ({
      carousel: { enabled: false, intervalSec: 300, source: "all" },
      timer: { durationSec: 300 },
      timerRun: { remainingSec: 300, running: false, finished: false },
      setCarousel: (patch) =>
        set((state) => ({ carousel: { ...state.carousel, ...patch } })),
      setTimer: (patch) => set((state) => ({ timer: { ...state.timer, ...patch } })),
      setTimerRun: (patch) =>
        set((state) => ({
          timerRun: {
            ...state.timerRun,
            ...(typeof patch === "function" ? patch(state.timerRun) : patch),
          },
        })),
    }),
    {
      name: "koyun-settings",
      // Solo la configuración se persiste: el estado en marcha del
      // temporizador es efímero y debe arrancar desde la duración elegida.
      partialize: (state) => ({ carousel: state.carousel, timer: state.timer }),
    },
  ),
);
