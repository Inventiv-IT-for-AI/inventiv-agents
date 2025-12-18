import type { LucideIcon } from "lucide-react";
import { Server, Zap, Cloud, Database, Archive, AlertTriangle, Clock, CheckCircle, RefreshCcw, Copy, Check } from "lucide-react";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useCallback, useEffect, useMemo, useState } from "react";
import { apiUrl } from "@/lib/api";
import type { ActionLog, ActionType, Instance } from "@/lib/types";
import type { LoadRangeResult } from "@/components/shared/VirtualizedRemoteList";
import { CopyButton } from "@/components/shared/CopyButton";
import { displayOrDash } from "@/lib/utils";
import { VirtualizedDataTable, type DataTableColumn } from "@/components/shared/VirtualizedDataTable";
import { useRealtimeEvents } from "@/hooks/useRealtimeEvents";

interface InstanceTimelineModalProps {
  open: boolean;
  onClose: () => void;
  instanceId: string;
}

export function InstanceTimelineModal({
  open,
  onClose,
  instanceId,
}: InstanceTimelineModalProps) {
  useRealtimeEvents();
  const [instance, setInstance] = useState<Instance | null>(null);
  const [actionTypes, setActionTypes] = useState<Record<string, ActionType>>({});
  const [selectedLog, setSelectedLog] = useState<ActionLog | null>(null);
  const [counts, setCounts] = useState({ total: 0, filtered: 0 });
  const [refreshToken, setRefreshToken] = useState(0);
  const [actionsRefreshSeq, setActionsRefreshSeq] = useState(0);
  const [copyingActions, setCopyingActions] = useState(false);
  const [copiedActions, setCopiedActions] = useState(false);
  const [recentLogs, setRecentLogs] = useState<ActionLog[]>([]);

  useEffect(() => {
    if (open && instanceId) {
      setSelectedLog(null);
      void fetchActionTypes();
      void fetchInstance();
      void fetchRecentLogs();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, instanceId, refreshToken]);

  useEffect(() => {
    const handler = () => setActionsRefreshSeq((v) => v + 1);
    window.addEventListener("refresh-action-logs", handler);
    return () => window.removeEventListener("refresh-action-logs", handler);
  }, []);

  const fetchActionTypes = useCallback(async () => {
    try {
      const response = await fetch(apiUrl(`action_types`));
      const data: ActionType[] = await response.json();
      const map: Record<string, ActionType> = {};
      for (const at of data) map[at.code] = at;
      setActionTypes(map);
    } catch (error) {
      console.error("Failed to fetch action types:", error);
    }
  }, []);

  const fetchInstance = useCallback(async () => {
    try {
      const response = await fetch(apiUrl(`instances/${instanceId}`));
      if (!response.ok) {
        setInstance(null);
        return;
      }
      const data: Instance = await response.json();
      setInstance(data);
    } catch (error) {
      console.error("Failed to fetch instance details:", error);
      setInstance(null);
    }
  }, [instanceId]);

  const fetchRecentLogs = useCallback(async () => {
    try {
      const params = new URLSearchParams();
      params.set("offset", "0");
      params.set("limit", "200");
      params.set("instance_id", instanceId);
      const res = await fetch(apiUrl(`action_logs/search?${params.toString()}`));
      if (!res.ok) {
        setRecentLogs([]);
        return;
      }
      const data = (await res.json()) as { rows: ActionLog[] };
      setRecentLogs(Array.isArray(data?.rows) ? data.rows : []);
    } catch {
      setRecentLogs([]);
    }
  }, [instanceId]);

  const getActionIcon = (actionType: string) => {
    const iconMap: Record<string, LucideIcon> = {
      Activity: Server,
      AlertTriangle,
      Archive,
      CheckCircle,
      Clock,
      Cloud,
      Database,
      Server,
      Zap,
    };
    const def = actionTypes[actionType];
    return iconMap[def?.icon || "Activity"] || Server;
  };

  const getCategoryDotClass = (actionType: string) => {
    const def = actionTypes[actionType];
    const cat = def?.category || "";
    if (cat === "create") return "bg-orange-500";
    if (cat === "terminate") return "bg-red-500";
    if (cat === "health") return "bg-teal-600";
    if (cat === "archive") return "bg-gray-600";
    if (cat === "reconcile") return "bg-yellow-600";
    return "bg-slate-400";
  };

  const formatActionLabel = (actionType: string) =>
    actionTypes[actionType]?.label ??
    actionType
      .toLowerCase()
      .replace(/_/g, " ")
      .replace(/\b\w/g, (l) => l.toUpperCase());

  const formatDuration = (ms: number | null) => {
    if (!ms) return "-";
    if (ms < 1000) return `${ms}ms`;
    if (ms < 60000) return `${(ms / 1000).toFixed(2)}s`;
    return `${(ms / 60000).toFixed(2)}min`;
  };

  const formatTimestamp = (dateString?: string | null) => {
    if (!dateString) return "-";
    const d = new Date(dateString);
    if (Number.isNaN(d.getTime())) return "-";
    return d.toLocaleString("fr-FR", { year: "numeric", month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" });
  };

  const lastPing = useMemo(() => {
    const a = instance?.last_reconciliation ? new Date(instance.last_reconciliation).getTime() : 0;
    const b = instance?.last_health_check ? new Date(instance.last_health_check).getTime() : 0;
    const t = Math.max(a, b);
    return t ? new Date(t).toISOString() : null;
  }, [instance?.last_health_check, instance?.last_reconciliation]);

  const vllmMode = useMemo(() => {
    // Prefer metadata.vllm_mode from the latest WORKER_SSH_INSTALL action.
    for (const log of recentLogs) {
      if (log.action_type === "WORKER_SSH_INSTALL") {
        const m = (log.metadata as any)?.vllm_mode;
        if (typeof m === "string" && m.trim()) return m.trim();
      }
    }
    return "mono";
  }, [recentLogs]);

  const readiness = useMemo(() => {
    const getStatus = (actionType: string) => {
      const l = recentLogs.find((x) => x.action_type === actionType);
      return l?.status ?? null;
    };
    return {
      vllmHttp: getStatus("WORKER_VLLM_HTTP_OK"),
      modelLoaded: getStatus("WORKER_MODEL_LOADED"),
      warmup: getStatus("WORKER_VLLM_WARMUP"),
    };
  }, [recentLogs]);

  type ActionLogsSearchResponse = {
    offset: number;
    limit: number;
    total_count: number;
    filtered_count: number;
    rows: ActionLog[];
  };

  const queryKey = useMemo(
    () => JSON.stringify({ instanceId, refreshToken, actionsRefreshSeq }),
    [instanceId, refreshToken, actionsRefreshSeq],
  );

  const handleCountsChange = useCallback(
    ({ total, filtered }: { total: number; filtered: number }) => {
      // For this modal, the only "filter" is instance_id: total displayed is per-instance.
      setCounts({ total, filtered });
    },
    []
  );

  const loadRange = useCallback(
    async (offset: number, limit: number): Promise<LoadRangeResult<ActionLog>> => {
      const params = new URLSearchParams();
      params.set("offset", String(offset));
      params.set("limit", String(limit));
      params.set("instance_id", instanceId);

      const res = await fetch(apiUrl(`action_logs/search?${params.toString()}`));
      const data: ActionLogsSearchResponse = await res.json();

      return {
        offset: data.offset,
        items: data.rows,
        totalCount: data.total_count,
        filteredCount: data.filtered_count,
      };
    },
    [instanceId]
  );

  const columns = useMemo<DataTableColumn<ActionLog>[]>(() => {
    return [
      {
        id: "time",
        label: "Heure",
        width: 110,
        cell: ({ row }) => (
          <span className="font-mono text-xs text-muted-foreground whitespace-nowrap">
            {new Date(row.created_at).toLocaleTimeString("fr-FR", { hour: "2-digit", minute: "2-digit", second: "2-digit" })}
          </span>
        ),
      },
      {
        id: "action",
        label: "Action",
        width: 420,
        cell: ({ row }) => {
          const actionLabel = formatActionLabel(row.action_type);
          const dotClass = getCategoryDotClass(row.action_type);
          return (
            <div className="min-w-0 flex items-center gap-2">
              <span className={`h-2 w-2 rounded-full ${dotClass}`} />
              <span className="truncate font-medium">{actionLabel}</span>
              {row.error_message ? <span className="ml-auto text-xs text-red-600">Erreur</span> : null}
            </div>
          );
        },
      },
      {
        id: "status",
        label: "Statut",
        width: 140,
        cell: ({ row }) => (
          <Badge variant="outline" className="text-[11px]">
            {row.status}
          </Badge>
        ),
      },
      {
        id: "duration",
        label: "Durée",
        width: 110,
        align: "right",
        cell: ({ row }) => <span className="font-mono text-xs text-muted-foreground">{formatDuration(row.duration_ms)}</span>,
      },
      {
        id: "transition",
        label: "Transition",
        width: 220,
        cellClassName: "truncate",
        cell: ({ row }) => (
          <span className="font-mono text-[11px] text-muted-foreground truncate">
            {(row.instance_status_before || row.instance_status_after)
              ? `${row.instance_status_before ?? "-"} → ${row.instance_status_after ?? "-"}`
              : "-"}
          </span>
        ),
      },
    ];
  }, [actionTypes]);

  const fetchAllActions = useCallback(async (): Promise<ActionLog[]> => {
    const allActions: ActionLog[] = [];
    let offset = 0;
    const limit = 1000; // Utiliser une limite élevée pour récupérer toutes les actions en une fois
    let hasMore = true;

    while (hasMore) {
      const params = new URLSearchParams();
      params.set("offset", String(offset));
      params.set("limit", String(limit));
      params.set("instance_id", instanceId);

      const res = await fetch(apiUrl(`action_logs/search?${params.toString()}`));
      const data: ActionLogsSearchResponse = await res.json();

      allActions.push(...data.rows);

      if (data.rows.length < limit || offset + data.rows.length >= data.filtered_count) {
        hasMore = false;
      } else {
        offset += limit;
      }
    }

    return allActions;
  }, [instanceId]);

  const copyAllActionsToClipboard = useCallback(async () => {
    if (copyingActions) return;

    setCopyingActions(true);
    try {
      const allActions = await fetchAllActions();
      
      // Créer un objet avec les informations de l'instance et toutes les actions
      const dataToCopy = {
        instance: {
          id: instanceId,
          status: instance?.status,
          created_at: instance?.created_at,
          region: instance?.region,
          zone: instance?.zone,
          instance_type: instance?.instance_type,
          ip_address: instance?.ip_address,
          provider_instance_id: instance?.provider_instance_id,
          gpu_count: instance?.gpu_count,
          gpu_vram: instance?.gpu_vram,
        },
        actions: allActions.map((action) => ({
          id: action.id,
          action_type: action.action_type,
          component: action.component,
          status: action.status,
          provider_name: action.provider_name,
          instance_type: action.instance_type,
          error_message: action.error_message,
          instance_id: action.instance_id,
          duration_ms: action.duration_ms,
          created_at: action.created_at,
          completed_at: action.completed_at,
          metadata: action.metadata,
          instance_status_before: action.instance_status_before,
          instance_status_after: action.instance_status_after,
        })),
        summary: {
          total_actions: allActions.length,
          exported_at: new Date().toISOString(),
        },
      };

      const jsonString = JSON.stringify(dataToCopy, null, 2);
      await navigator.clipboard.writeText(jsonString);
      
      setCopiedActions(true);
      setTimeout(() => {
        setCopiedActions(false);
      }, 2000);
    } catch (error) {
      console.error("Failed to copy actions to clipboard:", error);
    } finally {
      setCopyingActions(false);
    }
  }, [copyingActions, fetchAllActions, instanceId, instance]);

  return (
    <Dialog open={open} onOpenChange={onClose}>
      <DialogContent showCloseButton={false} className="w-[calc(100vw-2rem)] max-w-5xl sm:max-w-5xl p-0 overflow-hidden">
        <div className="flex flex-col max-h-[85vh]">
          <DialogHeader className="px-5 py-4 border-b">
            <div className="flex items-start justify-between gap-4">
              <div className="min-w-0">
                <div className="flex items-center gap-2">
                  <DialogTitle className="truncate">Actions de l&apos;instance</DialogTitle>
                  <Badge variant="outline" className="text-xs">{displayOrDash(instance?.status)}</Badge>
                </div>
                <div className="mt-1 flex items-center gap-2 text-xs text-muted-foreground font-mono break-all">
                  <span>{instanceId}</span>
                  <CopyButton text={instanceId} />
                </div>
              </div>
              <div className="flex items-center gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => setRefreshToken((v) => v + 1)}
                  title="Rafraîchir"
                >
                  <RefreshCcw className="h-4 w-4 mr-2" />
                  Rafraîchir
                </Button>
              </div>
            </div>

            <div className="mt-3 grid grid-cols-2 lg:grid-cols-4 gap-x-6 gap-y-2 text-xs">
              <div className="flex items-baseline gap-2 min-w-0 col-span-2 lg:col-span-2">
                <span className="text-muted-foreground">Model</span>
                <span className="text-muted-foreground">:</span>
                <span className="font-medium min-w-0 truncate">
                  {instance?.model_name && instance?.model_code
                    ? `${instance.model_name} (${instance.model_code})`
                    : instance?.model_code
                      ? instance.model_code
                      : "-"}
                </span>
              </div>
              <div className="flex items-baseline gap-2 min-w-0">
                <span className="text-muted-foreground">Mode</span>
                <span className="text-muted-foreground">:</span>
                <span className="font-medium">{vllmMode}</span>
              </div>
              <div className="flex items-baseline gap-2 min-w-0">
                <span className="text-muted-foreground">Créée</span>
                <span className="text-muted-foreground">:</span>
                <span className="font-medium min-w-0 truncate">{formatTimestamp(instance?.created_at)}</span>
              </div>
              <div className="flex items-baseline gap-2 min-w-0">
                <span className="text-muted-foreground">Dernier ping</span>
                <span className="text-muted-foreground">:</span>
                <span className="font-medium min-w-0 truncate">{formatTimestamp(lastPing)}</span>
              </div>
              <div className="flex items-baseline gap-2 min-w-0">
                <span className="text-muted-foreground">Région</span>
                <span className="text-muted-foreground">:</span>
                <span className="font-medium min-w-0 truncate">{displayOrDash(instance?.region)}</span>
              </div>
              <div className="flex items-baseline gap-2 min-w-0">
                <span className="text-muted-foreground">Zone</span>
                <span className="text-muted-foreground">:</span>
                <span className="font-medium min-w-0 truncate">{displayOrDash(instance?.zone)}</span>
              </div>
              <div className="flex items-baseline gap-2 min-w-0 col-span-2 lg:col-span-2">
                <span className="text-muted-foreground">Type</span>
                <span className="text-muted-foreground">:</span>
                <span className="font-medium min-w-0 truncate">{displayOrDash(instance?.instance_type)}</span>
              </div>
              <div className="flex items-baseline gap-2 min-w-0">
                <span className="text-muted-foreground">GPU</span>
                <span className="text-muted-foreground">:</span>
                <span className="font-medium">{instance?.gpu_count ?? "-"}</span>
              </div>
              <div className="flex items-baseline gap-2 min-w-0">
                <span className="text-muted-foreground">VRAM</span>
                <span className="text-muted-foreground">:</span>
                <span className="font-medium">{instance?.gpu_vram ? `${instance.gpu_vram} GB` : "-"}</span>
              </div>
              <div className="flex items-baseline gap-2 min-w-0 col-span-2 lg:col-span-2">
                <span className="text-muted-foreground">IP</span>
                <span className="text-muted-foreground">:</span>
                <span className="font-medium font-mono flex items-center gap-2 min-w-0 truncate">
                  {displayOrDash(instance?.ip_address)}
                  {instance?.ip_address ? <CopyButton text={instance.ip_address} /> : null}
                </span>
              </div>
              <div className="flex items-baseline gap-2 min-w-0 col-span-2 lg:col-span-2">
                <span className="text-muted-foreground">Provider</span>
                <span className="text-muted-foreground">:</span>
                <span className="font-medium min-w-0 truncate">{displayOrDash(instance?.provider_name)}</span>
              </div>
              <div className="flex items-baseline gap-2 min-w-0 col-span-2 lg:col-span-2">
                <span className="text-muted-foreground">Provider instance</span>
                <span className="text-muted-foreground">:</span>
                <span className="font-medium font-mono min-w-0 truncate">
                  {displayOrDash(instance?.provider_instance_id)}
                </span>
              </div>
              <div className="flex flex-col gap-1 min-w-0 col-span-2 lg:col-span-4">
                <div className="flex items-baseline gap-2 min-w-0">
                  <span className="text-muted-foreground">Storage</span>
                  <span className="text-muted-foreground">:</span>
                  <span className="font-medium min-w-0 truncate">
                    {(() => {
                      const count = instance?.storage_count ?? (instance?.storages?.length ?? 0);
                      // Prefer aggregated sizes from list/search payload; fallback to detailed storages (instance/:id).
                      const sizesFromSummary = (instance?.storage_sizes_gb ?? [])
                        .filter((n) => typeof n === "number" && n > 0)
                        .map((n) => `${n}GB`);
                      const sizesFromDetails = (instance?.storages ?? [])
                        .map((s) => s.size_gb)
                        .filter((n): n is number => typeof n === "number" && n > 0)
                        .map((n) => `${n}GB`);
                      const sizes = (sizesFromSummary.length ? sizesFromSummary : sizesFromDetails).join(", ");
                      return count > 0 ? `${count} storages${sizes ? ` (${sizes})` : ""}` : "-";
                    })()}
                  </span>
                </div>
                {instance?.storages?.length ? (
                  <div className="text-xs text-muted-foreground space-y-1">
                    {instance.storages.map((s, idx) => (
                      <div key={`${s.provider_volume_id}:${idx}`} className="truncate">
                        <span className="font-medium text-foreground/90">Storage {idx + 1}</span>
                        <span className="text-muted-foreground">:</span>{" "}
                        <span className="font-medium">{s.volume_type}</span>
                        {" - "}
                        <span className="font-medium">{s.size_gb ? `${s.size_gb}GB` : "-"}</span>
                        {" - "}
                        <span className="font-mono">{s.name ?? "-"}</span>
                        {" - "}
                        <span className="font-mono">{s.provider_volume_id}</span>
                        {s.is_boot ? <span className="text-muted-foreground"> (boot)</span> : null}
                      </div>
                    ))}
                  </div>
                ) : null}
              </div>
              <div className="flex items-center gap-2 min-w-0 col-span-2 lg:col-span-4">
                <span className="text-muted-foreground">Readiness</span>
                <span className="text-muted-foreground">:</span>
                <Badge variant="outline" className="text-[11px]">
                  vLLM HTTP {readiness.vllmHttp ?? "—"}
                </Badge>
                <Badge variant="outline" className="text-[11px]">
                  Model {readiness.modelLoaded ?? "—"}
                </Badge>
                <Badge variant="outline" className="text-[11px]">
                  Warmup {readiness.warmup ?? "—"}
                </Badge>
              </div>
            </div>
          </DialogHeader>

          <div className="grid grid-cols-1 lg:grid-cols-[1fr_380px] flex-1 min-h-0">
            <div className="min-w-0">
              <VirtualizedDataTable<ActionLog>
                // Use a stable listId so column prefs persist across instances (localStorage key is derived from listId).
                listId="monitoring:instance_actions"
                dataKey={queryKey}
                title="Actions"
                height={520}
                rowHeight={40}
                columns={columns}
                loadRange={loadRange}
                onCountsChange={handleCountsChange}
                rightHeader={
                  counts.filtered > 0 ? (
                    <Button
                      variant="outline"
                      size="sm"
                      className="h-8"
                      onClick={copyAllActionsToClipboard}
                      disabled={copyingActions}
                      title="Copier toutes les actions en JSON"
                    >
                      {copiedActions ? (
                        <>
                          <Check className="h-4 w-4 mr-2 text-green-600" />
                          Copié
                        </>
                      ) : (
                        <>
                          <Copy className="h-4 w-4 mr-2" />
                          {copyingActions ? "Copie..." : "Copier JSON"}
                        </>
                      )}
                    </Button>
                  ) : null
                }
                onRowClick={(row) => setSelectedLog(row)}
              />
            </div>

            <div className="hidden lg:block border-l bg-muted/10 min-w-0">
              <div className="p-4 h-full overflow-y-auto">
                {!selectedLog ? (
                  <div className="text-sm text-muted-foreground">
                    Clique une action pour voir le détail.
                  </div>
                ) : (
                  <div className="space-y-3">
                    <div className="flex items-center gap-2">
                      <div className="p-2 rounded-md bg-background border">
                        {(() => {
                          const Icon = getActionIcon(selectedLog.action_type);
                          return <Icon className="h-4 w-4" />;
                        })()}
                      </div>
                      <div className="min-w-0">
                        <div className="font-semibold truncate">{formatActionLabel(selectedLog.action_type)}</div>
                        <div className="text-xs text-muted-foreground font-mono">{selectedLog.id}</div>
                      </div>
                    </div>

                    <div className="grid grid-cols-2 gap-2 text-xs">
                      <div className="text-muted-foreground">Statut</div>
                      <div className="font-medium">{selectedLog.status}</div>
                      <div className="text-muted-foreground">Composant</div>
                      <div className="font-medium">{selectedLog.component}</div>
                      <div className="text-muted-foreground">Créé</div>
                      <div className="font-medium">{formatTimestamp(selectedLog.created_at)}</div>
                      <div className="text-muted-foreground">Durée</div>
                      <div className="font-medium font-mono">{formatDuration(selectedLog.duration_ms)}</div>
                      <div className="text-muted-foreground">Transition</div>
                      <div className="font-medium font-mono">
                        {selectedLog.instance_status_before ?? "-"} → {selectedLog.instance_status_after ?? "-"}
                      </div>
                    </div>

                    {selectedLog.error_message ? (
                      <div className="text-xs text-red-700 bg-red-50 border border-red-200 rounded-md p-2">
                        {selectedLog.error_message}
                      </div>
                    ) : null}

                    {selectedLog.metadata && Object.keys(selectedLog.metadata).length > 0 ? (
                      <div className="text-xs">
                        <div className="text-muted-foreground mb-1">Métadonnées</div>
                        <pre className="text-[11px] leading-snug bg-background border rounded-md p-2 overflow-x-auto">
{JSON.stringify(selectedLog.metadata, null, 2)}
                        </pre>
                      </div>
                    ) : (
                      <div className="text-xs text-muted-foreground">Aucune métadonnée.</div>
                    )}
                  </div>
                )}
              </div>
            </div>
          </div>

          <DialogFooter className="px-5 py-3 border-t sm:justify-between">
            <div className="text-xs text-muted-foreground">
              {counts.filtered > 0 ? `${counts.filtered} action(s) pour cette instance` : null}
            </div>
            <Button variant="outline" onClick={onClose}>
              Fermer
            </Button>
          </DialogFooter>
        </div>
      </DialogContent>
    </Dialog>
  );
}


