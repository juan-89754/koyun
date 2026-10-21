import { useCallback, useEffect } from "react";
import { PauseIcon, PlayIcon, RotateCcwIcon, TimerIcon } from "./icons";
import { countdownLabel, durationLabel } from "../lib/format";
import { TIMER_PRESETS, useSettingsStore } from "../store/settings";

/**
 * Cuenta atrás. El estado vive en la store (no en el componente) para que la
 * cuenta siga avanzando aunque el usuario cambie de pestaña.
 */
export function CountdownTimer() {
  const durationSec = useSettingsStore((s) => s.timer.durationSec);
  const setTimer = useSettingsStore((s) => s.setTimer);
  const { remainingSec: remaining, running, finished } = useSettingsStore(
    (s) => s.timerRun,
  );
  const setTimerRun = useSettingsStore((s) => s.setTimerRun);

  useEffect(() => {
    if (!running) return;
    const id = window.setInterval(() => {
      setTimerRun((prev) =>
        prev.remainingSec <= 1
          ? { remainingSec: 0, running: false, finished: true }
          : { remainingSec: prev.remainingSec - 1 },
      );
    }, 1000);
    return () => window.clearInterval(id);
  }, [running, setTimerRun]);

  const start = useCallback(() => {
    if (finished || remaining <= 0) {
      setTimerRun({ remainingSec: durationSec, finished: false, running: true });
      return;
    }
    setTimerRun({ running: !running });
  }, [durationSec, finished, remaining, running, setTimerRun]);

  const reset = useCallback(() => {
    setTimerRun({ running: false, finished: false, remainingSec: durationSec });
  }, [durationSec, setTimerRun]);

  const pickDuration = useCallback(
    (sec: number) => {
      setTimer({ durationSec: sec });
      setTimerRun({ remainingSec: sec, running: false, finished: false });
    },
    [setTimer, setTimerRun],
  );

  return (
    <section className="shrink-0 rounded-xl border border-white/10 bg-white/5 px-5 py-3">
      <div className="flex items-center gap-3">
        <div
          className={`flex items-center gap-2 ${
            finished ? "text-red-300" : running ? "text-cyan-300" : "text-white/45"
          }`}
        >
          <TimerIcon className="h-4 w-4" />
          <span className="text-sm font-medium text-slate-200">Temporizador</span>
        </div>

        <span
          className={`ml-auto text-lg font-semibold tabular-nums ${
            finished ? "text-red-300" : "text-slate-100"
          }`}
        >
          {finished ? "¡Tiempo!" : countdownLabel(remaining)}
        </span>

        <button
          type="button"
          onClick={start}
          aria-label={running ? "Pausar temporizador" : "Iniciar temporizador"}
          className={`flex h-9 w-9 shrink-0 items-center justify-center rounded-full border transition-all duration-150 hover:scale-[1.05] active:scale-95 ${
            running
              ? "border-cyan-400/50 bg-cyan-500/20 text-cyan-200"
              : "border-white/10 bg-white/5 text-slate-300 hover:border-cyan-400/40 hover:bg-cyan-500/15 hover:text-cyan-200"
          }`}
        >
          {running ? <PauseIcon className="h-4 w-4" /> : <PlayIcon className="h-4 w-4" />}
        </button>
        <button
          type="button"
          onClick={reset}
          aria-label="Reiniciar temporizador"
          className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full border border-white/10 bg-white/5 text-slate-300 transition-all duration-150 hover:scale-[1.05] hover:border-white/20 hover:text-white active:scale-95"
        >
          <RotateCcwIcon className="h-4 w-4" />
        </button>
      </div>

      <div className="mt-3 flex flex-wrap gap-1.5">
        {TIMER_PRESETS.map((sec) => (
          <button
            key={sec}
            type="button"
            onClick={() => pickDuration(sec)}
            className={`rounded-md border px-2.5 py-1 text-[11px] transition-colors ${
              durationSec === sec
                ? "border-cyan-400/45 bg-cyan-500/15 text-cyan-200"
                : "border-white/10 bg-white/5 text-slate-300 hover:border-white/20 hover:text-white"
            }`}
          >
            {durationLabel(sec)}
          </button>
        ))}
      </div>
    </section>
  );
}
