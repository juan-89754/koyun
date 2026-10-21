import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { TabId } from "../lib/tabs";

interface UiState {
  activeTab: TabId;
  setActiveTab: (tab: TabId) => void;
}

export const useUiStore = create<UiState>()(
  persist(
    (set) => ({
      activeTab: "controls",
      setActiveTab: (activeTab) => set({ activeTab }),
    }),
    { name: "koyun-ui" },
  ),
);
