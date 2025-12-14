"use client";

import { useCallback, useEffect, useState } from "react";
import { formatDistanceToNow, parseISO } from "date-fns";
import { Badge } from "@/components/ui/badge";
import { apiUrl } from "@/lib/api";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Table, TableHeader, TableRow, TableHead, TableBody, TableCell } from "@/components/ui/table";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import type { LucideIcon } from "lucide-react";
import { Activity, CheckCircle, XCircle, Clock, Server, Zap, Cloud, Database, Archive, AlertTriangle, Copy, Check } from 'lucide-react';
import { InstanceTimelineModal } from "@/components/instances/InstanceTimelineModal";
import type { ActionLog } from "@/lib/types";

export default function MonitoringPage() {
    const [logs, setLogs] = useState<ActionLog[]>([]);
    const [filterComponent, setFilterComponent] = useState<string>("all");
    const [filterStatus, setFilterStatus] = useState<string>("all");
    const [filterActionType, setFilterActionType] = useState<string>("all");
    const [stats, setStats] = useState({ total: 0, success: 0, failed: 0, inProgress: 0 });
    const [selectedInstanceId, setSelectedInstanceId] = useState<string | null>(null);
    const [timelineModalOpen, setTimelineModalOpen] = useState(false);
    const [copiedLogId, setCopiedLogId] = useState<string | null>(null);

    const fetchLogs = useCallback(async () => {
        try {
            const params = new URLSearchParams();
            if (filterComponent !== "all") params.append("component", filterComponent);
            if (filterStatus !== "all") params.append("status", filterStatus);
            if (filterActionType !== "all") params.append("action_type", filterActionType);
            params.append("limit", "100");

            // Use the Next.js proxy path like in Dashboard
            const response = await fetch(apiUrl(`action_logs?${params.toString()}`));
            const data = await response.json();
            setLogs(data);

            // Calculate stats
            const total = data.length;
            const success = data.filter((l: ActionLog) => l.status === "success").length;
            const failed = data.filter((l: ActionLog) => l.status === "failed").length;
            const inProgress = data.filter((l: ActionLog) => l.status === "in_progress").length;
            setStats({ total, success, failed, inProgress });
        } catch (error) {
            console.error("Failed to fetch logs:", error);
        }
    }, [filterActionType, filterComponent, filterStatus]);

    useEffect(() => {
        // eslint-disable-next-line react-hooks/set-state-in-effect
        fetchLogs();
        const interval = setInterval(fetchLogs, 10000); // Auto-refresh every 10s
        return () => clearInterval(interval);
    }, [fetchLogs]);

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
        const config: Record<string, { color: string; icon: LucideIcon; label: string }> = {
            // Creation workflow
            REQUEST_CREATE: { color: "bg-blue-500 hover:bg-blue-600 text-white", icon: Zap, label: "Request Create" },
            EXECUTE_CREATE: { color: "bg-purple-500 hover:bg-purple-600 text-white", icon: Server, label: "Execute Create" },
            PROVIDER_CREATE: { color: "bg-orange-500 hover:bg-orange-600 text-white", icon: Cloud, label: "Provider Create" },
            PROVIDER_START: { color: "bg-orange-500 hover:bg-orange-600 text-white", icon: Cloud, label: "Provider Start" },
            PROVIDER_GET_IP: { color: "bg-orange-500 hover:bg-orange-600 text-white", icon: Cloud, label: "Provider Get IP" },
            INSTANCE_CREATED: { color: "bg-green-500 hover:bg-green-600 text-white", icon: Database, label: "Instance Created" },

            // Termination workflow
            REQUEST_TERMINATE: { color: "bg-blue-600 hover:bg-blue-700 text-white", icon: Zap, label: "Request Terminate" },
            EXECUTE_TERMINATE: { color: "bg-purple-600 hover:bg-purple-700 text-white", icon: Server, label: "Execute Terminate" },
            PROVIDER_TERMINATE: { color: "bg-orange-600 hover:bg-orange-700 text-white", icon: Cloud, label: "Provider Terminate" },
            TERMINATION_PENDING: { color: "bg-yellow-500 hover:bg-yellow-600 text-white", icon: Clock, label: "Termination Pending" },
            TERMINATOR_RETRY: { color: "bg-orange-600 hover:bg-orange-700 text-white", icon: Cloud, label: "Terminator Retry" },
            TERMINATION_CONFIRMED: { color: "bg-red-500 hover:bg-red-600 text-white", icon: Database, label: "Termination Confirmed" },
            INSTANCE_TERMINATED: { color: "bg-red-500 hover:bg-red-600 text-white", icon: Database, label: "Instance Terminated" },

            // Archive workflow
            ARCHIVE_INSTANCE: { color: "bg-gray-600 hover:bg-gray-700 text-white", icon: Archive, label: "Archive Instance" },

            // Reconciliation & monitoring
            PROVIDER_DELETED_DETECTED: { color: "bg-yellow-600 hover:bg-yellow-700 text-white", icon: AlertTriangle, label: "Provider Deleted" },

            // Legacy (to be removed)
            TERMINATE_INSTANCE: { color: "bg-purple-600 hover:bg-purple-700 text-white", icon: Server, label: "Terminate Instance" },
            SCALEWAY_CREATE: { color: "bg-orange-500 hover:bg-orange-600 text-white", icon: Cloud, label: "Provider Create" },
            SCALEWAY_DELETE: { color: "bg-orange-600 hover:bg-orange-700 text-white", icon: Cloud, label: "Provider Delete" },
        };

        const { color, icon: Icon, label } = config[actionType] || {
            color: "bg-gray-500 hover:bg-gray-600 text-white",
            icon: Activity as LucideIcon,
            label: actionType.replace(/_/g, ' ').replace(/\b\w/g, l => l.toUpperCase())
        };

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

    return (
        <div className="p-8 space-y-6">
            <div className="flex justify-between items-center">
                <h1 className="text-3xl font-bold">Monitoring & Action Logs</h1>
                <div className="flex items-center gap-2">
                    <Clock className="h-4 w-4 text-muted-foreground" />
                    <div className="text-sm text-muted-foreground">Auto-refresh: 10s</div>
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
                                <p className="text-2xl font-bold">{stats.total}</p>
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
                                <p className="text-2xl font-bold">{stats.success}</p>
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
                                <p className="text-2xl font-bold">{stats.failed}</p>
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
                                <p className="text-2xl font-bold">{stats.inProgress}</p>
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
                <CardHeader>
                    <CardTitle>Action Logs ({logs.length})</CardTitle>
                </CardHeader>
                <CardContent>
                    <div className="overflow-x-auto">
                        <Table>
                            <TableHeader>
                                <TableRow>
                                    <TableHead>Time</TableHead>
                                    <TableHead>Action Type</TableHead>
                                    <TableHead>Component</TableHead>
                                    <TableHead>Status</TableHead>
                                    <TableHead>Duration</TableHead>
                                    <TableHead>Instance ID</TableHead>
                                    <TableHead>Metadata</TableHead>
                                    <TableHead>Error</TableHead>
                                    <TableHead className="text-right">Actions</TableHead>
                                </TableRow>
                            </TableHeader>
                            <TableBody>
                                {logs.length === 0 ? (
                                    <TableRow>
                                        <TableCell colSpan={9} className="text-center text-muted-foreground py-8">
                                            No logs found
                                        </TableCell>
                                    </TableRow>
                                ) : (
                                    logs.map((log) => (
                                        <TableRow
                                            key={log.id}
                                            className="hover:bg-muted/50 cursor-pointer transition-colors"
                                            onClick={() => {
                                                if (log.instance_id) {
                                                    setSelectedInstanceId(log.instance_id);
                                                    setTimelineModalOpen(true);
                                                }
                                            }}
                                        >
                                            <TableCell className="whitespace-nowrap">
                                                {formatDistanceToNow(parseISO(log.created_at), { addSuffix: true })}
                                            </TableCell>
                                            <TableCell>
                                                {getActionTypeBadge(log.action_type)}
                                            </TableCell>
                                            <TableCell>
                                                <span className={`font-semibold ${getComponentColor(log.component)}`}>
                                                    {log.component}
                                                </span>
                                            </TableCell>
                                            <TableCell>{getStatusBadge(log.status)}</TableCell>
                                            <TableCell className="font-mono text-sm">
                                                {formatDuration(log.duration_ms)}
                                            </TableCell>
                                            <TableCell className="font-mono text-xs">
                                                {log.instance_id ? (
                                                    <span className="bg-muted px-2 py-1 rounded">
                                                        {log.instance_id.substring(0, 8)}...
                                                    </span>
                                                ) : "-"}
                                            </TableCell>
                                            <TableCell className="max-w-xs truncate">
                                                {formatMetadata(log.metadata)}
                                            </TableCell>
                                            <TableCell className="text-red-600 text-sm max-w-xs truncate">
                                                {log.error_message || "-"}
                                            </TableCell>
                                            <TableCell className="text-right">
                                                <button
                                                    onClick={(e) => copyLogToClipboard(log, e)}
                                                    className="inline-flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-md transition-all hover:bg-muted focus:outline-none focus:ring-2 focus:ring-primary focus:ring-offset-1"
                                                    title="Copy log details"
                                                >
                                                    {copiedLogId === log.id ? (
                                                        <>
                                                            <Check className="h-3.5 w-3.5 text-green-600" />
                                                            <span className="text-green-600">Copied!</span>
                                                        </>
                                                    ) : (
                                                        <>
                                                            <Copy className="h-3.5 w-3.5" />
                                                            <span>Copy</span>
                                                        </>
                                                    )}
                                                </button>
                                            </TableCell>
                                        </TableRow>
                                    ))
                                )}
                            </TableBody>
                        </Table>
                    </div>
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
