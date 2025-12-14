"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Plus, RefreshCcw } from "lucide-react";
import { Instance } from "@/lib/types";
import { apiUrl } from "@/lib/api";
import { useInstances } from "@/hooks/useInstances";
import { useCatalog } from "@/hooks/useCatalog";
import { StatsCard } from "@/components/shared/StatsCard";
import { CreateInstanceModal } from "@/components/instances/CreateInstanceModal";
import { TerminateInstanceModal } from "@/components/instances/TerminateInstanceModal";
import { Server, Activity, AlertCircle, RefreshCcw as RefreshIcon } from "lucide-react";
import { InstanceTable } from "@/components/instances/InstanceTable";
import { InstanceDetailsModal } from "@/components/instances/InstanceDetailsModal";

export default function InstancesPage() {
    const { instances, refreshInstances } = useInstances();
    const catalog = useCatalog();

    const [isCreateOpen, setIsCreateOpen] = useState(false);
    const [isTerminateOpen, setIsTerminateOpen] = useState(false);
    const [instanceToTerminate, setInstanceToTerminate] = useState<string | null>(null);
    const [isDetailsOpen, setIsDetailsOpen] = useState(false);
    const [selectedInstance, setSelectedInstance] = useState<Instance | null>(null);

    const openCreateModal = async () => {
        await catalog.fetchCatalog();
        setIsCreateOpen(true);
    };

    const handleArchive = async (id: string) => {
        try {
            const res = await fetch(apiUrl(`instances/${id}/archive`), { method: "PUT" });
            if (res.ok) {
                refreshInstances();
            } else {
                alert("Failed to archive");
            }
        } catch (e) {
            console.error(e);
            alert("Error archiving instance");
        }
    };

    const openTerminateModal = (id: string) => {
        setInstanceToTerminate(id);
        setIsTerminateOpen(true);
    };

    const openDetailsModal = (instance: Instance) => {
        setSelectedInstance(instance);
        setIsDetailsOpen(true);
    };

    // Calculate stats
    const stats = {
        total: instances.length,
        active: instances.filter((i) => i.status.toLowerCase() === "ready").length,
        provisioning: instances.filter((i) =>
            ["provisioning", "booting"].includes(i.status.toLowerCase())
        ).length,
        failed: instances.filter((i) =>
            ["failed", "terminated"].includes(i.status.toLowerCase())
        ).length,
    };

    return (
        <div className="p-8 space-y-8">
            {/* Header */}
            <div className="flex items-center justify-between">
                <div>
                    <h1 className="text-3xl font-bold tracking-tight">Instances</h1>
                    <p className="text-muted-foreground">Manage your GPU infrastructure</p>
                </div>
                <div className="flex space-x-2">
                    <Button variant="outline" size="icon" onClick={refreshInstances}>
                        <RefreshCcw className="h-4 w-4" />
                    </Button>
                    <Button onClick={openCreateModal}>
                        <Plus className="mr-2 h-4 w-4" /> Create Instance
                    </Button>
                </div>
            </div>

            {/* Stats */}
            <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
                <StatsCard
                    title="Total Instances"
                    value={stats.total}
                    description="All time managed"
                    icon={Server}
                />
                <StatsCard
                    title="Active"
                    value={stats.active}
                    description="Operational"
                    icon={Activity}
                    valueClassName="text-green-600"
                />
                <StatsCard
                    title="Provisioning"
                    value={stats.provisioning}
                    icon={RefreshIcon}
                    valueClassName="text-blue-600"
                />
                <StatsCard
                    title="Failed/Terminated"
                    value={stats.failed}
                    icon={AlertCircle}
                    valueClassName="text-muted-foreground"
                />
            </div>

            {/* Instances Table */}
            <Card>
                <CardHeader>
                    <CardTitle>Instances ({instances.length})</CardTitle>
                </CardHeader>
                <CardContent>
                    <InstanceTable
                        instances={instances}
                        onViewDetails={openDetailsModal}
                        onTerminate={openTerminateModal}
                        onArchive={handleArchive}
                    />
                </CardContent>
            </Card>

            {/* Create Instance Modal */}
            <CreateInstanceModal
                open={isCreateOpen}
                onClose={() => setIsCreateOpen(false)}
                onSuccess={refreshInstances}
                providers={catalog.providers}
                regions={catalog.regions}
                allZones={catalog.zones}
                initialInstanceTypes={catalog.instanceTypes}
            />

            {/* Terminate Instance Modal */}
            <TerminateInstanceModal
                open={isTerminateOpen}
                onClose={() => {
                    setIsTerminateOpen(false);
                    setInstanceToTerminate(null);
                }}
                instanceId={instanceToTerminate}
                onSuccess={refreshInstances}
            />

            <InstanceDetailsModal
                open={isDetailsOpen}
                onClose={() => setIsDetailsOpen(false)}
                instance={selectedInstance}
            />
        </div>
    );
}
