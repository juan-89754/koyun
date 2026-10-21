import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  BatterySaverIcon,
  BellOffIcon,
  BluetoothIcon,
  PlaneIcon,
  PlayIcon,
  SettingsIcon,
  SkipBackIcon,
  SkipForwardIcon,
  SpeakerIcon,
  SunIcon,
  VolumeIcon,
  VolumeXIcon,
  WifiIcon,
} from "../icons";
import { CountdownTimer } from "../../components/CountdownTimer";
import { QuickSettings, type QuickPanel } from "../../components/QuickSettings";
import { useErrorState } from "../../lib/errors";
import { usePolling } from "../../lib/usePolling";

interface VolumeState {
  level: number;
  muted: boolean;
}

interface NetworkState {
  connected: boolean;
  internet: boolean;
  name: string | null;
  ssid: string | null;
  signal: number | null;
}

interface MediaInfo {
  active: boolean;
  title: string;
  artist: string;
  album: string;
  artwork_token: string | null;
  artwork: string | null;
  artwork_changed: boolean;
  playing: boolean;
  position_ms: number;
  duration_ms: number;
  can_next: boolean;
  can_prev: boolean;
}

interface TilesState {
  wifi: boolean | null;
  bluetooth: boolean | null;
  airplane: boolean;
  sticky: boolean;
  saver: boolean;
  dnd: boolean;
}

interface AudioDevice {
  id: string;
  name: string;
  default: boolean;
}

type MediaAction = "prev" | "play_pause" | "next";

interface Tile {
  key: keyof TilesState;
  label: string;
  Icon: typeof WifiIcon;
  setter: string;
  activeClass: string;
  panel?: QuickPanel;
}

const TILES: Tile[] = [
  {
    key: "wifi",
    label: "Wi-Fi",
    Icon: WifiIcon,
    setter: "controls_wifi_set",
    activeClass: "border-sky-400/45 bg-sky-500/15 text-sky-200",
    panel: "wifi",
  },
  {
    key: "bluetooth",
    label: "Bluetooth",
    Icon: BluetoothIcon,
    setter: "controls_bluetooth_set",
    activeClass: "border-blue-400/45 bg-blue-500/15 text-blue-200",
    panel: "bluetooth",
  },
  {
    key: "airplane",
    label: "Modo avión",
    Icon: PlaneIcon,
    setter: "controls_airplane_set",
    activeClass: "border-orange-400/45 bg-orange-500/15 text-orange-200",
  },
  {
    key: "dnd",
    label: "No molestar",
    Icon: BellOffIcon,
    setter: "controls_dnd_set",
    activeClass: "border-violet-400/45 bg-violet-500/15 text-violet-200",
  },
  {
    key: "saver",
    label: "Ahorro",
    Icon: BatterySaverIcon,
    setter: "controls_saver_set",
    activeClass: "border-emerald-400/45 bg-emerald-500/15 text-emerald-200",
    panel: "power",
  },
];

function SignalBars({ signal }: { signal: number }) {
  const bars = Math.round((signal / 100) * 4);
  return (
    <div className="flex items-end gap-[3px]" aria-label={`Señal ${signal}%`}>
      {[1, 2, 3, 4].map((i) => (
        <span
          key={i}
          className={`w-1.5 rounded-sm transition-colors ${
            i <= bars ? "bg-emerald-400" : "bg-white/15"
          }`}
          style={{ height: `${6 + i * 4}px` }}
        />
      ))}
    </div>
  );
}

function formatClock(ms: number) {
  const total = Math.max(0, Math.round(ms / 1000));
  const m = Math.floor(total / 60);
  const s = total % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

export function ControlsTab() {
  const [volume, setVolume] = useState<VolumeState | null>(null);
  const [network, setNetwork] = useState<NetworkState | null>(null);
  const [media, setMedia] = useState<MediaInfo | null>(null);
  const [mediaPos, setMediaPos] = useState(0);
  const [tiles, setTiles] = useState<TilesState | null>(null);
  const [brightness, setBrightness] = useState<number | null>(null);
  const [brightnessSupported, setBrightnessSupported] = useState(true);
  const [devices, setDevices] = useState<AudioDevice[]>([]);
  const [mediaBusy, setMediaBusy] = useState(false);
  const [quickPanel, setQuickPanel] = useState<QuickPanel | null>(null);
  const { error, reportError } = useErrorState("controles");
  const volumeCommit = useRef<number | null>(null);
  const brightnessCommit = useRef<number | null>(null);
  // Mientras se arrastra el volumen, el sondeo no debe pisar el valor local.
  const editingVolume = useRef(false);

  // El backend solo envía la carátula cuando cambia (artwork_changed); entre
  // sondeos la conservamos aquí para no retransmitir hasta ~6 MB cada 2,5 s.
  const artworkRef = useRef<string | null>(null);

  const loadMedia = useCallback(() => {
    invoke<MediaInfo>("media_info")
      .then((m) => {
        if (m.artwork_changed) artworkRef.current = m.artwork;
        setMedia({ ...m, artwork: artworkRef.current });
        setMediaPos(m.position_ms);
      })
      .catch(() => {});
  }, []);

  const loadTiles = useCallback(() => {
    invoke<TilesState>("controls_tiles_state")
      .then(setTiles)
      .catch(() => {});
  }, []);

  const loadNetwork = useCallback(() => {
    invoke<NetworkState>("controls_network")
      .then(setNetwork)
      .catch(() => {});
  }, []);

  // Sondeos con guardia de vuelta y pausa con la ventana oculta.
  usePolling(loadNetwork, 5000);
  usePolling(loadMedia, 2500);
  usePolling(loadTiles, 8000);
  usePolling(() => {
    if (editingVolume.current) return;
    invoke<VolumeState>("controls_volume_get")
      .then(setVolume)
      .catch(() => {});
  }, 1500);

  useEffect(() => {
    let cancelled = false;

    invoke<VolumeState>("controls_volume_get")
      .then((v) => {
        if (!cancelled) setVolume(v);
      })
      .catch((e) => {
        if (!cancelled) reportError(e);
      });

    invoke<number | null>("controls_brightness_get")
      .then((b) => {
        if (cancelled) return;
        if (b === null) {
          setBrightnessSupported(false);
        } else {
          setBrightness(b);
        }
      })
      .catch(() => {
        if (!cancelled) setBrightnessSupported(false);
      });

    invoke<AudioDevice[]>("audio_output_devices")
      .then((d) => {
        if (!cancelled) setDevices(d);
      })
      .catch(() => {});

    // Reloj de reproducción: no hace IPC, solo avanza la posición local.
    const clockId = window.setInterval(() => {
      if (document.hidden) return;
      setMediaPos((p) => p + 1000);
    }, 1000);

    return () => {
      cancelled = true;
      window.clearInterval(clockId);
      if (volumeCommit.current !== null) window.clearTimeout(volumeCommit.current);
      if (brightnessCommit.current !== null) window.clearTimeout(brightnessCommit.current);
    };
  }, [reportError]);

  const setLevel = useCallback((level: number) => {
    editingVolume.current = true;
    setVolume((v) => (v ? { ...v, level } : v));
    if (volumeCommit.current !== null) window.clearTimeout(volumeCommit.current);
    volumeCommit.current = window.setTimeout(() => {
      volumeCommit.current = null;
      invoke("controls_volume_set", { level })
        .catch(reportError)
        .finally(() => {
          editingVolume.current = false;
        });
    }, 120);
  }, [reportError]);

  const toggleMute = useCallback(async () => {
    if (!volume) return;
    const muted = !volume.muted;
    setVolume({ ...volume, muted });
    try {
      await invoke("controls_volume_set_mute", { muted });
    } catch (e) {
      reportError(e);
    }
  }, [reportError, volume]);

  const sendMedia = useCallback(async (action: MediaAction) => {
    if (mediaBusy) return;
    setMediaBusy(true);
    setMedia((m) => (m ? { ...m, playing: action === "play_pause" ? !m.playing : m.playing } : m));
    try {
      await invoke("media_action", { action });
    } catch (e) {
      reportError(e);
    } finally {
      window.setTimeout(() => {
        setMediaBusy(false);
        loadMedia();
      }, 300);
    }
  }, [loadMedia, mediaBusy, reportError]);

  const toggleTile = useCallback(async (tile: Tile) => {
    if (!tiles) return;
    const current = tiles[tile.key];
    const next = !(current === true);
    setTiles({ ...tiles, [tile.key]: next });
    try {
      await invoke(tile.setter, { on: next });
    } catch (e) {
      reportError(e);
    }
    window.setTimeout(loadTiles, 700);
  }, [loadTiles, reportError, tiles]);

  const setBright = useCallback((level: number) => {
    setBrightness(level);
    if (brightnessCommit.current !== null) window.clearTimeout(brightnessCommit.current);
    brightnessCommit.current = window.setTimeout(() => {
      brightnessCommit.current = null;
      invoke("controls_brightness_set", { level }).catch(reportError);
    }, 150);
  }, [reportError]);

  const selectDevice = useCallback(async (id: string) => {
    try {
      await invoke("audio_set_output_device", { id });
      const d = await invoke<AudioDevice[]>("audio_output_devices");
      setDevices(d);
    } catch (e) {
      reportError(e);
    }
  }, [reportError]);

  const percent = volume ? Math.round(volume.level * 100) : null;
  const muted = volume?.muted ?? false;
  const duration = media?.duration_ms ?? 0;
  const progress = duration > 0 ? Math.min(100, (mediaPos / duration) * 100) : 0;
  const defaultDevice = devices.find((d) => d.default);

  return (
    <div
      id="panel-controls"
      role="tabpanel"
      aria-labelledby="tab-controls"
      className="relative flex h-full flex-col gap-4 px-8 py-5"
    >
      <div className="grid shrink-0 grid-cols-5 gap-4">
        <section className="col-span-3 flex flex-col rounded-xl border border-white/10 bg-white/5 p-5">
          <div className="mb-5 flex items-center justify-between">
            <div className="flex items-center gap-2.5 text-sky-300">
              <VolumeIcon className="h-4.5 w-4.5" />
              <span className="text-sm font-medium text-slate-200">Volumen</span>
            </div>
            <span className="text-xs font-semibold tabular-nums text-white/50">
              {percent !== null ? `${percent}%` : "—"}
            </span>
          </div>

          <div className="flex items-center gap-4">
            <button
              type="button"
              aria-pressed={muted}
              aria-label={muted ? "Activar sonido" : "Silenciar"}
              onClick={() => void toggleMute()}
              className={`flex h-11 w-11 shrink-0 items-center justify-center rounded-full border transition-all duration-150 hover:scale-[1.04] active:scale-95 ${
                muted
                  ? "border-red-400/40 bg-red-500/20 text-red-200"
                  : "border-white/10 bg-white/5 text-slate-300 hover:border-sky-400/40 hover:bg-sky-500/15 hover:text-sky-200"
              }`}
            >
              {muted ? <VolumeXIcon className="h-5 w-5" /> : <VolumeIcon className="h-5 w-5" />}
            </button>
            <input
              type="range"
              min={0}
              max={100}
              step={1}
              value={percent ?? 0}
              onChange={(e) => setLevel(Number(e.target.value) / 100)}
              disabled={volume === null}
              aria-label="Nivel de volumen"
              className="h-2 w-full cursor-pointer accent-sky-400 disabled:opacity-40"
            />
          </div>

          <div className="mt-auto flex items-center gap-3 pt-4">
            <SpeakerIcon className="h-4 w-4 shrink-0 text-white/40" />
            <select
              value={defaultDevice?.id ?? ""}
              onChange={(e) => void selectDevice(e.target.value)}
              disabled={devices.length === 0}
              aria-label="Salida de audio"
              className="min-w-0 flex-1 truncate rounded-lg border border-white/10 bg-white/5 px-2.5 py-1.5 text-xs text-slate-200 outline-none transition-colors hover:border-white/20 focus:border-sky-400/50 disabled:opacity-40 [&>option]:bg-zinc-900"
            >
              {devices.length === 0 && <option value="">Sin dispositivos</option>}
              {devices.map((d) => (
                <option key={d.id} value={d.id}>
                  {d.name}
                </option>
              ))}
            </select>
          </div>
        </section>

        <section className="col-span-2 flex flex-col rounded-xl border border-white/10 bg-white/5 p-5">
          <div className="mb-4 flex items-center gap-2.5 text-violet-300">
            <PlayIcon className="h-4.5 w-4.5" />
            <span className="text-sm font-medium text-slate-200">Reproducción</span>
          </div>

          <div className="flex min-w-0 flex-1 items-center gap-3">
            {media?.artwork ? (
              <img
                src={media.artwork}
                alt=""
                className="h-14 w-14 shrink-0 rounded-lg border border-white/10 object-cover"
              />
            ) : (
              <div className="flex h-14 w-14 shrink-0 items-center justify-center rounded-lg border border-white/10 bg-white/5 text-white/25">
                <PlayIcon className="h-5 w-5" />
              </div>
            )}
            <div className="min-w-0 flex-1">
              <p className="truncate text-sm font-medium text-slate-100">
                {media?.active ? media.title || "Sin título" : "Sin reproducción activa"}
              </p>
              <p className="truncate text-xs text-white/45">
                {media?.active
                  ? media.artist || media.album || ""
                  : "Abre un reproductor para controlarlo"}
              </p>
            </div>
          </div>

          <div className="mt-3">
            <div className="h-1.5 w-full overflow-hidden rounded-full bg-white/10">
              <div
                className="h-full rounded-full bg-violet-400/80 transition-[width] duration-500"
                style={{ width: `${progress}%` }}
              />
            </div>
            <div className="mt-1 flex justify-between text-[10px] tabular-nums text-white/40">
              <span>{media?.active ? formatClock(mediaPos) : "0:00"}</span>
              <span>{media?.active ? formatClock(duration) : "0:00"}</span>
            </div>
          </div>

          <div className="mt-2 flex items-center justify-center gap-3">
            <button
              type="button"
              aria-label="Anterior"
              disabled={mediaBusy || media?.can_prev === false}
              onClick={() => void sendMedia("prev")}
              className="flex h-9 w-9 items-center justify-center rounded-full border border-white/10 bg-white/5 text-slate-300 transition-all duration-150 hover:scale-[1.04] hover:border-violet-400/40 hover:bg-violet-500/15 hover:text-violet-200 active:scale-95 disabled:opacity-40"
            >
              <SkipBackIcon className="h-4 w-4" />
            </button>
            <button
              type="button"
              aria-label={media?.playing ? "Pausar" : "Reproducir"}
              disabled={mediaBusy || !media?.active}
              onClick={() => void sendMedia("play_pause")}
              className="flex h-11 w-11 items-center justify-center rounded-full border border-white/10 bg-white/5 text-slate-200 transition-all duration-150 hover:scale-[1.06] hover:border-violet-400/50 hover:bg-violet-500/20 hover:text-white active:scale-95 disabled:opacity-40"
            >
              {media?.playing ? (
                <span className="flex gap-[3px]">
                  <span className="h-4 w-[3px] rounded-sm bg-current" />
                  <span className="h-4 w-[3px] rounded-sm bg-current" />
                </span>
              ) : (
                <PlayIcon className="h-5 w-5" />
              )}
            </button>
            <button
              type="button"
              aria-label="Siguiente"
              disabled={mediaBusy || media?.can_next === false}
              onClick={() => void sendMedia("next")}
              className="flex h-9 w-9 items-center justify-center rounded-full border border-white/10 bg-white/5 text-slate-300 transition-all duration-150 hover:scale-[1.04] hover:border-violet-400/40 hover:bg-violet-500/15 hover:text-violet-200 active:scale-95 disabled:opacity-40"
            >
              <SkipForwardIcon className="h-4 w-4" />
            </button>
          </div>
        </section>
      </div>

      <section className="shrink-0 rounded-xl border border-white/10 bg-white/5 px-5 py-4">
        <p className="mb-3 text-xs font-medium tracking-wide text-white/50">
          Azulejos rápidos
        </p>
        <div className="grid grid-cols-5 gap-2">
          {TILES.map((tile) => {
            const value = tiles ? tiles[tile.key] : null;
            const on = value === true;
            const unknown = value === null && (tile.key === "wifi" || tile.key === "bluetooth");
            const isOpen = tile.panel !== undefined && quickPanel === tile.panel;
            return (
              <div key={tile.key} className="group relative">
                <button
                  type="button"
                  aria-pressed={on}
                  disabled={tiles === null}
                  onClick={() => void toggleTile(tile)}
                  className={`flex w-full flex-col items-center gap-2 rounded-lg border px-2 py-3 text-[11px] transition-all duration-150 hover:scale-[1.03] disabled:opacity-40 ${
                    on
                      ? tile.activeClass
                      : unknown
                        ? "border-white/8 bg-white/3 text-white/35"
                        : "border-white/10 bg-white/5 text-slate-300 hover:border-white/20 hover:text-white"
                  }`}
                >
                  <tile.Icon className="h-4.5 w-4.5" />
                  <span className="font-medium">{tile.label}</span>
                </button>
                {tile.panel && (
                  <button
                    type="button"
                    aria-label={`Ajustes de ${tile.label}`}
                    title={`Ajustes de ${tile.label}`}
                    onClick={() => setQuickPanel(isOpen ? null : tile.panel!)}
                    className={`absolute right-1 top-1 rounded-md p-1 transition-all duration-150 ${
                      isOpen
                        ? "bg-white/15 text-white opacity-100"
                        : "text-white/40 opacity-0 hover:bg-white/15 hover:text-white group-hover:opacity-100"
                    }`}
                  >
                    <SettingsIcon className="h-3.5 w-3.5" />
                  </button>
                )}
              </div>
            );
          })}
        </div>
      </section>

      <div className="grid shrink-0 grid-cols-5 gap-4">
        <section className="col-span-3 flex flex-col justify-center rounded-xl border border-white/10 bg-white/5 px-5 py-4">
          <div className="mb-3 flex items-center justify-between">
            <div className="flex items-center gap-2.5 text-amber-300">
              <SunIcon className="h-4.5 w-4.5" />
              <span className="text-sm font-medium text-slate-200">Brillo</span>
            </div>
            <span className="text-xs font-semibold tabular-nums text-white/50">
              {brightnessSupported ? (brightness !== null ? `${brightness}%` : "—") : "No disponible"}
            </span>
          </div>
          <input
            type="range"
            min={0}
            max={100}
            step={1}
            value={brightness ?? 0}
            onChange={(e) => setBright(Number(e.target.value))}
            disabled={!brightnessSupported}
            aria-label="Nivel de brillo"
            className="h-2 w-full cursor-pointer accent-amber-400 disabled:opacity-40"
          />
        </section>

        <section className="col-span-2 flex items-center gap-3 rounded-xl border border-white/10 bg-white/5 px-4 py-4">
          <div
            className={`flex h-10 w-10 shrink-0 items-center justify-center rounded-xl border transition-colors ${
              network?.connected
                ? "border-emerald-400/40 bg-emerald-500/15 text-emerald-300"
                : "border-white/10 bg-white/5 text-slate-500"
            }`}
          >
            <WifiIcon className="h-5 w-5" />
          </div>
          <div className="min-w-0 flex-1">
            <p className="truncate text-sm font-medium text-slate-200">
              {network
                ? (network.ssid ??
                  network.name ??
                  (network.connected ? "Red conectada" : "Sin conexión"))
                : "Consultando red…"}
            </p>
            <p className="truncate text-xs text-white/45">
              {!network
                ? ""
                : !network.connected
                  ? "Sin conexión a la red"
                  : network.internet
                    ? "Con acceso a Internet"
                    : "Sin acceso a Internet"}
            </p>
          </div>
          {network?.connected && network.signal !== null ? (
            <SignalBars signal={network.signal} />
          ) : null}
        </section>
      </div>

      <CountdownTimer />
      {error && <p role="alert" className="text-center text-xs leading-5 text-red-300/90">{error}</p>}

      <QuickSettings panel={quickPanel} onClose={() => setQuickPanel(null)} />
    </div>
  );
}
