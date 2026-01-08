"use client";

import * as React from "react";
import { CheckCircle2, Info, AlertTriangle, XCircle, X, Copy, Check } from "lucide-react";
import { Button } from "ia-designsys";
import { cn } from "ia-designsys";

export type IASnackbarTone = "success" | "info" | "warning" | "error";

export type IASnackbarItem = {
  id: string;
  tone: IASnackbarTone;
  title?: string;
  message: string;
  /**
   * Optional long details (stacktrace, backend payload, etc.).
   * Shown in the UI and included in the copy payload.
   */
  details?: string;
  /**
   * Override copied text. If omitted, we build a useful payload from message/details.
   */
  copyText?: string;
  /**
   * Auto close after N ms. If 0/undefined for error => sticky (requires manual close).
   */
  durationMs?: number;
  createdAtMs: number;
};

type ShowArgs = Omit<IASnackbarItem, "id" | "createdAtMs"> & { id?: string };

type IASnackbarApi = {
  show: (snack: ShowArgs) => void;
  success: (message: string, opts?: Partial<Omit<ShowArgs, "tone" | "message">>) => void;
  info: (message: string, opts?: Partial<Omit<ShowArgs, "tone" | "message">>) => void;
  warning: (message: string, opts?: Partial<Omit<ShowArgs, "tone" | "message">>) => void;
  error: (message: string, opts?: Partial<Omit<ShowArgs, "tone" | "message">>) => void;
  dismiss: (id: string) => void;
  clear: () => void;
};

const IASnackbarContext = React.createContext<IASnackbarApi | null>(null);

function defaultDurationMs(tone: IASnackbarTone): number {
  switch (tone) {
    case "success":
      return 3500;
    case "info":
      return 4000;
    case "warning":
      return 6000;
    case "error":
      return 0; // sticky
  }
}

function toneIcon(tone: IASnackbarTone) {
  switch (tone) {
    case "success":
      return CheckCircle2;
    case "info":
      return Info;
    case "warning":
      return AlertTriangle;
    case "error":
      return XCircle;
  }
}

function toneClasses(tone: IASnackbarTone) {
  switch (tone) {
    case "success":
      return "border-green-200 bg-green-50 text-green-900";
    case "info":
      return "border-blue-200 bg-blue-50 text-blue-900";
    case "warning":
      return "border-amber-200 bg-amber-50 text-amber-900";
    case "error":
      return "border-red-200 bg-red-50 text-red-900";
  }
}

function buildCopyPayload(snack: IASnackbarItem): string {
  if (snack.copyText) return snack.copyText;
  const payload = {
    tone: snack.tone,
    title: snack.title ?? null,
    message: snack.message,
    details: snack.details ?? null,
    created_at: new Date(snack.createdAtMs).toISOString(),
  };
  return JSON.stringify(payload, null, 2);
}

export function IASnackbarProvider({ children }: { children: React.ReactNode }) {
  const [items, setItems] = React.useState<IASnackbarItem[]>([]);
  const timersRef = React.useRef<Map<string, number>>(new Map());
  const seqRef = React.useRef(0);
  const [copiedId, setCopiedId] = React.useState<string | null>(null);

  const dismiss = React.useCallback((id: string) => {
    const t = timersRef.current.get(id);
    if (t) {
      window.clearTimeout(t);
      timersRef.current.delete(id);
    }
    setItems((prev) => prev.filter((x) => x.id !== id));
    setCopiedId((c) => (c === id ? null : c));
  }, []);

  const clear = React.useCallback(() => {
    for (const t of timersRef.current.values()) window.clearTimeout(t);
    timersRef.current.clear();
    setItems([]);
    setCopiedId(null);
  }, []);

  const show = React.useCallback(
    (snack: ShowArgs) => {
      const id = snack.id ?? `${Date.now().toString(36)}-${(++seqRef.current).toString(36)}`;
      const createdAtMs = Date.now();
      const durationMs = snack.durationMs ?? defaultDurationMs(snack.tone);
      const next: IASnackbarItem = {
        id,
        tone: snack.tone,
        title: snack.title,
        message: snack.message,
        details: snack.details,
        copyText: snack.copyText,
        durationMs,
        createdAtMs,
      };

      setItems((prev) => {
        // cap queue size (prevent runaway)
        const capped = prev.length >= 4 ? prev.slice(prev.length - 3) : prev;
        return [...capped, next];
      });

      if (durationMs && durationMs > 0) {
        const t = window.setTimeout(() => dismiss(id), durationMs);
        timersRef.current.set(id, t);
      }
    },
    [dismiss]
  );

  const api = React.useMemo<IASnackbarApi>(
    () => ({
      show,
      success: (message, opts) => show({ tone: "success", message, title: opts?.title ?? "OK", ...opts }),
      info: (message, opts) => show({ tone: "info", message, title: opts?.title ?? "Info", ...opts }),
      warning: (message, opts) => show({ tone: "warning", message, title: opts?.title ?? "Attention", ...opts }),
      error: (message, opts) => show({ tone: "error", message, title: opts?.title ?? "Erreur", ...opts }),
      dismiss,
      clear,
    }),
    [dismiss, clear, show]
  );

  const onCopy = React.useCallback(async (snack: IASnackbarItem) => {
    try {
      await navigator.clipboard.writeText(buildCopyPayload(snack));
      setCopiedId(snack.id);
      window.setTimeout(() => setCopiedId((c) => (c === snack.id ? null : c)), 1200);
    } catch {
      // ignore (clipboard might be blocked); user can still select text.
    }
  }, []);

  return (
    <IASnackbarContext.Provider value={api}>
      {children}

      {/* Full-width bottom snackbar rail (requested) */}
      <div className="fixed inset-x-0 bottom-0 z-[1000] flex flex-col gap-2 pointer-events-none p-3">
        {items.map((snack) => {
          const Icon = toneIcon(snack.tone);
          const isError = snack.tone === "error";
          const copied = copiedId === snack.id;
          return (
            <div
              key={snack.id}
              role="status"
              aria-live={snack.tone === "error" ? "assertive" : "polite"}
              className={cn(
                "pointer-events-auto border shadow-sm p-3 w-full",
                "backdrop-blur supports-[backdrop-filter]:bg-opacity-90",
                toneClasses(snack.tone)
              )}
            >
              <div className="flex gap-3">
                <div className="pt-0.5">
                  <Icon className="h-4 w-4" />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="flex items-start justify-between gap-2">
                    <div className="min-w-0">
                      {snack.title ? <div className="text-sm font-semibold leading-5">{snack.title}</div> : null}
                      <div className="text-sm leading-5 break-words">{snack.message}</div>
                    </div>
                    <button
                      type="button"
                      onClick={() => dismiss(snack.id)}
                      className="shrink-0 rounded-md p-1 hover:bg-black/5"
                      title="Fermer"
                    >
                      <X className="h-4 w-4" />
                    </button>
                  </div>
                  {snack.details ? (
                    <pre className="mt-2 text-[11px] leading-4 whitespace-pre-wrap break-words bg-white/40 border border-black/5 rounded-md p-2 max-h-40 overflow-auto">
                      {snack.details}
                    </pre>
                  ) : null}

                  {isError ? (
                    <div className="mt-2 flex justify-end">
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={() => void onCopy(snack)}
                        title="Copier le message d’erreur (pour support/admin)"
                      >
                        {copied ? <Check className="h-4 w-4 mr-2" /> : <Copy className="h-4 w-4 mr-2" />}
                        {copied ? "Copié" : "Copier"}
                      </Button>
                    </div>
                  ) : null}
                </div>
              </div>
            </div>
          );
        })}
      </div>
    </IASnackbarContext.Provider>
  );
}

export function useSnackbar(): IASnackbarApi {
  const ctx = React.useContext(IASnackbarContext);
  if (!ctx) {
    throw new Error("useSnackbar must be used within IASnackbarProvider");
  }
  return ctx;
}


