import { useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import islandLogo from "./logo_isla.png";
import "../styles.css";

function Island() {
  const [hovered, setHovered] = useState(false);
  const hideTimer = useRef<number | null>(null);

  const clearHide = () => {
    if (hideTimer.current !== null) {
      window.clearTimeout(hideTimer.current);
      hideTimer.current = null;
    }
  };

  const scheduleHide = () => {
    clearHide();
    hideTimer.current = window.setTimeout(() => {
      setHovered(false);
      void getCurrentWindow().hide();
    }, 360);
  };

  const toggleMain = async () => {
    try {
      await invoke("island_toggle_main");
    } catch (e) {
      console.error(e);
    }
  };

  useEffect(() => {
    clearHide();
    return () => clearHide();
  }, []);

  return (
    <div
      className="flex h-full w-full items-center justify-center"
      onMouseEnter={() => {
        clearHide();
        setHovered(true);
      }}
      onMouseLeave={() => scheduleHide()}
    >
      <button
        type="button"
        onClick={() => void toggleMain()}
        className={`relative flex items-center justify-center overflow-hidden rounded-full border transition-all duration-200 ${
          hovered
            ? "h-10 w-10 border-white/15 bg-zinc-900 shadow-[0_0_40px_rgba(45,212,191,0.25)] scale-100"
            : "h-3.5 w-9 border-white/20 bg-zinc-900/90 shadow-[0_1px_10px_rgba(0,0,0,0.55)]"
        }`}
        aria-label="Abrir Koyun"
      >
        {hovered ? (
          <img
            src={islandLogo}
            alt="Koyun"
            draggable={false}
            className="h-8 w-8 rounded-full object-cover"
          />
        ) : (
          <span className="h-1.5 w-1.5 rounded-full bg-white/80 shadow-[0_0_8px_rgba(255,255,255,0.45)]" />
        )}
      </button>
    </div>
  );
}

createRoot(document.getElementById("island-root")!).render(<Island />);
