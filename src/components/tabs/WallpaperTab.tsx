import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { CarouselIcon, MonitorIcon, TrashIcon, UploadIcon, WallpaperIcon } from "../icons";
import { intervalLabel } from "../../lib/format";
import { useErrorState } from "../../lib/errors";
import { CAROUSEL_INTERVALS, useSettingsStore } from "../../store/settings";

interface Wallpaper {
  path: string;
  name: string;
  source: string;
  kind: string;
  ext: string;
}

interface MonitorWallpaper {
  id: string;
  index: number;
  primary: boolean;
  width: number;
  height: number;
  device: string | null;
  current: string | null;
}

const SOURCES: { id: string; label: string }[] = [
  { id: "all", label: "Todos" },
  { id: "propios", label: "Propios" },
  { id: "animados", label: "En movimiento" },
];

function isSamePath(a: string, b: string) {
  return a.replace(/\//g, "\\").toLowerCase() === b.replace(/\//g, "\\").toLowerCase();
}

/// Caché de miniaturas: el comando devuelve un `data:` URL (no depende del
/// protocolo `asset:`, que dejaba las imágenes en blanco). Se codifica una sola
/// vez por ruta y tamaño.
const thumbCache = new Map<string, Promise<string>>();

function loadThumbnail(path: string, max: number): Promise<string> {
  const key = `${max}:${path}`;
  const cached = thumbCache.get(key);
  if (cached) return cached;
  const request = invoke<string>("wallpaper_thumbnail", { path, max }).catch((e) => {
    thumbCache.delete(key);
    throw e;
  });
  thumbCache.set(key, request);
  return request;
}

function Thumbnail({ path, className }: { path: string; className?: string }) {
  const [src, setSrc] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    loadThumbnail(path, 480)
      .then((url) => {
        if (!cancelled) setSrc(url);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [path]);

  if (!src) return <div className={`animate-pulse bg-white/5 ${className ?? ""}`} />;
  return <img src={src} alt="" loading="lazy" className={className} />;
}

export function WallpaperTab() {
  const [items, setItems] = useState<Wallpaper[] | null>(null);
  const [current, setCurrent] = useState<string | null>(null);
  const [video, setVideo] = useState<string | null>(null);
  const [filter, setFilter] = useState("propios");
  const [busyPath, setBusyPath] = useState<string | null>(null);
  const [importing, setImporting] = useState(false);
  const [monitors, setMonitors] = useState<MonitorWallpaper[]>([]);
  // "todos" aplica el fondo a la vez; un id concreto lo fija en ese monitor.
  const [target, setTarget] = useState<string>("todos");
  const { error, setError, reportError } = useErrorState("fondos");

  const carousel = useSettingsStore((s) => s.carousel);
  const setCarousel = useSettingsStore((s) => s.setCarousel);

  const reload = useCallback(async () => {
    const list = await invoke<Wallpaper[]>("wallpaper_list");
    setItems(list);
    return list;
  }, []);

  const loadMonitors = useCallback(async () => {
    try {
      const list = await invoke<MonitorWallpaper[]>("wallpaper_monitors");
      setMonitors(list);
    } catch {
      setMonitors([]);
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    invoke<Wallpaper[]>("wallpaper_list")
      .then((list) => {
        if (!cancelled) setItems(list);
      })
      .catch((e) => {
        if (!cancelled) reportError(e);
      });
    invoke<string | null>("wallpaper_current")
      .then((path) => {
        if (!cancelled) setCurrent(path);
      })
      .catch(() => {});
    invoke<string | null>("wallpaper_video_state")
      .then((path) => {
        if (!cancelled) setVideo(path);
      })
      .catch(() => {});
    void loadMonitors();
    return () => {
      cancelled = true;
    };
  }, [reportError, loadMonitors]);

  const visible = useMemo(() => {
    if (!items) return [];
    if (filter === "all") return items.filter((item) => item.source === "propios");
    if (filter === "animados") return items.filter((item) => item.source === "propios" && item.kind === "video");
    return items.filter((item) => item.source === filter);
  }, [filter, items]);

  // Con un vídeo en movimiento no hay imagen estática "actual".
  const activePath = video ?? current;
  const currentMatch = useMemo(
    () => (activePath ? items?.find((item) => isSamePath(item.path, activePath)) : undefined),
    [activePath, items],
  );

  const apply = useCallback(
    async (wallpaper: Wallpaper) => {
      setBusyPath(wallpaper.path);
      setError(null);
      try {
        // Con un monitor concreto, la imagen se fija solo ahí (Windows lo
        // permite con `IDesktopWallpaper`). Los vídeos y el modo «todos» siguen
        // usando el fondo global.
        if (wallpaper.kind === "image" && target !== "todos") {
          await invoke("wallpaper_set_monitor", {
            monitorId: target,
            path: wallpaper.path,
          });
          await loadMonitors();
          return;
        }

        if (wallpaper.kind === "video") {
          await invoke("wallpaper_video_start", { path: wallpaper.path });
          setVideo(wallpaper.path);
          return;
        }
        await invoke("wallpaper_set", { path: wallpaper.path });
        setVideo(null);
        setCurrent(wallpaper.path);
        // Fijar el fondo global pisa el de cada monitor: se refresca.
        void loadMonitors();
      } catch (e) {
        reportError(e);
      } finally {
        setBusyPath(null);
      }
    },
    [loadMonitors, reportError, setError, target],
  );

  // Rotación automática: avanza por la lista visible según el intervalo elegido.
  // Los efectos deben ser puros: el updater de `setCurrent` solo devuelve el
  // nuevo valor y la llamada a `wallpaper_set` se lanza fuera (en React
  // StrictMode los updaters se ejecutan dos veces por render).
  const rotationRef = useRef<Wallpaper[]>([]);
  rotationRef.current = visible;
  const currentRef = useRef<string | null>(null);
  currentRef.current = current;

  useEffect(() => {
    // Un vídeo en movimiento tiene prioridad: la rotación lo apagaría.
    if (video) return;
    if (!carousel.enabled || visible.length < 2) return;
    const id = window.setInterval(() => {
      const list = rotationRef.current;
      if (list.length < 2) return;
      const prev = currentRef.current;
      const idx = prev ? list.findIndex((item) => isSamePath(item.path, prev)) : -1;
      const next = list[(idx + 1) % list.length];
      if (prev && isSamePath(prev, next.path)) return;
      setCurrent(next.path);
      void invoke("wallpaper_set", { path: next.path }).catch(reportError);
    }, carousel.intervalSec * 1000);
    return () => window.clearInterval(id);
  }, [carousel.enabled, carousel.intervalSec, visible.length, reportError, video]);

  const importWallpapers = useCallback(async () => {
    if (importing) return;
    setError(null);
    try {
      const selection = await open({
        multiple: true,
        filters: [
          {
            name: "Imágenes y vídeos",
            extensions: ["jpg", "jpeg", "png", "bmp", "webp", "tif", "tiff", "heic", "mp4", "webm", "mkv", "avi"],
          },
        ],
      });
      if (!selection) return;
      const paths = Array.isArray(selection) ? selection : [selection];
      if (paths.length === 0) return;
      setImporting(true);
      await invoke("wallpaper_import", { paths });
      await reload();
      setFilter("propios");
    } catch (e) {
      reportError(e);
    } finally {
      setImporting(false);
    }
  }, [importing, reload, reportError]);

  const removeWallpaper = useCallback(
    async (wallpaper: Wallpaper) => {
      setError(null);
      try {
        await invoke("wallpaper_delete", { path: wallpaper.path });
        await reload();
      } catch (e) {
        reportError(e);
      }
    },
    [reload, reportError],
  );

  return (
    <div
      id="panel-wallpaper"
      role="tabpanel"
      aria-labelledby="tab-wallpaper"
      className="flex h-full flex-col gap-3 px-8 py-5"
    >
      <div className="flex shrink-0 items-center gap-3">
        <WallpaperIcon className="h-4 w-4 text-cyan-300" />
        <span className="text-sm font-medium text-slate-200">Fondo de pantalla</span>
        {items && (
          <span className="rounded-full border border-white/12 bg-white/6 px-2 py-0.5 text-[10px] font-medium text-white/55">
            {visible.length}
          </span>
        )}
        <button
          type="button"
          onClick={() => void importWallpapers()}
          disabled={importing}
          className="ml-auto flex items-center gap-1.5 rounded-md border border-white/10 bg-white/5 px-2.5 py-1 text-[11px] text-slate-300 transition-colors hover:border-cyan-400/40 hover:bg-cyan-500/15 hover:text-cyan-200 disabled:opacity-40"
        >
          <UploadIcon className="h-3.5 w-3.5" />
          {importing ? "Importando…" : "Importar"}
        </button>
      </div>

      <div className="flex shrink-0 flex-wrap items-center gap-1">
        {SOURCES.map((source) => (
          <button
            key={source.id}
            type="button"
            onClick={() => setFilter(source.id)}
            className={`rounded-md px-2.5 py-1 text-[11px] transition-colors ${
              filter === source.id
                ? "bg-white/12 text-white"
                : "text-white/50 hover:bg-white/6 hover:text-white/85"
            }`}
          >
            {source.label}
          </button>
        ))}
      </div>

      <section className="flex shrink-0 items-center gap-3 rounded-xl border border-white/10 bg-white/5 px-4 py-3">
        {(() => {
          // Con un monitor concreto se muestra su fondo real; en «todos», el
          // fondo global (imagen o vídeo en movimiento).
          const selected =
            target !== "todos" ? monitors.find((mon) => mon.id === target) : undefined;
          const perMonitor = selected?.current ?? null;
          const previewPath = perMonitor ?? (selected ? null : currentMatch?.path ?? null);
          const label = selected
            ? `${selected.device ?? `Pantalla ${selected.index + 1}`} · ${selected.width}×${selected.height}`
            : null;
          const name = previewPath
            ? previewPath.split(/[\\/]/).pop()
            : selected
              ? "Sin fondo propio en esta pantalla"
              : currentMatch?.name ??
                (video ? "Vídeo en movimiento" : (current ?? "Sin fondo de pantalla establecido"));

          return (
            <>
              {previewPath ? (
                <Thumbnail
                  path={previewPath}
                  className="h-12 w-20 shrink-0 rounded-lg border border-white/10 object-cover"
                />
              ) : currentMatch?.kind === "video" && !selected ? (
                <video
                  src={convertFileSrc(currentMatch.path)}
                  muted
                  loop
                  playsInline
                  autoPlay
                  className="h-12 w-20 shrink-0 rounded-lg border border-white/10 object-cover"
                />
              ) : (
                <div className="flex h-12 w-20 shrink-0 items-center justify-center rounded-lg border border-white/10 bg-white/5 text-white/30">
                  <WallpaperIcon className="h-4 w-4" />
                </div>
              )}
              <div className="min-w-0 flex-1">
                <p className="text-[10px] font-medium tracking-wide text-white/45">
                  {selected ? "Fondo de la pantalla" : "Fondo actual"}
                </p>
                <p className="truncate text-xs text-slate-200">{name}</p>
                {label && <p className="truncate text-[10px] text-white/40">{label}</p>}
              </div>
            </>
          );
        })()}
        {monitors.length > 1 && (
          <label className="flex shrink-0 items-center gap-2 text-[11px] text-white/50">
            <MonitorIcon className="h-4 w-4 text-white/40" />
            <select
              value={target}
              onChange={(e) => setTarget(e.target.value)}
              aria-label="Monitor de destino"
              className="rounded-lg border border-white/10 bg-white/5 px-2 py-1 text-xs text-slate-200 outline-none transition-colors hover:border-white/20 focus:border-cyan-400/50 [&>option]:bg-zinc-900"
            >
              <option value="todos">Todos los monitores</option>
              {monitors.map((mon) => (
                <option key={mon.id} value={mon.id}>
                  Pantalla {mon.index + 1}
                  {mon.primary ? " (principal)" : ""}
                  {mon.device ? ` · ${mon.device}` : ""} · {mon.width}×{mon.height}
                </option>
              ))}
            </select>
          </label>
        )}
      </section>

      <section className="flex shrink-0 flex-wrap items-center gap-3 rounded-xl border border-white/10 bg-white/5 px-4 py-3">
        <div
          className={`flex items-center gap-2 ${
            carousel.enabled ? "text-cyan-300" : "text-white/45"
          }`}
        >
          <CarouselIcon className="h-4 w-4" />
          <span className="text-xs font-medium text-slate-200">Rotación automática</span>
        </div>

        <button
          type="button"
          role="switch"
          aria-checked={carousel.enabled}
          aria-label="Activar rotación automática"
          disabled={visible.length < 2}
          onClick={() => setCarousel({ enabled: !carousel.enabled })}
          className={`relative h-5 w-9 shrink-0 rounded-full border transition-colors disabled:opacity-40 ${
            carousel.enabled
              ? "border-cyan-400/50 bg-cyan-500/40"
              : "border-white/15 bg-white/8"
          }`}
        >
          <span
            className={`absolute top-0.5 h-3.5 w-3.5 rounded-full bg-white transition-all ${
              carousel.enabled ? "left-[18px]" : "left-0.5"
            }`}
          />
        </button>

        <label className="ml-auto flex items-center gap-2 text-[11px] text-white/50">
          Cada
          <select
            value={carousel.intervalSec}
            onChange={(e) => setCarousel({ intervalSec: Number(e.target.value) })}
            aria-label="Intervalo de rotación"
            className="rounded-lg border border-white/10 bg-white/5 px-2 py-1 text-xs text-slate-200 outline-none transition-colors hover:border-white/20 focus:border-cyan-400/50 [&>option]:bg-zinc-900"
          >
            {CAROUSEL_INTERVALS.map((sec) => (
              <option key={sec} value={sec}>
                {intervalLabel(sec)}
              </option>
            ))}
          </select>
        </label>
      </section>

      <div className="koyun-scroll grid min-h-0 flex-1 grid-cols-4 content-start gap-2 overflow-y-auto pr-1">
        {items === null && (
          <p className="col-span-4 pt-8 text-center text-xs text-white/40">Buscando fondos…</p>
        )}
        {items !== null && visible.length === 0 && (
          <p className="col-span-4 pt-8 text-center text-xs text-white/40">
            No se han encontrado fondos en esta categoría.
          </p>
        )}
        {visible.map((wallpaper) => {
          const isCurrent = activePath ? isSamePath(wallpaper.path, activePath) : false;
          const isBusy = busyPath === wallpaper.path;
          // En qué pantallas está puesto este fondo (cada monitor guarda su
          // ruta actual, así que se compara contra todas).
          const onMonitors = monitors.filter(
            (mon) => mon.current && isSamePath(mon.current, wallpaper.path),
          );
          return (
            <div key={wallpaper.path} className="group relative">
              <button
                type="button"
                onClick={() => void apply(wallpaper)}
                disabled={busyPath !== null}
                title={wallpaper.path}
                className={`relative aspect-video w-full overflow-hidden rounded-lg border transition-all duration-150 hover:scale-[1.02] disabled:cursor-not-allowed ${
                  isCurrent || onMonitors.length > 0
                    ? "border-cyan-400/70 ring-1 ring-cyan-400/40"
                    : "border-white/10 hover:border-white/30"
                } ${isBusy ? "opacity-50" : ""}`}
              >
                {wallpaper.kind === "video" ? (
                  <video
                    src={convertFileSrc(wallpaper.path)}
                    muted
                    loop
                    playsInline
                    className="h-full w-full object-cover"
                    onMouseEnter={(e) => e.currentTarget.play().catch(() => {})}
                    onMouseLeave={(e) => {
                      e.currentTarget.pause();
                      e.currentTarget.currentTime = 0;
                    }}
                  />
                ) : (
                  <Thumbnail
                    path={wallpaper.path}
                    className="h-full w-full object-cover"
                  />
                )}
                {wallpaper.kind === "video" && (
                  <span className="absolute left-1.5 top-1.5 rounded-full bg-black/60 px-1.5 py-0.5 text-[9px] font-medium text-white/80">
                    Vídeo
                  </span>
                )}
                <span className="absolute inset-x-0 bottom-0 truncate bg-gradient-to-t from-black/75 to-transparent px-2 pb-1 pt-4 text-left text-[10px] text-white/85">
                  {wallpaper.name}
                </span>
                {onMonitors.length > 1 ? (
                  <span className="absolute right-1.5 top-1.5 rounded-full bg-cyan-500/85 px-1.5 py-0.5 text-[9px] font-medium text-white">
                    Pantallas {onMonitors.map((m) => m.index + 1).join(", ")}
                  </span>
                ) : onMonitors.length === 1 ? (
                  <span className="absolute right-1.5 top-1.5 rounded-full bg-cyan-500/85 px-1.5 py-0.5 text-[9px] font-medium text-white">
                    Pantalla {onMonitors[0].index + 1}
                  </span>
                ) : isCurrent ? (
                  <span className="absolute right-1.5 top-1.5 rounded-full bg-cyan-500/85 px-1.5 py-0.5 text-[9px] font-medium text-white">
                    Actual
                  </span>
                ) : null}
              </button>
              {wallpaper.source === "propios" && (
                <button
                  type="button"
                  aria-label={`Borrar ${wallpaper.name}`}
                  title="Borrar fondo"
                  onClick={() => void removeWallpaper(wallpaper)}
                  className="absolute left-1.5 top-1.5 flex h-6 w-6 items-center justify-center rounded-md border border-white/10 bg-black/55 text-white/70 opacity-0 transition-opacity hover:bg-red-500/70 hover:text-white group-hover:opacity-100"
                >
                  <TrashIcon className="h-3.5 w-3.5" />
                </button>
              )}
            </div>
          );
        })}
      </div>

      {error && <p className="shrink-0 text-center text-xs leading-5 text-red-300/90">{error}</p>}
    </div>
  );
}
