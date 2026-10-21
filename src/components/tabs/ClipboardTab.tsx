import { invoke } from "@tauri-apps/api/core";
import { useCallback, useState } from "react";
import { ClipboardIcon, ImageIcon, TextIcon, TrashIcon } from "../icons";
import { relativeTime } from "../../lib/format";
import { useErrorState } from "../../lib/errors";
import { usePolling } from "../../lib/usePolling";

interface ClipItem {
  id: string;
  when_ms: number | null;
  kind: string;
  text: string | null;
  image: string | null;
}

interface ClipState {
  available: boolean;
  reason: string | null;
  items: ClipItem[];
}

export function ClipboardTab() {
  const [state, setState] = useState<ClipState | null>(null);
  const [busy, setBusy] = useState(false);
  const { error, setError, reportError } = useErrorState("portapapeles");

  const load = useCallback(() => {
    invoke<ClipState>("clipboard_history")
      .then((s) => {
        setError(null);
        setState(s);
      })
      .catch(reportError);
  }, [reportError]);

  usePolling(load, 3000);

  const restore = useCallback(async (id: string) => {
    try {
      await invoke("clipboard_restore", { id });
    } catch (e) {
      reportError(e);
    }
  }, [reportError]);

  const remove = useCallback(async (id: string) => {
    setState((s) => (s ? { ...s, items: s.items.filter((item) => item.id !== id) } : s));
    try {
      await invoke("clipboard_delete", { id });
    } catch (e) {
      reportError(e);
    }
  }, [reportError]);

  const clearAll = useCallback(async () => {
    setBusy(true);
    try {
      await invoke("clipboard_clear");
      load();
    } catch (e) {
      reportError(e);
    } finally {
      setBusy(false);
    }
  }, [load, reportError]);

  const items = state?.items ?? [];
  const unavailable = state !== null && !state.available;

  return (
    <div
      id="panel-clipboard"
      role="tabpanel"
      aria-labelledby="tab-clipboard"
      className="flex h-full flex-col gap-4 px-8 py-5"
    >
      <div className="flex shrink-0 items-center gap-3">
        <ClipboardIcon className="h-4 w-4 text-teal-300" />
        <span className="text-sm font-medium text-slate-200">Portapapeles</span>
        <span className="rounded-full border border-white/12 bg-white/6 px-2 py-0.5 text-[10px] font-medium text-white/55">
          {items.length}
        </span>
        <button
          type="button"
          disabled={busy || items.length === 0}
          onClick={() => void clearAll()}
          className="ml-auto flex items-center gap-1.5 rounded-md border border-white/10 bg-white/5 px-2.5 py-1.5 text-xs text-slate-300 transition-colors hover:border-red-400/40 hover:bg-red-500/15 hover:text-red-200 disabled:opacity-40"
        >
          <TrashIcon className="h-3.5 w-3.5" />
          Vaciar todo
        </button>
      </div>

      {unavailable ? (
        <section className="flex flex-1 flex-col items-center justify-center gap-2 text-center">
          <p className="text-sm font-medium text-white/85">
            {state?.reason === "disabled"
              ? "El historial del portapapeles está desactivado"
              : "Koyun no tiene acceso al portapapeles"}
          </p>
          <p className="max-w-sm text-xs leading-5 text-white/50">
            {state?.reason === "disabled"
              ? "Actívalo en Configuración › Sistema › Portapapeles para poder consultar y restaurar elementos."
              : "Revisa los permisos del historial del portapapeles en Windows."}
          </p>
        </section>
      ) : (
        <div className="koyun-scroll grid min-h-0 flex-1 grid-cols-2 content-start gap-2 overflow-y-auto pr-1">
          {state && items.length === 0 && (
            <p className="col-span-2 pt-10 text-center text-xs text-white/40">
              El historial del portapapeles está vacío.
            </p>
          )}
          {items.map((item) => (
            <article
              key={item.id}
              className="group flex items-start gap-3 rounded-xl border border-white/10 bg-white/5 px-3.5 py-3 transition-colors hover:border-teal-400/40 hover:bg-teal-500/8"
            >
              {item.kind === "image" && item.image ? (
                <img
                  src={item.image}
                  alt="Contenido del portapapeles"
                  className="h-14 w-14 shrink-0 rounded-lg border border-white/10 object-cover"
                />
              ) : (
                <div className="flex h-14 w-14 shrink-0 items-center justify-center rounded-lg border border-white/10 bg-white/5 text-white/35">
                  {item.kind === "image" ? (
                    <ImageIcon className="h-4.5 w-4.5" />
                  ) : (
                    <TextIcon className="h-4.5 w-4.5" />
                  )}
                </div>
              )}

              <div className="min-w-0 flex-1">
                <p className="line-clamp-3 text-xs leading-5 text-slate-200">
                  {item.kind === "image" ? "Imagen" : (item.text?.trim() || "Sin texto")}
                </p>
                <p className="mt-1 text-[10px] text-white/35">{relativeTime(item.when_ms)}</p>
              </div>

              <div className="flex shrink-0 flex-col gap-1 opacity-0 transition-opacity group-hover:opacity-100">
                <button
                  type="button"
                  onClick={() => void restore(item.id)}
                  className="rounded-md border border-white/10 bg-white/5 px-2 py-1 text-[10px] font-medium text-slate-300 transition-colors hover:border-teal-400/40 hover:bg-teal-500/15 hover:text-teal-100"
                >
                  Usar
                </button>
                <button
                  type="button"
                  aria-label="Eliminar del historial"
                  onClick={() => void remove(item.id)}
                  className="rounded-md border border-white/10 bg-white/5 p-1 text-white/45 transition-colors hover:border-red-400/40 hover:bg-red-500/15 hover:text-red-200"
                >
                  <TrashIcon className="h-3 w-3" />
                </button>
              </div>
            </article>
          ))}
        </div>
      )}

      {error && <p className="shrink-0 text-center text-xs leading-5 text-red-300/90">{error}</p>}
    </div>
  );
}
