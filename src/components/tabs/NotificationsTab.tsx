import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Bell, ExternalLink, RefreshCw } from "lucide-react";
import { useGameModeStore } from "../../store/gameMode";

interface NotificationItem {
  id: number;
  app: string;
  title: string;
  body: string;
  time_ms: number;
}

function formatTime(ts: number): string {
  if (!ts) return "";
  const d = new Date(ts);
  const now = new Date();
  const sameDay = d.toDateString() === now.toDateString();
  if (sameDay) {
    return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  }
  return d.toLocaleString([], { day: "2-digit", month: "2-digit", hour: "2-digit", minute: "2-digit" });
}

export default function NotificationsTab() {
  const [items, setItems] = useState<NotificationItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function load() {
    setLoading(true);
    setError(null);
    try {
      const res = await invoke<NotificationItem[]>("notification_list");
      setItems(res ?? []);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void load();
    const t = setInterval(() => {
      if (useGameModeStore.getState().enabled) return;
      void load();
    }, 8000);
    return () => clearInterval(t);
  }, []);

  const hasItems = items.length > 0;

  async function openSettings() {
    try {
      await invoke("notification_open_settings");
    } catch (e) {
      setError(String(e));
    }
  }

  const content = useMemo(() => {
    if (loading && !hasItems) {
      return (
        <div className="flex h-40 items-center justify-center text-[color:var(--text-sub)]">
          <RefreshCw className="mr-2 h-4 w-4 animate-spin" />
          Cargando...
        </div>
      );
    }
    if (!hasItems) {
      return (
        <div className="flex h-40 flex-col items-center justify-center gap-2 text-center text-[color:var(--text-sub)]">
          <Bell className="h-5 w-5" />
          <div>Sin notificaciones recientes</div>
          <button
            className="mt-2 flex items-center gap-1 rounded-md border border-[color:var(--border)] px-2 py-1 text-xs hover:bg-[color:var(--surface-2)]"
            onClick={openSettings}
          >
            <ExternalLink className="h-3 w-3" />
            Ajustes de notificaciones
          </button>
        </div>
      );
    }
    return (
      <div className="flex flex-col divide-y divide-[color:var(--border)]">
        {items.map((n) => (
          <div key={n.id} className="flex flex-col gap-1 px-3 py-2">
            <div className="flex items-center justify-between">
              <div className="text-sm font-medium text-[color:var(--text)]">{n.title || n.app}</div>
              <div className="text-xs text-[color:var(--text-sub)]">{formatTime(n.time_ms)}</div>
            </div>
            {n.title && n.body && (
              <div className="text-xs text-[color:var(--text-sub)]">{n.body}</div>
            )}
            {!n.title && n.body && (
              <div className="text-xs text-[color:var(--text-sub)]">{n.body}</div>
            )}
            {n.title && !n.body && (
              <div className="text-xs text-[color:var(--text-sub)]">{n.app}</div>
            )}
          </div>
        ))}
      </div>
    );
  }, [items, loading, hasItems]);

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center justify-between border-b border-[color:var(--border)] px-3 py-2">
        <div className="text-sm font-medium text-[color:var(--text)]">Notificaciones</div>
        <div className="flex items-center gap-2">
          <button
            className="rounded-md p-1 text-[color:var(--text-sub)] hover:bg-[color:var(--surface-2)] hover:text-[color:var(--text)]"
            onClick={load}
            title="Actualizar"
          >
            <RefreshCw className={`h-4 w-4 ${loading ? "animate-spin" : ""}`} />
          </button>
          <button
            className="rounded-md p-1 text-[color:var(--text-sub)] hover:bg-[color:var(--surface-2)] hover:text-[color:var(--text)]"
            onClick={openSettings}
            title="Ajustes de Windows"
          >
            <ExternalLink className="h-4 w-4" />
          </button>
        </div>
      </div>
      {error && <div className="px-3 py-1 text-xs text-red-400">{error}</div>}
      <div className="flex-1 overflow-auto">{content}</div>
    </div>
  );
}
