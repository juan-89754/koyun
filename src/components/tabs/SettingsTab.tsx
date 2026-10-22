import { useEffect, useRef, useState } from "react";
import {
  ACCENT_PRESETS,
  normalizeHex,
  type AccentMode,
  type ThemeMode,
} from "../../lib/theme";
import { acceleratorFromEvent, formatAccelerator } from "../../lib/hotkeys";
import { useConfigStore } from "../../store/config";
import { useGameModeStore } from "../../store/gameMode";
import { NumberInput, Row, Section, Segmented, Toggle } from "../settings/Controls";

const THEME_MODES: readonly { id: ThemeMode; label: string }[] = [
  { id: "auto", label: "Automático" },
  { id: "light", label: "Claro" },
  { id: "dark", label: "Oscuro" },
];

const ACCENT_MODES: readonly { id: AccentMode; label: string }[] = [
  { id: "system", label: "Del sistema" },
  { id: "custom", label: "Personalizado" },
];

const ISLAND_WIDGETS: readonly { id: string; label: string }[] = [
  { id: "auto", label: "Automático" },
  { id: "logo", label: "Logotipo" },
  { id: "media", label: "Multimedia" },
  { id: "stats", label: "Rendimiento" },
];

export function SettingsTab() {
  const config = useConfigStore((s) => s.config);
  const systemTheme = useConfigStore((s) => s.systemTheme);
  const hotkeys = useConfigStore((s) => s.hotkeys);
  const patch = useConfigStore((s) => s.patch);
  const gameMode = useGameModeStore();
  const [capturing, setCapturing] = useState<string | null>(null);
  const [customAccent, setCustomAccent] = useState("#2dd4bf");

  useEffect(() => {
    if (config?.theme.accentColor) setCustomAccent(config.theme.accentColor);
  }, [config?.theme.accentColor]);

  const captureRef = useRef<HTMLButtonElement | null>(null);

  // Mientras se captura un atajo no debe ejecutarse el resto de atajos.
  useEffect(() => {
    if (!capturing) return;

    const onKeyDown = (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopPropagation();
      if (event.key === "Escape") {
        setCapturing(null);
        return;
      }
      const accelerator = acceleratorFromEvent(event);
      if (!accelerator) return;
      setCapturing(null);
      void patch({ hotkeys: { [capturing]: accelerator } });
    };

    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [capturing, patch]);

  useEffect(() => {
    if (capturing) captureRef.current?.focus();
  }, [capturing]);

  useEffect(() => {
    void gameMode.status();
    // Solo al montar la pestaña.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (!config) {
    return (
      <div
        id="panel-settings"
        role="tabpanel"
        aria-labelledby="tab-settings"
        className="flex h-full items-center justify-center px-8 text-xs text-white/45"
      >
        Cargando ajustes…
      </div>
    );
  }

  const { theme, hotkeys: hotkeyConfig, video, island, notifications } = config;

  const setTheme = (patchTheme: Partial<typeof theme>) =>
    void patch({ theme: patchTheme });

  const setHotkey = (action: string, accelerator: string) =>
    void patch({ hotkeys: { [action]: accelerator } });

  const systemAccent = systemTheme?.accent_available ? systemTheme.accent : null;

  return (
    <div
      id="panel-settings"
      role="tabpanel"
      aria-labelledby="tab-settings"
      className="flex h-full flex-col gap-4 px-8 py-5"
    >
      <Section
        title="Apariencia"
        description="El tema se aplica al instante, sin reiniciar la aplicación."
      >
        <Row label="Tema" hint="«Automático» sigue el ajuste de Windows.">
          <Segmented
            label="Tema"
            value={theme.mode}
            options={THEME_MODES}
            onChange={(mode) => setTheme({ mode })}
          />
        </Row>

        <Row
          label="Color de acento"
          hint={
            systemAccent
              ? `Windows usa ${systemAccent}.`
              : "Windows no expone su color de acento."
          }
        >
          <Segmented
            label="Color de acento"
            value={theme.accentMode}
            options={ACCENT_MODES}
            onChange={(accentMode) => setTheme({ accentMode })}
          />
        </Row>

        {theme.accentMode === "custom" && (
          <Row label="Color personalizado" hint="Formato hexadecimal (#rrggbb).">
            <div className="flex items-center gap-2">
              <input
                type="color"
                aria-label="Elegir color de acento"
                value={normalizeHex(customAccent) ?? "#2dd4bf"}
                onChange={(e) => {
                  setCustomAccent(e.target.value);
                  setTheme({ accentColor: e.target.value });
                }}
                className="h-8 w-10 cursor-pointer rounded-lg border border-white/10 bg-transparent"
              />
              <input
                type="text"
                aria-label="Color de acento en hexadecimal"
                value={customAccent}
                onChange={(e) => setCustomAccent(e.target.value)}
                onBlur={() => {
                  const hex = normalizeHex(customAccent);
                  if (hex) setTheme({ accentColor: hex });
                  else setCustomAccent(theme.accentColor);
                }}
                className="w-24 rounded-lg border border-white/10 bg-white/5 px-2 py-1.5 text-xs text-slate-200 outline-none focus:border-accent/60"
              />
            </div>
          </Row>
        )}

        <Row label="Paleta" hint="Paletas predefinidas de acento.">
          <div className="flex flex-wrap items-center justify-end gap-1.5">
            {ACCENT_PRESETS.map((preset) => (
              <button
                key={preset.id || "default"}
                type="button"
                title={preset.label}
                aria-label={preset.label}
                aria-pressed={theme.preset === preset.id}
                onClick={() => setTheme({ preset: preset.id })}
                className={`h-6 w-6 rounded-full border-2 transition-transform hover:scale-110 ${
                  theme.preset === preset.id
                    ? "border-white/70"
                    : "border-white/15"
                }`}
                style={{ backgroundColor: preset.color }}
              />
            ))}
          </div>
        </Row>
      </Section>

      <Section
        title="Atajos globales"
        description="Pulsa una asignación y teclea la combinación nueva. Escape cancela."
      >
        {hotkeys.map((binding) => (
          <Row key={binding.action} label={binding.label}>
            <div className="flex items-center gap-2">
              {binding.error && (
                <span className="max-w-40 text-[10px] text-red-300/90">
                  {binding.error}
                </span>
              )}
              <button
                ref={capturing === binding.action ? captureRef : undefined}
                type="button"
                onClick={() =>
                  setCapturing(capturing === binding.action ? null : binding.action)
                }
                className={`min-w-28 rounded-lg border px-2.5 py-1.5 text-[11px] font-medium transition-colors ${
                  capturing === binding.action
                    ? "border-accent bg-accent/15 text-accent"
                    : "border-white/10 bg-white/5 text-slate-200 hover:border-white/25"
                }`}
              >
                {capturing === binding.action
                  ? "Pulsar combinación…"
                  : formatAccelerator(hotkeyConfig[binding.action as keyof typeof hotkeyConfig] ?? binding.accelerator)}
              </button>
              {(hotkeyConfig[binding.action as keyof typeof hotkeyConfig] ?? "") !== "" && (
                <button
                  type="button"
                  aria-label={`Quitar el atajo de ${binding.label}`}
                  onClick={() => setHotkey(binding.action, "")}
                  className="rounded-md border border-white/10 px-1.5 py-1 text-[10px] text-white/50 transition-colors hover:border-red-400/40 hover:text-red-200"
                >
                  Quitar
                </button>
              )}
            </div>
          </Row>
        ))}
      </Section>

      <Section title="Fondo en movimiento" description="Ajustes del vídeo de escritorio.">
        <Row label="Pausar en pantalla completa" hint="Libera la CPU al ver un vídeo.">
          <Toggle
            label="Pausar en pantalla completa"
            checked={video.pause_on_fullscreen}
            onChange={(value) => void patch({ video: { pause_on_fullscreen: value } })}
          />
        </Row>
        <Row label="Pausar con batería" hint="Ahorra energía en equipos portátiles.">
          <Toggle
            label="Pausar con batería"
            checked={video.pause_on_battery}
            onChange={(value) => void patch({ video: { pause_on_battery: value } })}
          />
        </Row>
        <Row label="Siempre sin sonido" hint="El vídeo nunca reproduce audio.">
          <Toggle
            label="Siempre sin sonido"
            checked={video.always_muted}
            onChange={(value) => void patch({ video: { always_muted: value } })}
          />
        </Row>
      </Section>

      <Section title="Modo Juego" description="Prioriza el juego y limita la carga secundaria.">
        <Row label="Activar Modo Juego" hint="Eleva la prioridad del juego, pausa el vídeo de fondo y los sondeos. Todo se restaura al desactivar o cerrar.">
          <Toggle
            label="Activar Modo Juego"
            checked={gameMode.enabled}
            onChange={(value) => {
              if (value) void gameMode.enable();
              else void gameMode.disable();
            }}
          />
        </Row>
        <Row
          label="Objetivo"
          hint={
            gameMode.targetExe
              ? `${gameMode.targetExe}${gameMode.targetPid ? ` (PID ${gameMode.targetPid})` : ""}`
              : "Sin objetivo: se usará la ventana en primer plano"
          }
        >
          <button
            className="rounded-md border border-white/10 px-2 py-1 text-xs text-slate-200 hover:bg-white/5 disabled:opacity-50"
            onClick={() => void gameMode.useForeground()}
            disabled={gameMode.loading}
          >
            Usar ventana en primer plano
          </button>
        </Row>
        <Row label="Limitar escaneo Wi-Fi" hint="Desactiva el escaneo periódico si estaba activo y lo restaura obligatoriamente al salir.">
          <Toggle
            label="Limitar escaneo Wi-Fi"
            checked={gameMode.limitWifi}
            onChange={(value) => useGameModeStore.setState({ limitWifi: value })}
          />
        </Row>
        <Row label="Seguridad" hint="Sin suspensión de procesos ni prioridad en tiempo real. Solo HIGH_PRIORITY_CLASS del propio juego.">
          <span className="text-[10px] text-amber-300/80">Sin permisos de administrador</span>
        </Row>
        {gameMode.error && (
          <Row label="Error" hint={gameMode.error}>
            <span className="text-[10px] text-red-300">Revisar</span>
          </Row>
        )}
      </Section>

      <Section title="Isla dinámica" description="El pequeño indicador superior.">
        <Row label="Contenido">
          <Segmented
            label="Contenido de la isla"
            value={island.widget}
            options={ISLAND_WIDGETS}
            onChange={(widget) => void patch({ island: { widget } })}
          />
        </Row>
        <Row
          label="Umbral de rendimiento"
          hint="Porcentaje de CPU a partir del cual se muestran las estadísticas."
        >
          <NumberInput
            label="Umbral de rendimiento"
            value={island.stats_threshold}
            min={10}
            max={100}
            step={5}
            onChange={(stats_threshold) => void patch({ island: { stats_threshold } })}
          />
        </Row>
      </Section>

      <Section title="Notificaciones" description="Avisos del sistema capturados.">
        <Row label="Capturar notificaciones" hint="Guarda los avisos de otras aplicaciones.">
          <Toggle
            label="Capturar notificaciones"
            checked={notifications.enabled}
            onChange={(value) => void patch({ notifications: { enabled: value } })}
          />
        </Row>
        <Row label="Máximo guardado" hint="Número de avisos conservados en el cajón.">
          <NumberInput
            label="Máximo guardado"
            value={notifications.keep}
            min={10}
            max={500}
            step={10}
            onChange={(keep) => void patch({ notifications: { keep } })}
          />
        </Row>
      </Section>
    </div>
  );
}
