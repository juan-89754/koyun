import { invoke } from "@tauri-apps/api/core";
import { confirm } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { LockIcon, MoonIcon, PowerIcon, RotateCcwIcon } from "../icons";
import { useErrorState } from "../../lib/errors";

type ActionId = "lock" | "suspend" | "restart" | "shutdown";

interface Action {
  id: ActionId;
  label: string;
  Icon: typeof LockIcon;
  cardClass: string;
  confirm?: { title: string; message: string; okLabel: string };
}

const ACTIONS: Action[] = [
  {
    id: "lock",
    label: "Bloquear",
    Icon: LockIcon,
    cardClass:
      "hover:bg-blue-500/10 hover:border-blue-400/40 hover:text-blue-200",
  },
  {
    id: "suspend",
    label: "Suspender",
    Icon: MoonIcon,
    cardClass:
      "hover:bg-indigo-500/10 hover:border-indigo-400/40 hover:text-indigo-200",
  },
  {
    id: "restart",
    label: "Reiniciar",
    Icon: RotateCcwIcon,
    cardClass:
      "hover:bg-amber-500/10 hover:border-amber-400/40 hover:text-amber-200",
    confirm: {
      title: "Reiniciar el equipo",
      message: "¿Reiniciar el equipo ahora? Se cerrarán todas las aplicaciones.",
      okLabel: "Reiniciar",
    },
  },
  {
    id: "shutdown",
    label: "Apagar",
    Icon: PowerIcon,
    cardClass: "hover:bg-red-500/20 hover:border-red-500/40 hover:text-red-200",
    confirm: {
      title: "Apagar el equipo",
      message: "¿Apagar el equipo ahora? Se cerrarán todas las aplicaciones.",
      okLabel: "Apagar",
    },
  },
];

export function PowerTab() {
  const [busy, setBusy] = useState<ActionId | null>(null);
  const { error, setError } = useErrorState("energía");

  async function run(action: Action) {
    if (busy) return;
    setError(null);

    if (action.confirm) {
      const ok = await confirm(action.confirm.message, {
        title: action.confirm.title,
        kind: "warning",
        okLabel: action.confirm.okLabel,
        cancelLabel: "Cancelar",
      });
      if (!ok) return;
    }

    setBusy(action.id);
    try {
      await invoke(`power_${action.id}`);
    } catch (e) {
      setError(typeof e === "string" ? e : JSON.stringify(e));
    } finally {
      setBusy(null);
    }
  }

  return (
    <div
      id="panel-power"
      role="tabpanel"
      aria-labelledby="tab-power"
      className="flex h-full flex-col items-center justify-center gap-4 px-8"
    >
      <div className="grid grid-cols-2 gap-4">
        {ACTIONS.map((action) => (
          <button
            key={action.id}
            type="button"
            disabled={busy !== null}
            onClick={() => run(action)}
            className={`group flex h-28 w-44 flex-col items-center justify-center gap-3 rounded-xl border border-white/10 bg-white/5 text-slate-300 transition-all duration-150 hover:scale-[1.02] hover:bg-white/10 hover:text-white disabled:cursor-not-allowed disabled:opacity-50 ${action.cardClass}`}
          >
            <action.Icon className="h-6 w-6" strokeWidth={1.6} />
            <span className="text-sm font-medium text-slate-200">
              {action.label}
            </span>
          </button>
        ))}
      </div>
      {error && (
        <p className="max-w-md text-center text-xs leading-5 text-red-300/90">
          {error}
        </p>
      )}
    </div>
  );
}
