"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import { formatDistanceToNow, parseISO } from "date-fns";
import { Badge } from "@/components/ui/badge";
import { apiUrl } from "@/lib/api";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import type { LucideIcon } from "lucide-react";
import { Activity, CheckCircle, XCircle, Clock, Server, Zap, Cloud, Database, Archive, AlertTriangle, Copy, Check } from 'lucide-react';
import { InstanceTimelineModal } from "@/components/instances/InstanceTimelineModal";
import type { ActionLog, ActionType } from "@/lib/types";
import { IADataTable, type IADataTableColumn, type DataTableSortState, type LoadRangeResult } from "ia-widgets";
import { useRealtimeEvents } from "@/hooks/useRealtimeEvents";
export default function MonitoringPage() {
    useRealtimeEvents();
    const [actionTypes, setActionTypes] = useState<Record<string, ActionType>>({});
    const [filterComponent, setFilterComponent] = useState<string>("all");
    const [filterStatus, setFilterStatus] = useState<string>("all");
    const [filterActionType, setFilterActionType] = useState<string>("all");
    const [counts, setCounts] = useState({ total: 0, filtered: 0 });
    const [statusCounts, setStatusCounts] = useState({ success: 0, failed: 0, inProgress: 0 });
    const [selectedInstanceId, setSelectedInstanceId] = useState<string | null>(null);
    const [timelineModalOpen, setTimelineModalOpen] = useState(false);
    const [copiedLogId, setCopiedLogId] = useState<string | null>(null);
    const [sort, setSort] = useState<DataTableSortState>(null);

    const [refreshSeq, setRefreshSeq] = useState(0);
    useEffect(() => {
        const handler = () => setRefreshSeq((v) => v + 1);
        window.addEventListener("refresh-action-logs", handler);
        return () => window.removeEventListener("refresh-action-logs", handler);
    }, []);

    const queryKey = useMemo(
        () => JSON.stringify({ filterComponent, filterStatus, filterActionType, refreshSeq, sort }),
        [filterComponent, filterStatus, filterActionType, refreshSeq, sort]
    );

    const fetchActionTypes = useCallback(async () => {
        try {
            const response = await fetch(apiUrl("action_types"));
            const data: ActionType[] = await response.json();
            const map: Record<string, ActionType> = {};
            for (const at of data) map[at.code] = at;
            setActionTypes(map);
        } catch (error) {
            console.error("Failed to fetch action types:", error);
        }
    }, []);

    useEffect(() => {
        // eslint-disable-next-line react-hooks/set-state-in-effect
        fetchActionTypes();
    }, [fetchActionTypes]);

    const copyLogToClipboard = useCallback(async (log: ActionLog, e: React.MouseEvent) => {
        e.stopPropagation(); // Prevent row click event

        const textFormat = `
Action Log
----------
ID: ${log.id}
Time: ${new Date(log.created_at).toLocaleString()}
Action: ${log.action_type}
Component: ${log.component}
Status: ${log.status}
Duration: ${log.duration_ms ? `${log.duration_ms}ms` : '-'}
Instance ID: ${log.instance_id || '-'}
Error: ${log.error_message || '-'}
Metadata: ${log.metadata ? JSON.stringify(log.metadata, null, 2) : '-'}
`.trim();

        try {
            await navigator.clipboard.writeText(textFormat);
            setCopiedLogId(log.id);
            setTimeout(() => setCopiedLogId(null), 2000);
        } catch (err) {
            console.error('Failed to copy:', err);
        }
    }, []);

    const getStatusBadge = useCallback((status: string) => {
        const config = {
            success: { variant: "default" as const, className: "bg-green-500 hover:bg-green-600" },
            failed: { variant: "destructive" as const, className: "" },
            in_progress: { variant: "secondary" as const, className: "bg-yellow-500 hover:bg-yellow-600 text-white" },
        };
        const { variant, className } = config[status as keyof typeof config] || { variant: "outline" as const, className: "" };
        return <Badge variant={variant} className={className}>{status}</Badge>;
    }, []);

    const getActionTypeBadge = useCallback((actionType: string) => {
        const iconMap: Record<string, LucideIcon> = {
            Activity,
            AlertTriangle,
            Archive,
            CheckCircle,
            Clock,
            Cloud,
            Database,
            Server,
            Zap,
            Copy,
            Check,
            XCircle,
        };

        const def = actionTypes[actionType];
        const label =
            def?.label ||
            actionType
                .toLowerCase()
                .replace(/_/g, " ")
                .replace(/\b\w/g, (l) => l.toUpperCase());
        const color = def?.color_class || "bg-gray-500 hover:bg-gray-600 text-white";
        const Icon = iconMap[def?.icon || "Activity"] || (Activity as LucideIcon);

        return (
            <Badge className={`${color} flex items-center gap-1 px-2 py-1`}>
                <Icon className="h-3 w-3" />
                <span className="text-xs">{label}</span>
            </Badge>
        );
    }, [actionTypes]);

    const getComponentColor = useCallback((component: string) => {
        return component === "api" ? "text-blue-600" : "text-indigo-600";
    }, []);

    const formatDuration = useCallback((ms: number | null) => {
        if (!ms) return "-";
        if (ms < 1000) return `${ms}ms`;
        if (ms < 60000) return `${(ms / 1000).toFixed(2)}s`;
        return `${(ms / 60000).toFixed(2)}min`;
    }, []);

    const formatMetadata = useCallback((metadata: Record<string, unknown> | null) => {
        if (!metadata) return null;
        const keys = Object.keys(metadata);
        if (keys.length === 0) return null;

        // Show first 2 keys as summary
        const summary = keys
            .slice(0, 2)
            .map((key) => `${key}: ${String(metadata[key])}`)
            .join(", ");
        return (
            <span className="text-xs text-muted-foreground font-mono">
                {summary}
                {keys.length > 2 && `... (+${keys.length - 2})`}
            </span>
        );
    }, []);

    const formatTransition = (before?: string | null, after?: string | null) => {
        if (!before && !after) return "-";
        if (before && after && before !== after) return `${before} → ${after}`;
        return before || after || "-";
    };

    type ActionLogsSearchResponse = {
        offset: number;
        limit: number;
        total_count: number;
        filtered_count: number;
        status_counts?: { success: number; failed: number; in_progress: number } | null;
        rows: ActionLog[];
    };

    const loadRange = useCallback(
        async (offset: number, limit: number): Promise<LoadRangeResult<ActionLog>> => {
            const params = new URLSearchParams();
            params.set("offset", String(offset));
            params.set("limit", String(limit));
            if (filterComponent !== "all") params.set("component", filterComponent);
            if (filterStatus !== "all") params.set("status", filterStatus);
            if (filterActionType !== "all") params.set("action_type", filterActionType);
            if (offset === 0) params.set("include_stats", "true");
            if (sort) {
                const colToSortBy: Record<string, string> = {
                    time: "created_at",
                    action: "action_type",
                    provider: "provider_name",
                    type: "instance_type",
                    component: "component",
                    status: "status",
                    duration: "duration_ms",
                    instance: "instance_id",
                };
                const sortBy = colToSortBy[sort.columnId];
                if (sortBy) {
                    params.set("sort_by", sortBy);
                    params.set("sort_dir", sort.direction);
                }
            }

            const res = await fetch(apiUrl(`action_logs/search?${params.toString()}`));
            const data: ActionLogsSearchResponse = await res.json();

            if (data.status_counts) {
                setStatusCounts({
                    success: data.status_counts.success ?? 0,
                    failed: data.status_counts.failed ?? 0,
                    inProgress: data.status_counts.in_progress ?? 0,
                });
            }

            return {
                offset: data.offset,
                items: data.rows,
                totalCount: data.total_count,
                filteredCount: data.filtered_count,
            };
        },
        [filterActionType, filterComponent, filterStatus, sort]
    );

    const columns = useMemo<IADataTableColumn<ActionLog>[]>(() => {
        return [
            {
                id: "time",
                label: "Temps",
                width: 140,
                sortable: true,
                cell: ({ row }) => (
                    <span className="whitespace-nowrap">
                        {formatDistanceToNow(parseISO(row.created_at), { addSuffix: true })}
                    </span>
                ),
            },
            {
                id: "action",
                label: "Action",
                width: 200,
                sortable: true,
                cell: ({ row }) => getActionTypeBadge(row.action_type),
            },
            {
                id: "provider",
                label: "Provider",
                width: 140,
                sortable: true,
                cell: ({ row }) => <span className="truncate">{row.provider_name ?? "-"}</span>,
            },
            {
                id: "type",
                label: "Type",
                width: 180,
                sortable: true,
                cell: ({ row }) => <span className="truncate">{row.instance_type ?? "-"}</span>,
            },
            {
                id: "component",
                label: "Composant",
                width: 140,
                sortable: true,
                cell: ({ row }) => (
                    <span className={`font-semibold ${getComponentColor(row.component)}`}>{row.component}</span>
                ),
            },
            {
                id: "status",
                label: "Statut",
                width: 120,
                sortable: true,
                cell: ({ row }) => getStatusBadge(row.status),
            },
            {
                id: "transition",
                label: "Transition",
                width: 190,
                cell: ({ row }) => (
                    <span className="font-mono text-xs text-muted-foreground whitespace-nowrap">
                        {formatTransition(row.instance_status_before, row.instance_status_after)}
                    </span>
                ),
            },
            {
                id: "duration",
                label: "Durée",
                width: 110,
                sortable: true,
                cell: ({ row }) => <span className="font-mono text-sm">{formatDuration(row.duration_ms)}</span>,
            },
            {
                id: "instance",
                label: "Instance",
                width: 150,
                sortable: true,
                cell: ({ row }) =>
                    row.instance_id ? (
                        <span className="font-mono text-xs bg-muted px-2 py-1 rounded">
                            {row.instance_id.substring(0, 8)}...
                        </span>
                    ) : (
                        "-"
                    ),
            },
            {
                id: "metadata",
                label: "Métadonnées",
                width: 320,
                cellClassName: "truncate",
                cell: ({ row }) => formatMetadata(row.metadata),
            },
            {
                id: "error",
                label: "Erreur",
                width: 320,
                cellClassName: "truncate text-red-600 text-sm",
                cell: ({ row }) => row.error_message || "-",
            },
            {
                id: "actions",
                label: "Actions",
                width: 140,
                align: "right",
                sortable: false,
                cell: ({ row }) => (
                    <button
                        onClick={(e) => copyLogToClipboard(row, e)}
                        className="inline-flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-md transition-all hover:bg-muted focus:outline-none focus:ring-2 focus:ring-primary focus:ring-offset-1"
                        title="Copier les détails"
                    >
                        {copiedLogId === row.id ? (
                            <>
                                <Check className="h-3.5 w-3.5 text-green-600" />
                                <span className="text-green-600">Copié</span>
                            </>
                        ) : (
                            <>
                                <Copy className="h-3.5 w-3.5" />
                                <span>Copier</span>
                            </>
                        )}
                    </button>
                ),
            },
        ];
        // depends on helpers + state
    }, [copiedLogId, copyLogToClipboard, formatDuration, formatMetadata, getActionTypeBadge, getComponentColor, getStatusBadge]);

    return (
        <div className="p-8 space-y-6">
            <div className="flex justify-between items-center">
                <h1 className="text-3xl font-bold">Monitoring & Action Logs</h1>
                <div className="flex items-center gap-2">
                    <Clock className="h-4 w-4 text-muted-foreground" />
                    <div className="text-sm text-muted-foreground">Défilement virtuel</div>
                </div>
            </div>

            {/* Stats Cards */}
            <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
                <Card>
                    <CardContent className="pt-6">
                        <div className="flex items-center space-x-4">
                            <Activity className="h-8 w-8 text-gray-500" />
                            <div>
                                <p className="text-sm text-muted-foreground">Total Actions</p>
                                <p className="text-2xl font-bold">{counts.total}</p>
                            </div>
                        </div>
                    </CardContent>
                </Card>

                <Card>
                    <CardContent className="pt-6">
                        <div className="flex items-center space-x-4">
                            <CheckCircle className="h-8 w-8 text-green-500" />
                            <div>
                                <p className="text-sm text-muted-foreground">Success</p>
                                <p className="text-2xl font-bold">{statusCounts.success}</p>
                            </div>
                        </div>
                    </CardContent>
                </Card>

                <Card>
                    <CardContent className="pt-6">
                        <div className="flex items-center space-x-4">
                            <XCircle className="h-8 w-8 text-red-500" />
                            <div>
                                <p className="text-sm text-muted-foreground">Failed</p>
                                <p className="text-2xl font-bold">{statusCounts.failed}</p>
                            </div>
                        </div>
                    </CardContent>
                </Card>

                <Card>
                    <CardContent className="pt-6">
                        <div className="flex items-center space-x-4">
                            <Clock className="h-8 w-8 text-blue-500" />
                            <div>
                                <p className="text-sm text-muted-foreground">In Progress</p>
                                <p className="text-2xl font-bold">{statusCounts.inProgress}</p>
                            </div>
                        </div>
                    </CardContent>
                </Card>
            </div>

            {/* Filters */}
            <Card>
                <CardHeader>
                    <CardTitle>Filters</CardTitle>
                </CardHeader>
                <CardContent>
                    <div className="flex space-x-4">
                        <div className="flex-1">
                            <label className="text-sm font-medium">Component</label>
                            <Select value={filterComponent} onValueChange={setFilterComponent}>
                                <SelectTrigger>
                                    <SelectValue />
                                </SelectTrigger>
                                <SelectContent>
                                    <SelectItem value="all">All</SelectItem>
                                    <SelectItem value="api">API</SelectItem>
                                    <SelectItem value="orchestrator">Orchestrator</SelectItem>
                                </SelectContent>
                            </Select>
                        </div>

                        <div className="flex-1">
                            <label className="text-sm font-medium">Status</label>
                            <Select value={filterStatus} onValueChange={setFilterStatus}>
                                <SelectTrigger>
                                    <SelectValue />
                                </SelectTrigger>
                                <SelectContent>
                                    <SelectItem value="all">All</SelectItem>
                                    <SelectItem value="success">Success</SelectItem>
                                    <SelectItem value="failed">Failed</SelectItem>
                                    <SelectItem value="in_progress">In Progress</SelectItem>
                                </SelectContent>
                            </Select>
                        </div>

                        <div className="flex-1">
                            <label className="text-sm font-medium">Action Type</label>
                            <Select value={filterActionType} onValueChange={setFilterActionType}>
                                <SelectTrigger>
                                    <SelectValue />
                                </SelectTrigger>
                                <SelectContent>
                                    <SelectItem value="all">All</SelectItem>
                                    {/* Creation workflow */}
                                    <SelectItem value="REQUEST_CREATE">Request Create</SelectItem>
                                    <SelectItem value="EXECUTE_CREATE">Execute Create</SelectItem>
                                    <SelectItem value="PROVIDER_CREATE">Provider Create</SelectItem>
                                    <SelectItem value="INSTANCE_CREATED">Instance Created</SelectItem>
                                    {/* Termination workflow */}
                                    <SelectItem value="REQUEST_TERMINATE">Request Terminate</SelectItem>
                                    <SelectItem value="EXECUTE_TERMINATE">Execute Terminate</SelectItem>
                                    <SelectItem value="PROVIDER_TERMINATE">Provider Terminate</SelectItem>
                                    <SelectItem value="INSTANCE_TERMINATED">Instance Terminated</SelectItem>
                                    {/* Other actions */}
                                    <SelectItem value="ARCHIVE_INSTANCE">Archive Instance</SelectItem>
                                    <SelectItem value="PROVIDER_DELETED_DETECTED">Provider Deleted</SelectItem>
                                </SelectContent>
                            </Select>
                        </div>
                    </div>
                </CardContent>
            </Card>

            {/* Action Logs Table */}
            <Card>
                <CardContent>
                    <IADataTable<ActionLog>
                        listId="monitoring:action_logs"
                        dataKey={queryKey}
                        title="Action Logs"
                        autoHeight={true}
                        height={300}
                        rowHeight={56}
                        columns={columns}
                        loadRange={loadRange}
                        onCountsChange={setCounts}
                        sortState={sort}
                        onSortChange={setSort}
                        sortingMode="server"
                        onRowClick={(row) => {
                            if (row.instance_id) {
                                setSelectedInstanceId(row.instance_id);
                                setTimelineModalOpen(true);
                            }
                        }}
                    />
                </CardContent>
            </Card>

            {/* Instance Timeline Modal */}
            {selectedInstanceId && (
                <InstanceTimelineModal
                    open={timelineModalOpen}
                    onClose={() => setTimelineModalOpen(false)}
                    instanceId={selectedInstanceId}
                />
            )}
        </div>
    );
}


