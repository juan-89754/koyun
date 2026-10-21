import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";
import { createRoot } from "react-dom/client";
import "../styles.css";

/**
 * Ventana "wallpaper": se coloca detrás de los iconos del escritorio (hijo del
 * WorkerW) y reproduce el vídeo en bucle. No recibe el cursor, así que nunca
 * estorba al usuario aunque la inserción falle.
 */
function Wallpaper() {
  const videoRef = useRef<HTMLVideoElement>(null);

  const play = (path: string | null) => {
    const video = videoRef.current;
    if (!video) return;
    if (!path) {
      video.pause();
      video.removeAttribute("src");
      video.load();
      return;
    }
    video.src = convertFileSrc(path);
    video.loop = true;
    video.muted = true;
    void video.play().catch(() => {});
  };

  useEffect(() => {
    void invoke<string | null>("wallpaper_video_state").then(play).catch(() => {});
    const unlisten = listen<string | null>("wallpaper:video", (event) => play(event.payload));
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  return (
    <video
      ref={videoRef}
      autoPlay
      loop
      muted
      playsInline
      className="h-screen w-screen bg-black object-cover"
    />
  );
}

createRoot(document.getElementById("wallpaper-root")!).render(<Wallpaper />);
