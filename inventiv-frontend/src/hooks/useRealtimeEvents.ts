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
    if (window.__inventivSse) return;

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

    // If the connection errors, close it so we can recreate on next mount.
    es.onerror = () => {
      try {
        es.close();
      } catch {
        // ignore
      }
      window.__inventivSse = undefined;
    };

    return () => {
      // Keep the connection singleton; do not close on unmount.
      es.removeEventListener("instance.updated", onInstanceUpdated);
      es.removeEventListener("action_log.created", onActionLogCreated);
    };
  }, []);
}


