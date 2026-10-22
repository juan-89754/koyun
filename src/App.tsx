import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { TABS, type TabId } from "./lib/tabs";
import { useUiStore } from "./store/ui";
import { useConfigStore } from "./store/config";
import { TopBar } from "./components/TopBar";
import { ControlsTab } from "./components/tabs/ControlsTab";
import { PowerTab } from "./components/tabs/PowerTab";
import { StatsTab } from "./components/tabs/StatsTab";
import { ClipboardTab } from "./components/tabs/ClipboardTab";
import { WallpaperTab } from "./components/tabs/WallpaperTab";
import { MixerTab } from "./components/tabs/MixerTab";
import NotificationsTab from "./components/tabs/NotificationsTab";
import { SettingsTab } from "./components/tabs/SettingsTab";

function renderTab(tab: TabId) {
  switch (tab) {
    case "controls":
      return <ControlsTab />;
    case "mixer":
      return <MixerTab />;
    case "stats":
      return <StatsTab />;
    case "clipboard":
      return <ClipboardTab />;
    case "wallpaper":
      return <WallpaperTab />;
    case "notifications":
      return <NotificationsTab />;
    case "power":
      return <PowerTab />;
    case "settings":
      return <SettingsTab />;
    default:
      return <ControlsTab />;
  }
}

function App() {
  const activeTab = useUiStore((s) => s.activeTab);
  const setActiveTab = useUiStore((s) => s.setActiveTab);
  const loadConfig = useConfigStore((s) => s.load);
  const [tabIndex, setTabIndex] = useState(() =>
    Math.max(0, TABS.findIndex((tab) => tab.id === useUiStore.getState().activeTab)),
  );

  useEffect(() => {
    void loadConfig();
  }, [loadConfig]);

  useEffect(() => {
    const index = TABS.findIndex((tab) => tab.id === activeTab);
    if (index >= 0) setTabIndex(index);
  }, [activeTab]);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        void getCurrentWindow().hide();
        return;
      }
      if (e.ctrlKey && !e.altKey && !e.shiftKey && e.key >= "1" && e.key <= "5") {
        const tab = TABS[Number(e.key) - 1];
        if (tab) {
          e.preventDefault();
          setActiveTab(tab.id);
        }
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [setActiveTab]);

  return (
    <div className="panel flex flex-col text-white">
      <TopBar tabIndex={tabIndex} onRequestFocusTab={setTabIndex} />
      <main key={activeTab} className="koyun-scroll tab-in min-h-0 flex-1 overflow-y-auto">
        {renderTab(activeTab)}
      </main>
    </div>
  );
}

export default App;
