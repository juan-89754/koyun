import { invoke } from "@tauri-apps/api/core";
import { useCallback, useRef, useState } from "react";
import {
  ClockIcon,
  CpuIcon,
  DiskIcon,
  DownloadIcon,
  GaugeIcon,
  GpuIcon,
  RamIcon,
  UploadIcon,
} from "../icons";
import {
  formatBps,
  formatBytes,
  formatMbps,
  formatUptime,
  ratio,
} from "../../lib/format";
import { useErrorState } from "../../lib/errors";
import { usePolling } from "../../lib/usePolling";

interface DiskInfo {
  name: string;
  total: number;
  free: number;
}

interface StatsSnapshot {
  cpu: number | null;
  ram_used: number;
  ram_total: number;
  gpu_name: string | null;
  gpu_vram: number | null;
  gpu_usage: number | null;
  disks: DiskInfo[];
  uptime_ms: number;
  net_down_bps: number | null;
  net_up_bps: number | null;
}

interface SpeedResult {
  latency_ms: number | null;
  down_bps: number | null;
  up_bps: number | null;
}

const DOWNLOAD_BYTES = 25_000_000;
const UPLOAD_BYTES = 8_000_000;
/** Límite global de la prueba de velocidad (descarga + subida + latencia). */
const SPEED_TEST_TIMEOUT_MS = 90_000;
const HOST = "speed.cloudflare.com";

function isAbort(error: unknown): boolean {
  return error instanceof DOMException && error.name === "AbortError";
}

async function measureLatency(signal: AbortSignal): Promise<number | null> {
  const samples: number[] = [];
  for (let i = 0; i < 4; i++) {
    if (signal.aborted) break;
    const started = performance.now();
    try {
      await fetch(`https://${HOST}/__down?bytes=0&t=${Date.now()}`, {
        cache: "no-store",
        signal,
      });
      samples.push(performance.now() - started);
    } catch {
      return samples.length > 0 ? Math.min(...samples) : null;
    }
  }
  if (samples.length === 0) return null;
  return Math.min(...samples);
}

async function measureDownload(
  onProgress: (fraction: number) => void,
  signal: AbortSignal,
): Promise<number | null> {
  try {
    const url = `https://${HOST}/__down?bytes=${DOWNLOAD_BYTES}&t=${Date.now()}`;
    const response = await fetch(url, { cache: "no-store", signal });
    if (!response.ok || !response.body) return null;
    const reader = response.body.getReader();
    let received = 0;
    const started = performance.now();
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      received += value?.length ?? 0;
      onProgress(Math.min(1, received / DOWNLOAD_BYTES));
    }
    const seconds = (performance.now() - started) / 1000;
    if (seconds <= 0 || received === 0) return null;
    return (received * 8) / seconds;
  } catch {
    return null;
  }
}

/** XHR en lugar de `fetch`: es la única forma de leer el progreso real de
 * subida y de poder cancelarla a mitad con `AbortSignal`. */
function measureUpload(
  onProgress: (fraction: number) => void,
  signal: AbortSignal,
): Promise<number | null> {
  return new Promise((resolve) => {
    if (signal.aborted) {
      resolve(null);
      return;
    }

    const xhr = new XMLHttpRequest();
    let finished = false;
    const done = (value: number | null) => {
      if (finished) return;
      finished = true;
      signal.removeEventListener("abort", cancel);
      resolve(value);
    };
    function cancel() {
      xhr.abort();
    }

    const started = performance.now();
    xhr.open("POST", `https://${HOST}/__up`);
    xhr.timeout = 30_000;
    xhr.responseType = "text";
    xhr.upload.onprogress = (e) => {
      if (e.lengthComputable) onProgress(e.loaded / e.total);
    };
    xhr.onload = () => {
      const seconds = (performance.now() - started) / 1000;
      const ok = xhr.status >= 200 && xhr.status < 300 && seconds > 0;
      done(ok ? (UPLOAD_BYTES * 8) / seconds : null);
    };
    xhr.onabort = () => done(null);
    xhr.onerror = () => done(null);
    xhr.ontimeout = () => done(null);

    signal.addEventListener("abort", cancel, { once: true });
    xhr.send(new Blob([new Uint8Array(UPLOAD_BYTES)]));
  });
}

function Bar({ value, className }: { value: number; className: string }) {
  return (
    <div className="h-1.5 w-full overflow-hidden rounded-full bg-white/10">
      <div
        className={`h-full rounded-full transition-[width] duration-500 ${className}`}
        style={{ width: `${Math.min(100, Math.max(0, value))}%` }}
      />
    </div>
  );
}

function MetricCard({
  label,
  icon: Icon,
  iconClass,
  value,
  detail,
  children,
}: {
  label: string;
  icon: typeof CpuIcon;
  iconClass: string;
  value: string;
  detail?: string;
  children?: React.ReactNode;
}) {
  return (
    <section className="flex flex-col gap-3 rounded-xl border border-white/10 bg-white/5 p-4">
      <div className="flex items-center gap-2.5">
        <Icon className={`h-4 w-4 ${iconClass}`} />
        <span className="text-xs font-medium tracking-wide text-white/50">{label}</span>
      </div>
      <div>
        <p className="truncate text-lg font-semibold tabular-nums text-slate-100">{value}</p>
        {detail && <p className="truncate text-[11px] text-white/40">{detail}</p>}
      </div>
      {children}
    </section>
  );
}

export function StatsTab() {
  const [stats, setStats] = useState<StatsSnapshot | null>(null);
  const { error, setError, reportError } = useErrorState("estadísticas");
  const [speed, setSpeed] = useState<SpeedResult>({
    latency_ms: null,
    down_bps: null,
    up_bps: null,
  });
  const [speedBusy, setSpeedBusy] = useState(false);
  const [progress, setProgress] = useState(0);
  const [speedError, setSpeedError] = useState<string | null>(null);
  const running = useRef(false);
  const abortRef = useRef<AbortController | null>(null);
  const timedOut = useRef(false);

  const load = useCallback(() => {
    invoke<StatsSnapshot>("stats_snapshot")
      .then((s) => {
        setError(null);
        setStats(s);
      })
      .catch(reportError);
  }, [reportError, setError]);

  usePolling(load, 1500);

  const cancelSpeedTest = useCallback(() => {
    abortRef.current?.abort();
  }, []);

  const runSpeedTest = useCallback(async () => {
    if (running.current) return;
    running.current = true;
    timedOut.current = false;

    const controller = new AbortController();
    abortRef.current = controller;
    const timer = window.setTimeout(() => {
      timedOut.current = true;
      controller.abort();
    }, SPEED_TEST_TIMEOUT_MS);
    const { signal } = controller;

    setSpeedBusy(true);
    setSpeedError(null);
    setProgress(0);
    setSpeed({ latency_ms: null, down_bps: null, up_bps: null });

    try {
      const latency = await measureLatency(signal);
      setSpeed((s) => ({ ...s, latency_ms: latency }));

      const down = await measureDownload((f) => setProgress(f * 0.7), signal);
      setSpeed((s) => ({ ...s, down_bps: down }));

      setProgress(0.7);
      const up = await measureUpload((f) => setProgress(0.7 + f * 0.3), signal);
      setSpeed((s) => ({ ...s, up_bps: up }));

      if (signal.aborted) {
        setSpeedError(
          timedOut.current
            ? "La prueba ha superado el tiempo límite (90 s)."
            : "Prueba cancelada.",
        );
      } else if (down === null && up === null) {
        setSpeedError("No se pudo completar la prueba. Comprueba tu conexión a Internet.");
      }
    } catch (e) {
      setSpeedError(
        timedOut.current
          ? "La prueba ha superado el tiempo límite (90 s)."
          : isAbort(e)
            ? "Prueba cancelada."
            : "No se pudo completar la prueba.",
      );
    } finally {
      window.clearTimeout(timer);
      abortRef.current = null;
      if (!signal.aborted) setProgress(1);
      setSpeedBusy(false);
      running.current = false;
    }
  }, []);

  const cpu = stats?.cpu ?? null;
  const ramPercent = stats ? ratio(stats.ram_used, stats.ram_total) : 0;
  const gpuUsage = stats?.gpu_usage ?? null;

  return (
    <div
      id="panel-stats"
      role="tabpanel"
      aria-labelledby="tab-stats"
      className="flex h-full flex-col gap-4 px-8 py-5"
    >
      <div className="grid shrink-0 grid-cols-4 gap-4">
        <MetricCard
          label="Procesador"
          icon={CpuIcon}
          iconClass="text-sky-300"
          value={cpu !== null ? `${cpu.toFixed(0)}%` : "—"}
          detail="Uso total de CPU"
        >
          <Bar value={cpu ?? 0} className="bg-sky-400/80" />
        </MetricCard>

        <MetricCard
          label="Memoria"
          icon={RamIcon}
          iconClass="text-emerald-300"
          value={stats ? `${Math.round(ramPercent)}%` : "—"}
          detail={stats ? `${formatBytes(stats.ram_used)} de ${formatBytes(stats.ram_total)}` : ""}
        >
          <Bar value={ramPercent} className="bg-emerald-400/80" />
        </MetricCard>

        <MetricCard
          label="Gráficos"
          icon={GpuIcon}
          iconClass="text-fuchsia-300"
          value={gpuUsage !== null ? `${gpuUsage.toFixed(0)}%` : "—"}
          detail={
            stats?.gpu_name
              ? stats.gpu_vram
                ? `${stats.gpu_name} · ${formatBytes(stats.gpu_vram)}`
                : stats.gpu_name
              : "Sin datos de GPU"
          }
        >
          <Bar value={gpuUsage ?? 0} className="bg-fuchsia-400/80" />
        </MetricCard>

        <MetricCard
          label="Tiempo activo"
          icon={ClockIcon}
          iconClass="text-amber-300"
          value={stats ? formatUptime(stats.uptime_ms) : "—"}
          detail="Desde el último inicio"
        />
      </div>

      <section className="shrink-0 rounded-xl border border-white/10 bg-white/5 px-5 py-4">
        <p className="mb-3 text-xs font-medium tracking-wide text-white/50">Discos</p>
        <div className="space-y-2.5">
          {stats && stats.disks.length === 0 && (
            <p className="text-xs text-white/40">No se han detectado unidades.</p>
          )}
          {stats?.disks.map((disk) => {
            const used = disk.total - disk.free;
            const percent = ratio(used, disk.total);
            return (
              <div key={disk.name} className="flex items-center gap-3">
                <DiskIcon className="h-4 w-4 shrink-0 text-white/40" />
                <span className="w-10 shrink-0 text-xs font-medium text-slate-200">
                  {disk.name}
                </span>
                <div className="min-w-0 flex-1">
                  <Bar
                    value={percent}
                    className={percent > 90 ? "bg-red-400/80" : "bg-indigo-400/80"}
                  />
                </div>
                <span className="w-40 shrink-0 text-right text-[11px] tabular-nums text-white/45">
                  {formatBytes(disk.free)} libres de {formatBytes(disk.total)}
                </span>
              </div>
            );
          })}
        </div>
      </section>

      <div className="grid shrink-0 grid-cols-5 gap-4">
        <section className="col-span-3 flex items-center gap-4 rounded-xl border border-white/10 bg-white/5 px-5 py-4">
          <GaugeIcon className="h-4 w-4 shrink-0 text-white/40" />
          <div className="flex flex-1 items-center justify-around">
            <div className="flex items-center gap-2">
              <DownloadIcon className="h-4 w-4 text-sky-300" />
              <div>
                <p className="text-sm font-semibold tabular-nums text-slate-100">
                  {stats ? formatBps(stats.net_down_bps) : "—"}
                </p>
                <p className="text-[10px] text-white/40">Descarga</p>
              </div>
            </div>
            <div className="flex items-center gap-2">
              <UploadIcon className="h-4 w-4 text-violet-300" />
              <div>
                <p className="text-sm font-semibold tabular-nums text-slate-100">
                  {stats ? formatBps(stats.net_up_bps) : "—"}
                </p>
                <p className="text-[10px] text-white/40">Subida</p>
              </div>
            </div>
          </div>
        </section>

        <section
          aria-busy={speedBusy}
          className="col-span-2 flex flex-col gap-2 rounded-xl border border-white/10 bg-white/5 px-4 py-3"
        >
          <div className="flex items-center justify-between gap-2">
            <span className="text-xs font-medium tracking-wide text-white/50">
              Prueba de velocidad
            </span>
            {speedBusy ? (
              <button
                type="button"
                onClick={cancelSpeedTest}
                className="rounded-md border border-amber-400/40 bg-amber-500/15 px-2.5 py-1 text-[11px] font-medium text-amber-100 transition-colors hover:bg-amber-500/25"
              >
                Cancelar
              </button>
            ) : (
              <button
                type="button"
                onClick={() => void runSpeedTest()}
                className="rounded-md border border-sky-400/40 bg-sky-500/15 px-2.5 py-1 text-[11px] font-medium text-sky-100 transition-colors hover:bg-sky-500/25"
              >
                Iniciar
              </button>
            )}
          </div>
          {speedBusy && <Bar value={progress * 100} className="bg-sky-400/80" />}
          <p className="text-[10px] leading-4 text-white/35">
            Los datos de la prueba viajan únicamente a speed.cloudflare.com: Koyun no los
            almacena.
          </p>
          <div className="flex flex-1 items-center justify-around">
            <div className="text-center">
              <p className="text-sm font-semibold tabular-nums text-slate-100">
                {speed.down_bps !== null ? formatMbps(speed.down_bps) : "—"}
              </p>
              <p className="text-[10px] text-white/40">Descarga</p>
            </div>
            <div className="text-center">
              <p className="text-sm font-semibold tabular-nums text-slate-100">
                {speed.up_bps !== null ? formatMbps(speed.up_bps) : "—"}
              </p>
              <p className="text-[10px] text-white/40">Subida</p>
            </div>
            <div className="text-center">
              <p className="text-sm font-semibold tabular-nums text-slate-100">
                {speed.latency_ms !== null ? `${Math.round(speed.latency_ms)} ms` : "—"}
              </p>
              <p className="text-[10px] text-white/40">Latencia</p>
            </div>
          </div>
        </section>
      </div>

      {(error || speedError) && (
        <p
          role="alert"
          className="shrink-0 text-center text-xs leading-5 text-red-300/90"
        >
          {speedError ?? error}
        </p>
      )}
    </div>
  );
}
