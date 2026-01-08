"use client";

import { useCallback, useMemo, useState, type MouseEvent } from "react";
import { formatDistanceToNow, parseISO } from "date-fns";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { RefreshCcw, Eye } from "lucide-react";
import { apiUrl } from "@/lib/api";
import { displayOrDash, formatEur } from "@/lib/utils";
import type { Instance } from "@/lib/types";
import { IADataTable, type DataTableSortState, type IADataTableColumn, type LoadRangeResult } from "ia-widgets";
export default function Traces() {
    const [refreshTick, setRefreshTick] = useState(0);
    const [sort, setSort] = useState<DataTableSortState>(null);
    const [isDetailsOpen, setIsDetailsOpen] = useState(false);
    const [selectedInstance, setSelectedInstance] = useState<Instance | null>(null);

    const openDetailsModal = (instance: Instance) => {
        setSelectedInstance(instance);
        setIsDetailsOpen(true);
    };

    type InstancesSearchResponse = {
        offset: number;
        limit: number;
        total_count: number;
        filtered_count: number;
        rows: Instance[];
    };

    const loadRange = useCallback(
        async (offset: number, limit: number): Promise<LoadRangeResult<Instance>> => {
            const params = new URLSearchParams();
            params.set("archived", "true");
            params.set("offset", String(offset));
            params.set("limit", String(limit));
            if (sort) {
                const by = ({ provider: "provider", region: "region", zone: "zone", type: "type", cost: "total_cost", status: "status", created: "created_at" } as Record<string, string>)[sort.columnId];
                if (by) {
                    params.set("sort_by", by);
                    params.set("sort_dir", sort.direction);
                }
            }
            const res = await fetch(apiUrl(`instances/search?${params.toString()}`));
            if (!res.ok) throw new Error(`instances/search failed (${res.status})`);
            const data = (await res.json()) as InstancesSearchResponse;
            return { offset: data.offset, items: data.rows, totalCount: data.total_count, filteredCount: data.filtered_count };
        },
        [sort]
    );

    const columns = useMemo<IADataTableColumn<Instance>[]>(() => {
        return [
            {
                id: "id",
                label: "ID",
                width: 140,
                sortable: false,
                cell: ({ row }) => <span className="font-mono text-xs">{row.id.split("-")[0]}...</span>,
            },
            {
                id: "provider",
                label: "Provider",
                width: 140,
                sortable: true,
                cell: ({ row }) => displayOrDash(row.provider_name),
            },
            {
                id: "region",
                label: "Region",
                width: 160,
                sortable: true,
                cell: ({ row }) => displayOrDash(row.region),
            },
            {
                id: "zone",
                label: "Zone",
                width: 140,
                sortable: true,
                cell: ({ row }) => displayOrDash(row.zone),
            },
            {
                id: "type",
                label: "Type",
                width: 220,
                sortable: true,
                cell: ({ row }) => displayOrDash(row.instance_type),
            },
            {
                id: "cost",
                label: "Cost",
                width: 120,
                align: "right",
                sortable: true,
                cell: ({ row }) => (
                    <span className="font-mono">
                        {row.total_cost != null ? formatEur(row.total_cost, { minFrac: 4, maxFrac: 4 }) : "-"}
                    </span>
                ),
            },
            {
                id: "status",
                label: "Status",
                width: 140,
                sortable: true,
                cell: ({ row }) => <Badge variant="secondary">{row.status}</Badge>,
            },
            {
                id: "created",
                label: "Created",
                width: 170,
                sortable: true,
                cell: ({ row }) => (
                    <span className="whitespace-nowrap text-muted-foreground">
                        {formatDistanceToNow(parseISO(row.created_at), { addSuffix: true })}
                    </span>
                ),
            },
            {
                id: "ip",
                label: "IP Address",
                width: 160,
                sortable: false,
                cell: ({ row }) => <span className="text-muted-foreground font-mono">{displayOrDash(row.ip_address)}</span>,
            },
            {
                id: "actions",
                label: "Actions",
                width: 120,
                align: "right",
                disableReorder: true,
                sortable: false,
                cell: ({ row }) => (
                    <Button
                        variant="ghost"
                        size="icon"
                        onClick={(e: MouseEvent<HTMLButtonElement>) => {
                            e.stopPropagation();
                            openDetailsModal(row);
                        }}
                        title="Voir détails"
                    >
                        <Eye className="h-4 w-4" />
                    </Button>
                ),
            },
        ];
    }, []);

    return (
        <div className="p-8 space-y-8">
            {/* Header */}
            <div className="flex items-center justify-between">
                <div>
                    <h1 className="text-3xl font-bold tracking-tight">Traces</h1>
                    <p className="text-muted-foreground">Archived instances history.</p>
                </div>
                <div className="flex space-x-2">
                    <Button variant="outline" size="icon" onClick={() => setRefreshTick((v) => v + 1)}>
                        <RefreshCcw className="h-4 w-4" />
                    </Button>
                </div>
            </div>

            {/* Instances Table */}
            <Card className="col-span-4">
                <CardHeader>
                    <CardTitle>Archived Instances</CardTitle>
                </CardHeader>
                <CardContent>
                    <IADataTable<Instance>
                        listId="traces:archived_instances"
                        title="Archived Instances"
                        dataKey={JSON.stringify({ refreshTick, sort })}
                        autoHeight={true}
                        height={300}
                        rowHeight={56}
                        columns={columns}
                        loadRange={loadRange}
                        sortState={sort}
                        onSortChange={setSort}
                        sortingMode="server"
                        onRowClick={openDetailsModal}
                    />
                </CardContent>
            </Card>

            <Dialog open={isDetailsOpen} onOpenChange={setIsDetailsOpen}>
                <DialogContent showCloseButton={false} className="sm:max-w-[600px]">
                    <DialogHeader>
                        <DialogTitle>Instance Details</DialogTitle>
                    </DialogHeader>
                    {selectedInstance && (
                        <div className="grid gap-6 py-4">
                            <div className="grid grid-cols-2 gap-4">
                                <div>
                                    <h4 className="font-semibold mb-2 text-sm text-muted-foreground">Identity</h4>
                                    <div className="space-y-1 text-sm">
                                        <div className="flex justify-between border-b pb-1"><span>ID</span> <span className="font-mono text-xs">{selectedInstance.id}</span></div>
                                        <div className="flex justify-between border-b pb-1"><span>Provider</span> <span className="font-medium">{displayOrDash(selectedInstance.provider_name)}</span></div>
                                        <div className="flex justify-between border-b pb-1"><span>Region</span> <span>{displayOrDash(selectedInstance.region)}</span></div>
                                        <div className="flex justify-between border-b pb-1"><span>Zone</span> <span>{displayOrDash(selectedInstance.zone)}</span></div>
                                    </div>
                                </div>
                                <div>
                                    <h4 className="font-semibold mb-2 text-sm text-muted-foreground">Specs & Status</h4>
                                    <div className="space-y-1 text-sm">
                                        <div className="flex justify-between border-b pb-1"><span>Type</span> <span className="font-medium">{displayOrDash(selectedInstance.instance_type)}</span></div>
                                        <div className="flex justify-between border-b pb-1"><span>VRAM</span> <span>{selectedInstance.gpu_vram ? `${selectedInstance.gpu_vram} GB` : '-'}</span></div>
                                        <div className="flex justify-between border-b pb-1"><span>Rate</span> <span>{selectedInstance.cost_per_hour != null ? `${formatEur(selectedInstance.cost_per_hour, { minFrac: 4, maxFrac: 4 })}/h` : '-'}</span></div>
                                        <div className="flex justify-between border-b pb-1"><span>Total Cost</span> <span className="font-bold text-green-600">{selectedInstance.total_cost != null ? formatEur(selectedInstance.total_cost, { minFrac: 4, maxFrac: 4 }) : '-'}</span></div>
                                        <div className="flex justify-between border-b pb-1"><span>Status</span> <Badge variant="secondary">{selectedInstance.status}</Badge></div>
                                        <div className="flex justify-between border-b pb-1"><span>Created</span> <span>{formatDistanceToNow(parseISO(selectedInstance.created_at), { addSuffix: true })}</span></div>
                                    </div>
                                </div>
                            </div>
                        </div>
                    )}
                </DialogContent>
            </Dialog>
        </div>
    );
}


