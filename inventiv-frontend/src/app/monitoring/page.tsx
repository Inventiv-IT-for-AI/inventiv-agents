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
import { VirtualizedRemoteList, type LoadRangeResult } from "@/components/shared/VirtualizedRemoteList";

export default function MonitoringPage() {
    const [actionTypes, setActionTypes] = useState<Record<string, ActionType>>({});
    const [filterComponent, setFilterComponent] = useState<string>("all");
    const [filterStatus, setFilterStatus] = useState<string>("all");
    const [filterActionType, setFilterActionType] = useState<string>("all");
    const [counts, setCounts] = useState({ total: 0, filtered: 0 });
    const [statusCounts, setStatusCounts] = useState({ success: 0, failed: 0, inProgress: 0 });
    const [selectedInstanceId, setSelectedInstanceId] = useState<string | null>(null);
    const [timelineModalOpen, setTimelineModalOpen] = useState(false);
    const [copiedLogId, setCopiedLogId] = useState<string | null>(null);

    const filtersActive = useMemo(
        () => filterComponent !== "all" || filterStatus !== "all" || filterActionType !== "all",
        [filterActionType, filterComponent, filterStatus]
    );

    const queryKey = useMemo(
        () => JSON.stringify({ filterComponent, filterStatus, filterActionType }),
        [filterComponent, filterStatus, filterActionType]
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

    const handleCountsChange = useCallback(
        ({ total, filtered }: { total: number; filtered: number }) => {
            setCounts({ total, filtered });
        },
        []
    );

    useEffect(() => {
        // eslint-disable-next-line react-hooks/set-state-in-effect
        fetchActionTypes();
    }, [fetchActionTypes]);

    const copyLogToClipboard = async (log: ActionLog, e: React.MouseEvent) => {
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
    };

    const getStatusBadge = (status: string) => {
        const config = {
            success: { variant: "default" as const, className: "bg-green-500 hover:bg-green-600" },
            failed: { variant: "destructive" as const, className: "" },
            in_progress: { variant: "secondary" as const, className: "bg-yellow-500 hover:bg-yellow-600 text-white" },
        };
        const { variant, className } = config[status as keyof typeof config] || { variant: "outline" as const, className: "" };
        return <Badge variant={variant} className={className}>{status}</Badge>;
    };

    const getActionTypeBadge = (actionType: string) => {
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
    };

    const getComponentColor = (component: string) => {
        return component === "api" ? "text-blue-600" : "text-indigo-600";
    };

    const formatDuration = (ms: number | null) => {
        if (!ms) return "-";
        if (ms < 1000) return `${ms}ms`;
        if (ms < 60000) return `${(ms / 1000).toFixed(2)}s`;
        return `${(ms / 60000).toFixed(2)}min`;
    };

    const formatMetadata = (metadata: Record<string, unknown> | null) => {
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
    };

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
        [filterActionType, filterComponent, filterStatus]
    );

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
                <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                    <CardTitle>Action Logs</CardTitle>
                    <div className="text-sm text-muted-foreground font-mono">
                        {filtersActive
                            ? `Filtrés ${counts.filtered} / Total ${counts.total} lignes`
                            : `Total ${counts.total} lignes`}
                    </div>
                </CardHeader>
                <CardContent>
                    {(() => {
                        const cols =
                            "grid-cols-[72px_140px_200px_130px_120px_160px_110px_160px_320px_280px_120px]";
                        const header = (
                            <div
                                className={`grid ${cols} gap-2 px-3 py-2 text-xs font-semibold text-muted-foreground border-b bg-background`}
                            >
                                <div>#</div>
                                <div>Temps</div>
                                <div>Action</div>
                                <div>Composant</div>
                                <div>Statut</div>
                                <div>Transition</div>
                                <div>Durée</div>
                                <div>Instance</div>
                                <div>Métadonnées</div>
                                <div>Erreur</div>
                                <div className="text-right">Actions</div>
                            </div>
                        );

                        return (
                            <div className="border rounded-md overflow-hidden bg-background">
                                <VirtualizedRemoteList<ActionLog>
                                    queryKey={queryKey}
                                    height={560}
                                    header={header}
                                    headerHeight={40}
                                    contentWidth={1920}
                                    rowHeight={56}
                                    className="w-full"
                                    loadRange={loadRange}
                                    onCountsChange={handleCountsChange}
                                    renderRow={({ index, item, style, isLoaded }) => {
                                        const rowNumber = index + 1;
                                        const onRowClick = () => {
                                            if (item?.instance_id) {
                                                setSelectedInstanceId(item.instance_id);
                                                setTimelineModalOpen(true);
                                            }
                                        };

                                        return (
                                            <div
                                                style={style}
                                                className={`grid ${cols} gap-2 px-3 items-center border-b ${
                                                    index % 2 === 0 ? "bg-background" : "bg-muted/10"
                                                } ${item?.instance_id ? "cursor-pointer hover:bg-muted/30" : ""}`}
                                                onClick={onRowClick}
                                            >
                                                <div className="font-mono text-xs text-muted-foreground">{rowNumber}</div>
                                                <div className="whitespace-nowrap text-sm">
                                                    {isLoaded && item
                                                        ? formatDistanceToNow(parseISO(item.created_at), { addSuffix: true })
                                                        : "…"}
                                                </div>
                                                <div>{isLoaded && item ? getActionTypeBadge(item.action_type) : <Badge variant="outline">…</Badge>}</div>
                                                <div className={`font-semibold ${isLoaded && item ? getComponentColor(item.component) : ""}`}>
                                                    {isLoaded && item ? item.component : "…"}
                                                </div>
                                                <div>{isLoaded && item ? getStatusBadge(item.status) : <Badge variant="outline">…</Badge>}</div>
                                                <div className="font-mono text-xs text-muted-foreground whitespace-nowrap">
                                                    {isLoaded && item ? formatTransition(item.instance_status_before, item.instance_status_after) : "…"}
                                                </div>
                                                <div className="font-mono text-sm">{isLoaded && item ? formatDuration(item.duration_ms) : "…"}</div>
                                                <div className="font-mono text-xs">
                                                    {isLoaded && item && item.instance_id ? (
                                                        <span className="bg-muted px-2 py-1 rounded">
                                                            {item.instance_id.substring(0, 8)}...
                                                        </span>
                                                    ) : (
                                                        "-"
                                                    )}
                                                </div>
                                                <div className="truncate">{isLoaded && item ? formatMetadata(item.metadata) : null}</div>
                                                <div className="truncate text-red-600 text-sm">{isLoaded && item ? item.error_message || "-" : "…"}</div>
                                                <div className="text-right">
                                                    {isLoaded && item ? (
                                                        <button
                                                            onClick={(e) => copyLogToClipboard(item, e)}
                                                            className="inline-flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-md transition-all hover:bg-muted focus:outline-none focus:ring-2 focus:ring-primary focus:ring-offset-1"
                                                            title="Copier les détails"
                                                        >
                                                            {copiedLogId === item.id ? (
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
                                                    ) : null}
                                                </div>
                                            </div>
                                        );
                                    }}
                                />
                            </div>
                        );
                    })()}
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
