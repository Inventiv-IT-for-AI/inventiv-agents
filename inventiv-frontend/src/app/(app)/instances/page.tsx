"use client";

import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Plus, RefreshCcw } from "lucide-react";
import { Instance } from "@/lib/types";
import { apiUrl } from "@/lib/api";
import { useInstances } from "@/hooks/useInstances";
import { useRealtimeEvents } from "@/hooks/useRealtimeEvents";
import { useCatalog } from "@/hooks/useCatalog";
import { StatsCard } from "@/components/shared/StatsCard";
import { CreateInstanceModal } from "@/components/instances/CreateInstanceModal";
import { TerminateInstanceModal } from "@/components/instances/TerminateInstanceModal";
import { ReinstallInstanceModal } from "@/components/instances/ReinstallInstanceModal";
import { Server, Activity, AlertCircle, RefreshCcw as RefreshIcon } from "lucide-react";
import { InstanceTable } from "@/components/instances/InstanceTable";
import { InstanceTimelineModal } from "@/components/instances/InstanceTimelineModal";

export default function InstancesPage() {
    useRealtimeEvents();
    const { instances, refreshInstances } = useInstances();
    const catalog = useCatalog();

    const [refreshSeq, setRefreshSeq] = useState(0);

    const [isCreateOpen, setIsCreateOpen] = useState(false);
    const [isTerminateOpen, setIsTerminateOpen] = useState(false);
    const [instanceToTerminate, setInstanceToTerminate] = useState<string | null>(null);
    const [isReinstallOpen, setIsReinstallOpen] = useState(false);
    const [instanceToReinstall, setInstanceToReinstall] = useState<string | null>(null);
    const [isTimelineOpen, setIsTimelineOpen] = useState(false);
    const [selectedInstanceId, setSelectedInstanceId] = useState<string | null>(null);

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

    const openReinstallModal = (id: string) => {
        setInstanceToReinstall(id);
        setIsReinstallOpen(true);
    };

    const openTimelineModal = (instance: Instance) => {
        setSelectedInstanceId(instance.id);
        setIsTimelineOpen(true);
    };

    useEffect(() => {
        const handler = () => setRefreshSeq((v) => v + 1);
        window.addEventListener("refresh-instances", handler);
        return () => window.removeEventListener("refresh-instances", handler);
    }, []);

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
                    <Button
                        variant="outline"
                        size="icon"
                        onClick={() => {
                            refreshInstances();
                            setRefreshSeq((v) => v + 1);
                        }}
                    >
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
                <CardContent>
                    <InstanceTable
                        onViewDetails={openTimelineModal}
                        onTerminate={openTerminateModal}
                        onReinstall={openReinstallModal}
                        onArchive={handleArchive}
                        refreshKey={String(refreshSeq)}
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

            {/* Reinstall Instance Modal */}
            <ReinstallInstanceModal
                open={isReinstallOpen}
                onClose={() => {
                    setIsReinstallOpen(false);
                    setInstanceToReinstall(null);
                }}
                instanceId={instanceToReinstall}
                onSuccess={refreshInstances}
            />

            {/* Instance Actions / Timeline Modal */}
            {selectedInstanceId ? (
                <InstanceTimelineModal
                    open={isTimelineOpen}
                    onClose={() => setIsTimelineOpen(false)}
                    instanceId={selectedInstanceId}
                />
            ) : null}
        </div>
    );
}


