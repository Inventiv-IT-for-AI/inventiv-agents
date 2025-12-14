"use client";

import { useEffect, useMemo, useState } from "react";
import { formatDistanceToNow, parseISO } from "date-fns";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { RefreshCcw, Eye } from "lucide-react";
import { apiUrl } from "@/lib/api";
import { displayOrDash, formatEur } from "@/lib/utils";
import type { Instance } from "@/lib/types";
import { VirtualizedDataTable, type DataTableColumn } from "@/components/shared/VirtualizedDataTable";

export default function Traces() {
    const [instances, setInstances] = useState<Instance[]>([]);
    const [isDetailsOpen, setIsDetailsOpen] = useState(false);
    const [selectedInstance, setSelectedInstance] = useState<Instance | null>(null);

    const openDetailsModal = (instance: Instance) => {
        setSelectedInstance(instance);
        setIsDetailsOpen(true);
    };

    useEffect(() => {
        const fetchData = async () => {
            try {
                const res = await fetch(apiUrl("instances?archived=true"));
                if (res.ok) {
                    const data = await res.json();
                    setInstances(data);
                }
            } catch (err) {
                console.error("Polling Error:", err);
            }
        };
        fetchData();
    }, []);

    const columns = useMemo<DataTableColumn<Instance>[]>(() => {
        return [
            {
                id: "id",
                label: "ID",
                width: 140,
                cell: ({ row }) => <span className="font-mono text-xs">{row.id.split("-")[0]}...</span>,
            },
            {
                id: "provider",
                label: "Provider",
                width: 140,
                cell: ({ row }) => displayOrDash(row.provider_name),
            },
            {
                id: "region",
                label: "Region",
                width: 160,
                cell: ({ row }) => displayOrDash(row.region),
            },
            {
                id: "zone",
                label: "Zone",
                width: 140,
                cell: ({ row }) => displayOrDash(row.zone),
            },
            {
                id: "type",
                label: "Type",
                width: 220,
                cell: ({ row }) => displayOrDash(row.instance_type),
            },
            {
                id: "cost",
                label: "Cost",
                width: 120,
                align: "right",
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
                cell: ({ row }) => <Badge variant="secondary">{row.status}</Badge>,
            },
            {
                id: "created",
                label: "Created",
                width: 170,
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
                cell: ({ row }) => <span className="text-muted-foreground font-mono">{displayOrDash(row.ip_address)}</span>,
            },
            {
                id: "actions",
                label: "Actions",
                width: 120,
                align: "right",
                disableReorder: true,
                cell: ({ row }) => (
                    <Button
                        variant="ghost"
                        size="icon"
                        onClick={(e) => {
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
                    <Button variant="outline" size="icon" onClick={() => window.location.reload()}>
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
                    <VirtualizedDataTable<Instance>
                        listId="traces:archived_instances"
                        title="Archived Instances"
                        height={560}
                        rowHeight={56}
                        columns={columns}
                        rows={instances}
                        getRowKey={(r) => r.id}
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
