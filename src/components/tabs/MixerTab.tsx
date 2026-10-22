import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import { VolumeIcon, VolumeXIcon } from "../icons";
import { useErrorState } from "../../lib/errors";

interface AudioSession {
  pid: number;
  name: string;
  process: string;
  volume: number;
  muted: boolean;
}

interface MicState {
  muted: boolean;
  level: number;
  name: string;
}

/** Silueta de la aplicación a partir del nombre del proceso. */
function appMonogram(name: string): string {
  const clean = name.replace(/\.exe$/i, "").trim();
  if (!clean) return "?";
  const words = clean.split(/[\s._-]+/).filter(Boolean);
  if (words.length >= 2) {
    return (words[0][0] + words[1][0]).toUpperCase();
  }
  return clean.slice(0, 2).toUpperCase();
}

export function MixerTab() {
  const [sessions, setSessions] = useState<AudioSession[] | null>(null);
  const [mic, setMic] = useState<MicState | null>(null);
  const [busy, setBusy] = useState<number | null>(null);
  const { error, setError, reportError } = useErrorState("mezclador");

  // Mientras se arrastra un deslizador no debe pisarse el valor local.
  const editing = useRef<number | null>(null);

  const load = useCallback(async () => {
    try {
      const [list, micState] = await Promise.all([
        invoke<AudioSession[]>("audio_sessions"),
        invoke<MicState>("audio_mic_state"),
      ]);
      if (editing.current === null) setSessions(list);
      setMic(micState);
    } catch (e) {
      reportError(e);
    }
  }, [reportError]);

  useEffect(() => {
    void load();
    const timer = window.setInterval(() => void load(), 2000);
    return () => window.clearInterval(timer);
  }, [load]);

  const setVolume = useCallback(
    async (session: AudioSession, level: number) => {
      editing.current = session.pid;
      setSessions((prev) => (prev ?? []).map((s) => (s.pid === session.pid ? { ...s, volume: level } : s)));
      try {
        await invoke("audio_session_set_volume", { pid: session.pid, level });
      } catch (e) {
        reportError(e);
      } finally {
        editing.current = null;
      }
    },
    [reportError],
  );

  const toggleMute = useCallback(
    async (session: AudioSession) => {
      setBusy(session.pid);
      setError(null);
      const muted = !session.muted;
      setSessions((prev) => (prev ?? []).map((s) => (s.pid === session.pid ? { ...s, muted } : s)));
      try {
        await invoke("audio_session_set_mute", { pid: session.pid, muted });
      } catch (e) {
        reportError(e);
        await load();
      } finally {
        setBusy(null);
      }
    },
    [load, reportError, setError],
  );

  const toggleMic = useCallback(async () => {
    setError(null);
    const muted = !(mic?.muted ?? false);
    setMic((prev) => (prev ? { ...prev, muted } : prev));
    try {
      await invoke("audio_mic_set_mute", { muted });
      await load();
    } catch (e) {
      reportError(e);
      await load();
    }
  }, [load, mic?.muted, reportError, setError]);

  return (
    <div
      id="panel-mixer"
      role="tabpanel"
      aria-labelledby="tab-mixer"
      className="flex h-full flex-col gap-4 px-8 py-5"
    >
      <section className="shrink-0 rounded-xl border border-white/10 bg-white/5 p-5">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2.5 text-sky-300">
            <VolumeIcon className="h-4.5 w-4.5" />
            <span className="text-sm font-medium text-slate-200">Micrófono</span>
          </div>
          <button
            type="button"
            aria-pressed={mic?.muted ?? false}
            onClick={() => void toggleMic()}
            className={`flex items-center gap-2 rounded-lg border px-2.5 py-1.5 text-[11px] font-medium transition-colors ${
              mic?.muted
                ? "border-red-400/40 bg-red-500/15 text-red-200"
                : "border-white/10 bg-white/5 text-slate-300 hover:border-emerald-400/40 hover:text-emerald-200"
            }`}
          >
            {mic?.muted ? <VolumeXIcon className="h-3.5 w-3.5" /> : <VolumeIcon className="h-3.5 w-3.5" />}
            {mic?.muted ? "Silenciado" : "Activo"}
          </button>
        </div>
        <p className="mt-1.5 truncate text-[11px] text-white/40">
          {mic?.name ?? "Consultando dispositivos…"}
        </p>
      </section>

      <section className="flex min-h-0 flex-1 flex-col rounded-xl border border-white/10 bg-white/5 p-5">
        <div className="mb-4 flex items-center justify-between">
          <div className="flex items-center gap-2.5 text-violet-300">
            <VolumeIcon className="h-4.5 w-4.5" />
            <span className="text-sm font-medium text-slate-200">Aplicaciones</span>
          </div>
          <span className="text-[11px] text-white/40">
            {sessions === null ? "Buscando…" : `${sessions.length} con sonido`}
          </span>
        </div>

        <div className="koyun-scroll min-h-0 flex-1 overflow-y-auto">
          {sessions === null && (
            <p className="py-4 text-center text-xs text-white/40">
              Consultando las sesiones de audio…
            </p>
          )}
          {sessions !== null && sessions.length === 0 && (
            <p className="py-4 text-center text-xs text-white/40">
              Ninguna aplicación está reproduciendo sonido ahora mismo.
            </p>
          )}
          <ul className="flex flex-col gap-1.5">
            {(sessions ?? []).map((session) => (
              <li
                key={session.pid}
                className="flex items-center gap-3 rounded-lg border border-white/8 bg-white/4 px-3 py-2.5"
              >
                <span
                  aria-hidden
                  className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-white/8 text-[11px] font-semibold text-slate-300"
                >
                  {appMonogram(session.process)}
                </span>
                <div className="min-w-0 flex-1">
                  <p className="truncate text-xs font-medium text-slate-200">{session.name}</p>
                  <p className="truncate text-[10px] text-white/40">{session.process}</p>
                </div>
                <input
                  type="range"
                  min={0}
                  max={100}
                  step={1}
                  value={Math.round(session.volume * 100)}
                  onChange={(e) =>
                    void setVolume(session, Number(e.target.value) / 100)
                  }
                  aria-label={`Volumen de ${session.name}`}
                  className="h-1.5 w-32 shrink-0 cursor-pointer disabled:opacity-40"
                />
                <span className="w-9 shrink-0 text-right text-[11px] tabular-nums text-white/50">
                  {Math.round(session.volume * 100)}%
                </span>
                <button
                  type="button"
                  aria-label={session.muted ? `Activar ${session.name}` : `Silenciar ${session.name}`}
                  aria-pressed={session.muted}
                  disabled={busy === session.pid}
                  onClick={() => void toggleMute(session)}
                  className={`flex h-7 w-7 shrink-0 items-center justify-center rounded-md border transition-colors disabled:opacity-40 ${
                    session.muted
                      ? "border-red-400/40 bg-red-500/15 text-red-200"
                      : "border-white/10 text-white/50 hover:border-white/25 hover:text-white"
                  }`}
                >
                  {session.muted ? (
                    <VolumeXIcon className="h-3.5 w-3.5" />
                  ) : (
                    <VolumeIcon className="h-3.5 w-3.5" />
                  )}
                </button>
              </li>
            ))}
          </ul>
        </div>
      </section>

      {error && <p className="shrink-0 text-center text-xs text-red-300/90">{error}</p>}
    </div>
  );
}
