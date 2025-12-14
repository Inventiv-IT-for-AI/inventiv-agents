"use client";

import { useEffect, useState } from "react";
import { formatDistanceToNow, parseISO } from "date-fns";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Table, TableHeader, TableRow, TableHead, TableBody, TableCell } from "@/components/ui/table";
import { RefreshCcw, Eye } from "lucide-react";
import { apiUrl } from "@/lib/api";

type Instance = {
    id: string;
    provider_id: string;
    provider_name: string;
    zone: string;
    region: string;
    instance_type: string;
    status: string;
    ip_address: string | null;
    created_at: string;
    gpu_vram?: number;
    cost_per_hour?: number;
    total_cost?: number;
};


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
                    <Table>
                        <TableHeader>
                            <TableRow>
                                <TableHead>ID</TableHead>
                                <TableHead>Provider</TableHead>
                                <TableHead>Region</TableHead>
                                <TableHead>Zone</TableHead>
                                <TableHead>Type</TableHead>
                                <TableHead>Cost</TableHead>
                                <TableHead>Status</TableHead>
                                <TableHead>Created</TableHead>
                                <TableHead>IP Address</TableHead>
                                <TableHead className="text-right">Actions</TableHead>
                            </TableRow>
                        </TableHeader>
                        <TableBody>
                            {instances.map((instance) => (
                                <TableRow key={instance.id}>
                                    <TableCell className="font-mono text-xs">{instance.id.split('-')[0]}...</TableCell>
                                    <TableCell>{instance.provider_name}</TableCell>
                                    <TableCell>{instance.region}</TableCell>
                                    <TableCell>{instance.zone}</TableCell>
                                    <TableCell>{instance.instance_type}</TableCell>
                                    <TableCell className="font-mono">
                                        {instance.total_cost != null ? `$${instance.total_cost.toFixed(4)}` : '-'}
                                    </TableCell>
                                    <TableCell>
                                        <Badge variant="secondary">
                                            {instance.status}
                                        </Badge>
                                    </TableCell>
                                    <TableCell className="whitespace-nowrap text-muted-foreground">
                                        {formatDistanceToNow(parseISO(instance.created_at), { addSuffix: true })}
                                    </TableCell>
                                    <TableCell className="font-mono text-sm">
                                        <span className="text-muted-foreground">-</span>
                                    </TableCell>
                                    <TableCell className="text-right">
                                        <div className="flex justify-end space-x-2">
                                            <Button variant="ghost" size="icon" onClick={() => openDetailsModal(instance)}>
                                                <Eye className="h-4 w-4" />
                                            </Button>
                                        </div>
                                    </TableCell>
                                </TableRow>
                            ))}
                            {instances.length === 0 && (
                                <TableRow>
                                    <TableCell colSpan={10} className="text-center h-24 text-muted-foreground">
                                        No traces found.
                                    </TableCell>
                                </TableRow>
                            )}
                        </TableBody>
                    </Table>
                </CardContent>
            </Card>

            <Dialog open={isDetailsOpen} onOpenChange={setIsDetailsOpen}>
                <DialogContent className="sm:max-w-[600px]">
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
                                        <div className="flex justify-between border-b pb-1"><span>Provider</span> <span className="font-medium">{selectedInstance.provider_name}</span></div>
                                        <div className="flex justify-between border-b pb-1"><span>Region</span> <span>{selectedInstance.region}</span></div>
                                        <div className="flex justify-between border-b pb-1"><span>Zone</span> <span>{selectedInstance.zone}</span></div>
                                    </div>
                                </div>
                                <div>
                                    <h4 className="font-semibold mb-2 text-sm text-muted-foreground">Specs & Status</h4>
                                    <div className="space-y-1 text-sm">
                                        <div className="flex justify-between border-b pb-1"><span>Type</span> <span className="font-medium">{selectedInstance.instance_type}</span></div>
                                        <div className="flex justify-between border-b pb-1"><span>VRAM</span> <span>{selectedInstance.gpu_vram ? `${selectedInstance.gpu_vram} GB` : '-'}</span></div>
                                        <div className="flex justify-between border-b pb-1"><span>Rate</span> <span>{selectedInstance.cost_per_hour != null ? `$${selectedInstance.cost_per_hour}/hr` : '-'}</span></div>
                                        <div className="flex justify-between border-b pb-1"><span>Total Cost</span> <span className="font-bold text-green-600">{selectedInstance.total_cost != null ? `$${selectedInstance.total_cost.toFixed(4)}` : '-'}</span></div>
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
