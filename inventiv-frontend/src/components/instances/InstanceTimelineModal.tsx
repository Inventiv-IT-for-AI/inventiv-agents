import type { LucideIcon } from "lucide-react";
import { Server, Zap, Cloud, Database, Archive, AlertTriangle, Clock, CheckCircle, RefreshCcw, Copy, Check, ChevronLeft, ChevronRight } from "lucide-react";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useCallback, useEffect, useMemo, useState, useRef } from "react";
import { apiUrl } from "@/lib/api";
import type { ActionLog, ActionType, Instance } from "@/lib/types";
import type { LoadRangeResult } from "ia-widgets";
import { IACopyButton, IADataTable, type IADataTableColumn } from "ia-widgets";
import { displayOrDash } from "@/lib/utils";
import { useRealtimeEvents } from "@/hooks/useRealtimeEvents";

interface InstanceTimelineModalProps {
  open: boolean;
  onClose: () => void;
  instanceId: string;
  instances?: Instance[];
  onInstanceChange?: (instanceId: string) => void;
}

export function InstanceTimelineModal({
  open,
  onClose,
  instanceId,
  instances = [],
  onInstanceChange,
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
  const tableContainerRef = useRef<HTMLDivElement>(null);
  const [tableHeight, setTableHeight] = useState(400);

  useEffect(() => {
    if (open && instanceId) {
      setSelectedLog(null);
      void fetchActionTypes();
      void fetchInstance();
      void fetchRecentLogs();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, instanceId, refreshToken]);

  // Calculer la hauteur disponible pour le tableau en fonction de l'espace dans le conteneur flex
  useEffect(() => {
    if (!open) return;

    const updateHeight = () => {
      const container = tableContainerRef.current;
      if (!container) {
        // Fallback si le conteneur n'est pas encore monté
        setTableHeight(400);
        return;
      }

      const rect = container.getBoundingClientRect();
      // Hauteur disponible = hauteur du conteneur
      // Le tableau IADataTable gère son propre header, donc on utilise toute la hauteur disponible
      // On soustrait juste un peu pour éviter les débordements (environ 60px pour le header du tableau)
      const availableHeight = Math.max(300, rect.height - 60);
      setTableHeight(availableHeight);
    };

    // Mettre à jour immédiatement et après un délai pour s'assurer que le layout est stabilisé
    const timeoutId1 = setTimeout(updateHeight, 0);
    const timeoutId2 = setTimeout(updateHeight, 100);
    const timeoutId3 = setTimeout(updateHeight, 300);

    // Observer les changements de taille du conteneur
    let resizeObserver: ResizeObserver | null = null;
    if (tableContainerRef.current && typeof ResizeObserver !== "undefined") {
      resizeObserver = new ResizeObserver(updateHeight);
      resizeObserver.observe(tableContainerRef.current);
    }

    return () => {
      clearTimeout(timeoutId1);
      clearTimeout(timeoutId2);
      clearTimeout(timeoutId3);
      resizeObserver?.disconnect();
    };
  }, [open, refreshToken]);

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
        const meta = (log.metadata ?? null) as Record<string, unknown> | null;
        const m = meta?.vllm_mode;
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

  const columns = useMemo<IADataTableColumn<ActionLog>[]>(() => {
    return [
      {
        id: "time",
        label: "Heure",
        width: 110,
        cell: ({ row }) => (
          <span className="font-mono text-[11px] text-muted-foreground whitespace-nowrap">
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
              <span className="truncate text-xs font-medium">{actionLabel}</span>
              {row.error_message ? <span className="ml-auto text-[10px] text-red-600">Erreur</span> : null}
            </div>
          );
        },
      },
      {
        id: "status",
        label: "Statut",
        width: 140,
        cell: ({ row }) => (
          <Badge variant="outline" className="text-[10px]">
            {row.status}
          </Badge>
        ),
      },
      {
        id: "duration",
        label: "Durée",
        width: 110,
        align: "right",
        cell: ({ row }) => <span className="font-mono text-[11px] text-muted-foreground">{formatDuration(row.duration_ms)}</span>,
      },
      {
        id: "transition",
        label: "Transition",
        width: 220,
        cellClassName: "truncate",
        cell: ({ row }) => (
          <span className="font-mono text-[10px] text-muted-foreground truncate">
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
          // Include all known instance fields (including new ones like storages).
          ...(instance ?? { id: instanceId }),
          // Explicitly include compute fields from instance types (for schema stability in exports).
          cpu_count: instance?.cpu_count ?? null,
          ram_gb: instance?.ram_gb ?? null,
          // Derived / computed fields shown in header.
          vllm_mode: vllmMode,
          readiness,
          last_ping: lastPing,
        },
        // Keep full action objects so any newly added fields are automatically included.
        actions: allActions,
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
  }, [copyingActions, fetchAllActions, instanceId, instance, lastPing, readiness, vllmMode]);

  // Navigation entre instances
  const currentInstanceIndex = useMemo(() => {
    if (!instances.length) return -1;
    return instances.findIndex((inst) => inst.id === instanceId);
  }, [instances, instanceId]);

  const hasPreviousInstance = currentInstanceIndex > 0;
  const hasNextInstance = currentInstanceIndex >= 0 && currentInstanceIndex < instances.length - 1;

  const navigateToPreviousInstance = useCallback(() => {
    if (!hasPreviousInstance || !onInstanceChange) return;
    const prevInstance = instances[currentInstanceIndex - 1];
    if (prevInstance) {
      onInstanceChange(prevInstance.id);
    }
  }, [hasPreviousInstance, instances, currentInstanceIndex, onInstanceChange]);

  const navigateToNextInstance = useCallback(() => {
    if (!hasNextInstance || !onInstanceChange) return;
    const nextInstance = instances[currentInstanceIndex + 1];
    if (nextInstance) {
      onInstanceChange(nextInstance.id);
    }
  }, [hasNextInstance, instances, currentInstanceIndex, onInstanceChange]);

  return (
    <Dialog open={open} onOpenChange={onClose}>
      <DialogContent showCloseButton={false} className="w-[calc(100vw-2rem)] max-w-5xl sm:max-w-5xl p-0 overflow-hidden">
        <div className="flex flex-col h-[85vh] overflow-hidden">
          <DialogHeader className="px-5 py-4 border-b flex-shrink-0">
            <div className="flex items-start justify-between gap-4">
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <DialogTitle className="truncate">Actions de l&apos;instance</DialogTitle>
                  <Badge variant="outline" className="text-xs">{displayOrDash(instance?.status)}</Badge>
                </div>
                <div className="mt-1 flex items-center gap-2 text-xs text-muted-foreground font-mono break-all">
                  <span>{instanceId}</span>
                  <IACopyButton text={instanceId} />
                </div>
              </div>
              <div className="flex items-center gap-2">
                {instances.length > 1 && (
                  <>
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={navigateToPreviousInstance}
                      disabled={!hasPreviousInstance}
                      title="Instance précédente"
                    >
                      <ChevronLeft className="h-4 w-4 mr-1" />
                      Précédente
                    </Button>
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={navigateToNextInstance}
                      disabled={!hasNextInstance}
                      title="Instance suivante"
                    >
                      Suivante
                      <ChevronRight className="h-4 w-4 ml-1" />
                    </Button>
                  </>
                )}
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
                <span className="text-muted-foreground">CPU</span>
                <span className="text-muted-foreground">:</span>
                <span className="font-medium">{typeof instance?.cpu_count === "number" && instance.cpu_count > 0 ? instance.cpu_count : "-"}</span>
              </div>
              <div className="flex items-baseline gap-2 min-w-0">
                <span className="text-muted-foreground">RAM</span>
                <span className="text-muted-foreground">:</span>
                <span className="font-medium">{typeof instance?.ram_gb === "number" && instance.ram_gb > 0 ? `${instance.ram_gb} GB` : "-"}</span>
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
                  {instance?.ip_address ? <IACopyButton text={instance.ip_address} /> : null}
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

          <div className="grid grid-cols-1 lg:grid-cols-[1fr_380px] flex-1 min-h-0 overflow-hidden">
                <div ref={tableContainerRef} className="min-w-0 flex flex-col overflow-hidden px-3 pt-2">
                  <IADataTable<ActionLog>
                    // Use a stable listId so column prefs persist across instances (localStorage key is derived from listId).
                    listId="monitoring:instance_actions"
                    dataKey={queryKey}
                    title={<span className="text-sm font-medium pl-1">Actions</span>}
                    height={tableHeight}
                    rowHeight={40}
                    columns={columns}
                    loadRange={loadRange}
                    onCountsChange={handleCountsChange}
                    leftMeta={
                      <span className="text-xs text-muted-foreground font-mono">
                        {counts.filtered !== counts.total
                          ? `Filtré ${counts.filtered} - Total ${counts.total}`
                          : `Total ${counts.total}`}
                      </span>
                    }
                    rightHeader={
                      <div className="flex items-center gap-2">
                        {counts.filtered > 0 ? (
                          <Button
                            variant="outline"
                            size="sm"
                            className="h-7 text-xs"
                            onClick={copyAllActionsToClipboard}
                            disabled={copyingActions}
                            title="Copier toutes les actions en JSON"
                          >
                            {copiedActions ? (
                              <>
                                <Check className="h-3 w-3 mr-1 text-green-600" />
                                Copié
                              </>
                            ) : (
                              <>
                                <Copy className="h-3 w-3 mr-1" />
                                {copyingActions ? "Copie..." : "Copier JSON"}
                              </>
                            )}
                          </Button>
                        ) : null}
                      </div>
                    }
                    onRowClick={(row) => setSelectedLog(row)}
                  />
                </div>

                <div className="hidden lg:block border-l bg-muted/10 min-w-0 flex flex-col overflow-hidden">
                  <div className="p-4 flex-1 overflow-y-auto">
                    {!selectedLog ? (
                      <div className="text-sm text-muted-foreground">Clique une action pour voir le détail.</div>
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

          <DialogFooter className="px-5 py-3 border-t sm:justify-between flex-shrink-0">
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


