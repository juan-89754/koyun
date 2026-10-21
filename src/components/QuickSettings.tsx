import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import {
  BoltIcon,
  CheckIcon,
  DeviceIcon,
  WifiIcon,
  XIcon,
} from "./icons";
import { useErrorState } from "../lib/errors";

export type QuickPanel = "wifi" | "bluetooth" | "power";

interface NetworkDetail {
  name: string | null;
  ssid: string | null;
  signal: number | null;
  internet: boolean;
  ipv4: string | null;
  gateway: string | null;
  dns: string[];
  adapter: string | null;
  security: string | null;
}

interface WifiNetwork {
  ssid: string;
  signal: number;
  secure: boolean;
  connected: boolean;
  profile: boolean;
}

interface BluetoothDeviceInfo {
  id: string;
  name: string;
  connected: boolean;
  kind: string;
}

interface BatteryInfo {
  present: boolean;
  charging: boolean;
  percent: number | null;
  minutes_left: number | null;
}

interface PowerPlan {
  guid: string;
  name: string;
  active: boolean;
}

const PANEL_TITLES: Record<QuickPanel, string> = {
  wifi: "Wi-Fi y red",
  bluetooth: "Bluetooth",
  power: "Energía",
};

function SignalMeter({ signal }: { signal: number }) {
  const bars = Math.round((signal / 100) * 4);
  return (
    <div className="flex items-end gap-[2px]" aria-hidden>
      {[1, 2, 3, 4].map((i) => (
        <span
          key={i}
          className={`w-1 rounded-sm ${i <= bars ? "bg-emerald-400" : "bg-white/15"}`}
          style={{ height: `${4 + i * 3}px` }}
        />
      ))}
    </div>
  );
}

function formatMinutes(minutes: number) {
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  return h > 0 ? `${h} h ${m} min` : `${m} min`;
}

export function QuickSettings({
  panel,
  onClose,
}: {
  panel: QuickPanel | null;
  onClose: () => void;
}) {
  const [detail, setDetail] = useState<NetworkDetail | null>(null);
  const [networks, setNetworks] = useState<WifiNetwork[] | null>(null);
  const [devices, setDevices] = useState<BluetoothDeviceInfo[] | null>(null);
  const [battery, setBattery] = useState<BatteryInfo | null>(null);
  const [plans, setPlans] = useState<PowerPlan[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const { error, setError, reportError } = useErrorState("ajustes-rápidos");

  const load = useCallback(async (target: QuickPanel) => {
    setError(null);
    try {
      if (target === "wifi") {
        const [d, list] = await Promise.all([
          invoke<NetworkDetail>("quick_network_detail"),
          invoke<WifiNetwork[]>("quick_wifi_networks"),
        ]);
        setDetail(d);
        setNetworks(list);
      } else if (target === "bluetooth") {
        const list = await invoke<BluetoothDeviceInfo[]>("quick_bluetooth_devices");
        setDevices(list);
      } else {
        const [b, p] = await Promise.all([
          invoke<BatteryInfo>("quick_battery"),
          invoke<PowerPlan[]>("quick_power_plans"),
        ]);
        setBattery(b);
        setPlans(p);
      }
    } catch (e) {
      reportError(e);
    }
  }, [reportError, setError]);

  useEffect(() => {
    if (panel) void load(panel);
  }, [panel, load]);

  const choosePlan = useCallback(
    async (guid: string) => {
      setBusy(guid);
      setError(null);
      try {
        await invoke("quick_power_plan_set", { guid });
        setPlans((prev) => prev?.map((p) => ({ ...p, active: p.guid === guid })) ?? prev);
      } catch (e) {
        reportError(e);
      } finally {
        setBusy(null);
      }
    },
    [reportError, setError],
  );

  const connectWifi = useCallback(
    async (ssid: string) => {
      setBusy(ssid);
      setError(null);
      try {
        await invoke("quick_wifi_connect", { ssid });
        // El enlace tarda unos segundos en establecerse: se refresca tras una
        // pausa breve para reflejar el estado real.
        window.setTimeout(() => void load("wifi"), 2500);
      } catch (e) {
        reportError(e);
      } finally {
        setBusy(null);
      }
    },
    [load, reportError, setError],
  );

  const disconnectWifi = useCallback(async () => {
    setBusy("disconnect");
    setError(null);
    try {
      await invoke("quick_wifi_disconnect");
      window.setTimeout(() => void load("wifi"), 2000);
    } catch (e) {
      reportError(e);
    } finally {
      setBusy(null);
    }
  }, [load, reportError, setError]);

  const pairBluetooth = useCallback(
    async (id: string) => {
      setBusy(id);
      setError(null);
      try {
        await invoke("quick_bluetooth_pair", { id });
        window.setTimeout(() => void load("bluetooth"), 2500);
      } catch (e) {
        reportError(e);
      } finally {
        setBusy(null);
      }
    },
    [load, reportError, setError],
  );

  const unpairBluetooth = useCallback(
    async (id: string) => {
      setBusy(id);
      setError(null);
      try {
        await invoke("quick_bluetooth_unpair", { id });
        window.setTimeout(() => void load("bluetooth"), 1500);
      } catch (e) {
        reportError(e);
      } finally {
        setBusy(null);
      }
    },
    [load, reportError, setError],
  );

  return (
    <div
      role="dialog"
      aria-label={panel ? PANEL_TITLES[panel] : "Ajustes rápidos"}
      className={`absolute inset-0 z-20 flex justify-end transition-opacity duration-200 ${
        panel ? "pointer-events-auto opacity-100" : "pointer-events-none opacity-0"
      }`}
    >
      <button
        type="button"
        tabIndex={panel ? 0 : -1}
        aria-label="Cerrar ajustes rápidos"
        onClick={onClose}
        className="absolute inset-0 cursor-default bg-black/45"
      />
      <aside
        className={`relative flex h-full w-[380px] max-w-[85%] flex-col border-l border-white/10 bg-zinc-900 shadow-2xl transition-transform duration-200 ${
          panel ? "translate-x-0" : "translate-x-full"
        }`}
      >
        <header className="flex h-12 shrink-0 items-center justify-between border-b border-white/8 px-4">
          <h2 className="text-sm font-medium text-slate-100">
            {panel ? PANEL_TITLES[panel] : ""}
          </h2>
          <button
            type="button"
            aria-label="Cerrar"
            onClick={onClose}
            className="rounded-md p-1.5 text-white/55 transition-colors hover:bg-white/8 hover:text-white"
          >
            <XIcon className="h-4 w-4" />
          </button>
        </header>

        <div className="koyun-scroll min-h-0 flex-1 overflow-y-auto px-4 py-4">
          {panel === "wifi" && (
            <div className="flex flex-col gap-5">
              <section>
                <p className="mb-2 text-[11px] font-medium tracking-wide text-white/45">
                  Red actual
                </p>
                <div className="rounded-xl border border-white/10 bg-white/5 p-3.5">
                  {detail ? (
                    <>
                      <div className="flex items-center gap-2.5">
                        <div
                          className={`flex h-8 w-8 items-center justify-center rounded-lg border ${
                            detail.internet
                              ? "border-emerald-400/40 bg-emerald-500/15 text-emerald-300"
                              : "border-white/10 bg-white/5 text-slate-400"
                          }`}
                        >
                          <WifiIcon className="h-4 w-4" />
                        </div>
                        <div className="min-w-0 flex-1">
                          <p className="truncate text-sm font-medium text-slate-100">
                            {detail.ssid ?? detail.name ?? "Sin conexión"}
                          </p>
                          <p className="truncate text-[11px] text-white/45">
                            {detail.internet ? "Con acceso a Internet" : "Sin acceso a Internet"}
                          </p>
                        </div>
                        {detail.signal !== null && <SignalMeter signal={detail.signal} />}
                      </div>
                      <dl className="mt-3 grid grid-cols-[auto_1fr] gap-x-3 gap-y-1.5 text-[11px]">
                        {detail.ipv4 && (
                          <>
                            <dt className="text-white/40">IP</dt>
                            <dd className="truncate tabular-nums text-slate-200">{detail.ipv4}</dd>
                          </>
                        )}
                        {detail.gateway && (
                          <>
                            <dt className="text-white/40">Puerta</dt>
                            <dd className="truncate tabular-nums text-slate-200">{detail.gateway}</dd>
                          </>
                        )}
                        {detail.dns.length > 0 && (
                          <>
                            <dt className="text-white/40">DNS</dt>
                            <dd className="truncate tabular-nums text-slate-200">
                              {detail.dns.join(", ")}
                            </dd>
                          </>
                        )}
                        {detail.adapter && (
                          <>
                            <dt className="text-white/40">Adaptador</dt>
                            <dd className="truncate text-slate-200">{detail.adapter}</dd>
                          </>
                        )}
                        {detail.security && (
                          <>
                            <dt className="text-white/40">Seguridad</dt>
                            <dd className="truncate text-slate-200">{detail.security}</dd>
                          </>
                        )}
                      </dl>
                    </>
                  ) : (
                    <p className="text-xs text-white/40">Consultando red…</p>
                  )}
                </div>
              </section>

              <section>
                <div className="mb-2 flex items-center justify-between">
                  <p className="text-[11px] font-medium tracking-wide text-white/45">
                    Redes disponibles
                  </p>
                  {detail?.ssid && (
                    <button
                      type="button"
                      disabled={busy !== null}
                      onClick={() => void disconnectWifi()}
                      className="text-[11px] text-white/45 transition-colors hover:text-white disabled:opacity-40"
                    >
                      Desconectar
                    </button>
                  )}
                </div>
                <ul className="flex flex-col gap-1">
                  {(networks ?? []).map((net) => {
                    const canConnect = net.profile && !net.connected;
                    const isBusy = busy === net.ssid;
                    return (
                      <li key={net.ssid}>
                        <button
                          type="button"
                          disabled={!canConnect || busy !== null}
                          onClick={() => canConnect && void connectWifi(net.ssid)}
                          title={
                            net.profile
                              ? undefined
                              : "Sin perfil guardado: conéctate una vez desde Windows"
                          }
                          className={`flex w-full items-center gap-3 rounded-lg border px-3 py-2 text-left transition-colors disabled:cursor-default ${
                            net.connected
                              ? "border-sky-400/40 bg-sky-500/10"
                              : canConnect
                                ? "border-white/8 bg-white/4 hover:border-white/20 hover:bg-white/8"
                                : "border-white/8 bg-white/4 opacity-60"
                          }`}
                        >
                          <WifiIcon
                            className={`h-4 w-4 shrink-0 ${
                              net.connected ? "text-sky-300" : "text-white/45"
                            }`}
                          />
                          <div className="min-w-0 flex-1">
                            <p className="truncate text-xs font-medium text-slate-200">
                              {net.ssid}
                            </p>
                            <p className="text-[10px] text-white/40">
                              {isBusy
                                ? "Conectando…"
                                : `${net.secure ? "Protegida" : "Abierta"}${
                                    net.connected
                                      ? " · Conectada"
                                      : net.profile
                                        ? ""
                                        : " · Sin perfil"
                                  }`}
                            </p>
                          </div>
                          <SignalMeter signal={net.signal} />
                        </button>
                      </li>
                    );
                  })}
                  {networks !== null && networks.length === 0 && (
                    <li className="rounded-lg border border-white/8 bg-white/4 px-3 py-4 text-center text-xs text-white/40">
                      No se han encontrado redes Wi-Fi.
                    </li>
                  )}
                  {networks === null && (
                    <li className="px-3 py-2 text-xs text-white/40">Buscando redes…</li>
                  )}
                </ul>
              </section>
            </div>
          )}

          {panel === "bluetooth" && (
            <ul className="flex flex-col gap-1.5">
              {(devices ?? []).map((dev) => {
                const isBusy = busy === dev.id;
                return (
                  <li
                    key={dev.id}
                    className={`flex items-center gap-3 rounded-lg border px-3 py-2.5 ${
                      dev.connected
                        ? "border-blue-400/40 bg-blue-500/10"
                        : "border-white/8 bg-white/4"
                    }`}
                  >
                    <DeviceIcon
                      className={`h-4 w-4 shrink-0 ${
                        dev.connected ? "text-blue-300" : "text-white/45"
                      }`}
                    />
                    <div className="min-w-0 flex-1">
                      <p className="truncate text-xs font-medium text-slate-200">{dev.name}</p>
                      <p className="text-[10px] text-white/40">
                        {isBusy
                          ? "Procesando…"
                          : `${dev.kind}${dev.connected ? " · Conectado" : ""}`}
                      </p>
                    </div>
                    <button
                      type="button"
                      disabled={busy !== null}
                      onClick={() =>
                        dev.connected
                          ? void unpairBluetooth(dev.id)
                          : void pairBluetooth(dev.id)
                      }
                      className={`shrink-0 rounded-md border px-2 py-1 text-[10px] font-medium transition-colors disabled:opacity-40 ${
                        dev.connected
                          ? "border-white/10 text-white/55 hover:border-red-400/40 hover:bg-red-500/15 hover:text-red-200"
                          : "border-blue-400/40 text-blue-200 hover:bg-blue-500/15"
                      }`}
                    >
                      {dev.connected ? "Quitar" : "Conectar"}
                    </button>
                  </li>
                );
              })}
              {devices !== null && devices.length === 0 && (
                <li className="rounded-lg border border-white/8 bg-white/4 px-3 py-4 text-center text-xs text-white/40">
                  No hay dispositivos Bluetooth emparejados.
                </li>
              )}
              {devices === null && <li className="px-1 py-2 text-xs text-white/40">Buscando…</li>}
            </ul>
          )}

          {panel === "power" && (
            <div className="flex flex-col gap-5">
              <section>
                <p className="mb-2 text-[11px] font-medium tracking-wide text-white/45">Batería</p>
                <div className="rounded-xl border border-white/10 bg-white/5 p-3.5">
                  {battery ? (
                    battery.present ? (
                      <div className="flex items-center gap-3">
                        <BoltIcon
                          className={`h-5 w-5 ${
                            battery.charging ? "text-emerald-300" : "text-amber-300"
                          }`}
                        />
                        <div className="flex-1">
                          <p className="text-sm font-medium text-slate-100">
                            {battery.percent !== null ? `${battery.percent}%` : "—"}
                          </p>
                          <p className="text-[11px] text-white/45">
                            {battery.charging
                              ? "Enchufado"
                              : battery.minutes_left !== null
                                ? `${formatMinutes(battery.minutes_left)} restantes`
                                : "Con batería"}
                          </p>
                        </div>
                      </div>
                    ) : (
                      <p className="text-xs text-white/45">Este equipo no tiene batería.</p>
                    )
                  ) : (
                    <p className="text-xs text-white/40">Consultando…</p>
                  )}
                </div>
              </section>

              <section>
                <p className="mb-2 text-[11px] font-medium tracking-wide text-white/45">
                  Plan de energía
                </p>
                <ul className="flex flex-col gap-1">
                  {(plans ?? []).map((plan) => (
                    <li key={plan.guid}>
                      <button
                        type="button"
                        disabled={busy !== null || plan.active}
                        onClick={() => void choosePlan(plan.guid)}
                        className={`flex w-full items-center gap-3 rounded-lg border px-3 py-2.5 text-left transition-colors disabled:cursor-default ${
                          plan.active
                            ? "border-emerald-400/40 bg-emerald-500/10"
                            : "border-white/8 bg-white/4 hover:border-white/20 hover:bg-white/8"
                        }`}
                      >
                        <BoltIcon
                          className={`h-4 w-4 shrink-0 ${
                            plan.active ? "text-emerald-300" : "text-white/45"
                          }`}
                        />
                        <span className="min-w-0 flex-1 truncate text-xs font-medium text-slate-200">
                          {plan.name}
                        </span>
                        {plan.active && <CheckIcon className="h-4 w-4 text-emerald-300" />}
                      </button>
                    </li>
                  ))}
                  {plans === null && <li className="px-1 py-2 text-xs text-white/40">Cargando…</li>}
                </ul>
              </section>
            </div>
          )}

          {error && <p className="mt-4 text-center text-xs text-red-300/90">{error}</p>}
        </div>
      </aside>
    </div>
  );
}
