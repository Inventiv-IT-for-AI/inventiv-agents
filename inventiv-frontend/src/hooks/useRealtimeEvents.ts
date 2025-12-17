import { useEffect } from "react";
import { apiUrl } from "@/lib/api";

declare global {
  interface Window {
    __inventivSse?: EventSource;
  }
}

/**
 * Opens a singleton SSE connection (server -> UI) and broadcasts refresh events:
 * - `refresh-instances` when instances change
 * - `refresh-action-logs` when action logs change
 *
 * This keeps the scope small: existing pages/hooks decide how to refetch.
 */
export function useRealtimeEvents() {
  useEffect(() => {
    if (typeof window === "undefined") return;
    // If we already have a connection, keep it. If it's CLOSED, recreate it.
    if (window.__inventivSse && window.__inventivSse.readyState !== EventSource.CLOSED) return;

    let reconnectTimer: number | undefined;

    const create = () => {
      const es = new EventSource(apiUrl("events/stream?topics=instances,actions"));
      window.__inventivSse = es;

      const onInstanceUpdated = () => {
        window.dispatchEvent(new Event("refresh-instances"));
      };
      const onActionLogCreated = () => {
        window.dispatchEvent(new Event("refresh-action-logs"));
      };

      es.addEventListener("instance.updated", onInstanceUpdated);
      es.addEventListener("action_log.created", onActionLogCreated);

      // IMPORTANT:
      // Do NOT call `es.close()` on error. EventSource handles reconnection by itself.
      // We only recreate the object if it ends up in CLOSED state (e.g. proxy/server ended stream).
      es.onerror = () => {
        if (reconnectTimer) window.clearTimeout(reconnectTimer);
        reconnectTimer = window.setTimeout(() => {
          if (window.__inventivSse?.readyState === EventSource.CLOSED) {
            try {
              window.__inventivSse?.close();
            } catch {
              // ignore
            }
            window.__inventivSse = undefined;
            create();
          }
        }, 1000);
      };

      return () => {
        es.removeEventListener("instance.updated", onInstanceUpdated);
        es.removeEventListener("action_log.created", onActionLogCreated);
      };
    };

    const cleanupListeners = create();

    return () => {
      // Keep the connection singleton; do not close on unmount.
      if (reconnectTimer) window.clearTimeout(reconnectTimer);
      cleanupListeners?.();
    };
  }, []);
}


