export type TabId = "controls" | "stats" | "clipboard" | "wallpaper" | "power";

export interface TabDef {
  id: TabId;
  label: string;
  phase: string;
}

export const TABS: readonly TabDef[] = [
  { id: "controls", label: "Controles", phase: "Fase 3-5" },
  { id: "stats", label: "Sistema", phase: "Fase 6" },
  { id: "clipboard", label: "Portapapeles", phase: "Fase 8" },
  { id: "wallpaper", label: "Fondo", phase: "Fase 9" },
  { id: "power", label: "Energía", phase: "Fase 2" },
] as const;
