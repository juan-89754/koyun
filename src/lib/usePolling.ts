import { useEffect, useRef } from "react";
import { useGameModeStore } from "../store/gameMode";

/**
 * Sondeo periódico que:
 *
 * - no apila peticiones: espera a que termine la anterior antes de lanzar otra;
 * - se pausa con la ventana oculta y se reanuda en cuanto vuelve a estar visible.
 *
 * Sin estas dos garantías los cinco sondeos del panel seguían corriendo con el
 * escritorio oculto y se acumulaban si alguna llamada tardaba más que su
 * intervalo (histórico de portapapeles, carátula de multimedia, WMI…).
 *
 * `task` puede devolver `void` o una promesa.
 */
export function usePolling(task: () => unknown, intervalMs: number) {
  const taskRef = useRef(task);
  taskRef.current = task;

  useEffect(() => {
    let busy = false;

    const tick = () => {
      // En Modo Juego se suspenden los sondeos para liberar CPU.
      if (busy || document.hidden || useGameModeStore.getState().enabled) return;
      busy = true;
      try {
        void Promise.resolve(taskRef.current()).finally(() => {
          busy = false;
        });
      } catch (e) {
        busy = false;
        console.error("[Koyun] error en sondeo", e);
      }
    };

    const onVisibility = () => {
      if (!document.hidden) tick();
    };

    tick();
    const id = window.setInterval(tick, intervalMs);
    document.addEventListener("visibilitychange", onVisibility);

    return () => {
      window.clearInterval(id);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [intervalMs]);
}
