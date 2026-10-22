export type TabId =
  | "controls"
  | "stats"
  | "clipboard"
  | "wallpaper"
  | "power"
  | "mixer"
  | "notifications"
  | "settings";

export interface TabDef {
  id: TabId;
  label: string;
  phase: string;
}

export const TABS: readonly TabDef[] = [
  { id: "controls", label: "Controles", phase: "Fase 3-5" },
  { id: "mixer", label: "Mezclador", phase: "Fase 10" },
  { id: "stats", label: "Sistema", phase: "Fase 6" },
  { id: "clipboard", label: "Portapapeles", phase: "Fase 8" },
  { id: "wallpaper", label: "Fondo", phase: "Fase 9" },
  { id: "notifications", label: "Avisos", phase: "Fase 11" },
  { id: "power", label: "Energía", phase: "Fase 2" },
  { id: "settings", label: "Ajustes", phase: "Fase 12" },
] as const;
