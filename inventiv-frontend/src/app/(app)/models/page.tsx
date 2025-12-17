"use client";

import { useEffect, useMemo, useState } from "react";
import { apiUrl } from "@/lib/api";
import type { RuntimeModel } from "@/lib/types";
import { Card, CardContent } from "@/components/ui/card";
import { VirtualizedDataTable, type DataTableColumn } from "@/components/shared/VirtualizedDataTable";
import { useRealtimeEvents } from "@/hooks/useRealtimeEvents";
import { Button } from "@/components/ui/button";

export default function ModelsPage() {
    useRealtimeEvents();
    const [rows, setRows] = useState<RuntimeModel[]>([]);
    const [refreshTick, setRefreshTick] = useState(0);

    const load = async () => {
        const res = await fetch(apiUrl("runtime/models"));
        if (!res.ok) return;
        const data = (await res.json()) as RuntimeModel[];
        setRows(data);
    };

    useEffect(() => {
        void load().catch(() => null);
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [refreshTick]);

    useEffect(() => {
        const onRefresh = () => setRefreshTick((t) => t + 1);
        window.addEventListener("refresh-instances", onRefresh as any);
        return () => window.removeEventListener("refresh-instances", onRefresh as any);
    }, []);

    const columns: DataTableColumn<RuntimeModel>[] = useMemo(
        () => [
            {
                id: "model_id",
                label: "Model",
                width: 420,
                cell: ({ row }) => <span className="font-mono text-xs">{row.model_id}</span>,
            },
            {
                id: "available",
                label: "Available",
                width: 120,
                cell: ({ row }) =>
                    row.instances_available > 0 ? (
                        <span className="text-xs px-2 py-1 rounded bg-green-200 text-green-800">yes</span>
                    ) : (
                        <span className="text-xs px-2 py-1 rounded bg-gray-200 text-gray-700">no</span>
                    ),
            },
            {
                id: "instances",
                label: "Instances",
                width: 120,
                cell: ({ row }) => <span className="tabular-nums">{row.instances_available}</span>,
            },
            {
                id: "gpus",
                label: "GPUs",
                width: 100,
                cell: ({ row }) => <span className="tabular-nums">{row.gpus_available}</span>,
            },
            {
                id: "vram",
                label: "VRAM (GB)",
                width: 130,
                cell: ({ row }) => <span className="tabular-nums">{row.vram_total_gb}</span>,
            },
            {
                id: "req_total",
                label: "Requests",
                width: 130,
                cell: ({ row }) => <span className="tabular-nums">{row.total_requests}</span>,
            },
            {
                id: "req_failed",
                label: "Failed",
                width: 110,
                cell: ({ row }) => <span className="tabular-nums">{row.failed_requests}</span>,
            },
            {
                id: "last_seen",
                label: "Last seen",
                width: 190,
                cell: ({ row }) => <span className="text-sm text-muted-foreground">{new Date(row.last_seen_at).toLocaleString()}</span>,
            },
            {
                id: "first_seen",
                label: "First seen",
                width: 190,
                cell: ({ row }) => <span className="text-sm text-muted-foreground">{new Date(row.first_seen_at).toLocaleString()}</span>,
            },
        ],
        []
    );

    return (
        <div className="p-8 space-y-8">
            <div className="flex items-center justify-between">
                <div>
                    <h1 className="text-3xl font-bold tracking-tight">Models</h1>
                    <p className="text-muted-foreground">
                        Runtime models seen on workers (capacity + request counters). Catalog is in Settings → Models.
                    </p>
                </div>
                <Button variant="secondary" onClick={() => setRefreshTick((t) => t + 1)}>
                    Refresh
                </Button>
            </div>

            <Card>
                <CardContent>
                    <VirtualizedDataTable<RuntimeModel>
                        listId="models:runtime"
                        title="Runtime Models"
                        dataKey={String(refreshTick)}
                        autoHeight={true}
                        height={480}
                        rowHeight={52}
                        columns={columns}
                        rows={rows}
                    />
                </CardContent>
            </Card>
        </div>
    );
}
