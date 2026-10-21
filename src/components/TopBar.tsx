import { useRef } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { TABS, type TabId } from "../lib/tabs";
import { useUiStore } from "../store/ui";
import { ControlsIcon, StatsIcon, ClipboardIcon, WallpaperIcon, PowerIcon, HideIcon } from "./icons";

const TAB_ICONS: Record<TabId, typeof ControlsIcon> = {
  controls: ControlsIcon,
  stats: StatsIcon,
  clipboard: ClipboardIcon,
  wallpaper: WallpaperIcon,
  power: PowerIcon,
};

interface TopBarProps {
  onRequestFocusTab: (index: number) => void;
  tabIndex: number;
}

export function TopBar({ onRequestFocusTab, tabIndex }: TopBarProps) {
  const activeTab = useUiStore((s) => s.activeTab);
  const setActiveTab = useUiStore((s) => s.setActiveTab);
  const refs = useRef<(HTMLButtonElement | null)[]>([]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    let next = -1;
    if (e.key === "ArrowRight") next = (tabIndex + 1) % TABS.length;
    else if (e.key === "ArrowLeft") next = (tabIndex - 1 + TABS.length) % TABS.length;
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = TABS.length - 1;
    if (next >= 0) {
      e.preventDefault();
      setActiveTab(TABS[next].id);
      onRequestFocusTab(next);
      refs.current[next]?.focus();
    }
  };

  return (
    <header
      data-tauri-drag-region
      className="flex h-12 shrink-0 items-center gap-1 border-b border-white/8 px-3 select-none"
    >
      <span data-tauri-drag-region className="mr-2 text-[13px] font-semibold tracking-wide text-white/90">
        Koyun
      </span>

      <nav
        role="tablist"
        aria-label="Secciones"
        className="flex items-center gap-1"
        onKeyDown={handleKeyDown}
      >
        {TABS.map((tab, i) => {
          const Icon = TAB_ICONS[tab.id];
          const isActive = activeTab === tab.id;
          return (
            <button
              key={tab.id}
              ref={(el) => {
                refs.current[i] = el;
              }}
              role="tab"
              id={`tab-${tab.id}`}
              aria-selected={isActive}
              aria-controls={`panel-${tab.id}`}
              tabIndex={isActive ? 0 : -1}
              onClick={() => setActiveTab(tab.id)}
              className={[
                "flex items-center gap-1.5 rounded-md px-2.5 py-1.5 text-xs transition-colors",
                isActive
                  ? "bg-white/12 text-white"
                  : "text-white/55 hover:bg-white/6 hover:text-white/85",
              ].join(" ")}
            >
              <Icon />
              {tab.label}
            </button>
          );
        })}
      </nav>

      <div className="ml-auto flex items-center gap-1">
        <button
          type="button"
          aria-label="Ocultar panel (Ctrl+Alt+K)"
          title="Ocultar panel (Ctrl+Alt+K)"
          onClick={() => void getCurrentWindow().hide()}
          className="rounded-md p-1.5 text-white/55 transition-colors hover:bg-white/6 hover:text-white"
        >
          <HideIcon width={15} height={15} />
        </button>
      </div>
    </header>
  );
}
