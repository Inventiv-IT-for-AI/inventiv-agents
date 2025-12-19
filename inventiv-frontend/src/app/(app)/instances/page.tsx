"use client";

import { useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Plus, RefreshCcw } from "lucide-react";
import { Instance } from "@/lib/types";
import { useInstances } from "@/hooks/useInstances";
import { useRealtimeEvents } from "@/hooks/useRealtimeEvents";
import { useCatalog } from "@/hooks/useCatalog";
import { IAStatsCard } from "ia-widgets";
import { CreateInstanceModal } from "@/components/instances/CreateInstanceModal";
import { TerminateInstanceModal } from "@/components/instances/TerminateInstanceModal";
import { ReinstallInstanceModal } from "@/components/instances/ReinstallInstanceModal";
import { Server, Activity, AlertCircle, RefreshCcw as RefreshIcon } from "lucide-react";
import { InstanceTable } from "@/components/instances/InstanceTable";
import { InstanceTimelineModal } from "@/components/instances/InstanceTimelineModal";
import { ArchiveInstanceModal } from "@/components/instances/ArchiveInstanceModal";
import { WorkspaceBanner } from "@/components/shared/WorkspaceBanner";

export default function InstancesPage() {
    useRealtimeEvents();
    const { instances, refreshInstances } = useInstances();
    const catalog = useCatalog();

    const [refreshSeq, setRefreshSeq] = useState(0);
    const refreshTimerRef = useRef<number | null>(null);

    const [isCreateOpen, setIsCreateOpen] = useState(false);
    const [isTerminateOpen, setIsTerminateOpen] = useState(false);
    const [instanceToTerminate, setInstanceToTerminate] = useState<string | null>(null);
    const [isReinstallOpen, setIsReinstallOpen] = useState(false);
    const [instanceToReinstall, setInstanceToReinstall] = useState<string | null>(null);
    const [isArchiveOpen, setIsArchiveOpen] = useState(false);
    const [instanceToArchive, setInstanceToArchive] = useState<string | null>(null);
    const [isTimelineOpen, setIsTimelineOpen] = useState(false);
    const [selectedInstanceId, setSelectedInstanceId] = useState<string | null>(null);

    const openCreateModal = async () => {
        await catalog.fetchCatalog();
        setIsCreateOpen(true);
    };

    const openArchiveModal = (id: string) => {
        setInstanceToArchive(id);
        setIsArchiveOpen(true);
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
        const schedule = () => {
            // Debounce frequent refresh bursts (SSE + polling) to avoid hammering the virtualized list.
            if (refreshTimerRef.current != null) return;
            refreshTimerRef.current = window.setTimeout(() => {
                refreshTimerRef.current = null;
                setRefreshSeq((v) => v + 1);
            }, 2000);
        };
        const handler = () => schedule();
        window.addEventListener("refresh-instances", handler);
        return () => {
            window.removeEventListener("refresh-instances", handler);
            if (refreshTimerRef.current != null) {
                window.clearTimeout(refreshTimerRef.current);
                refreshTimerRef.current = null;
            }
        };
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
                            // If no event fires (edge case), still bump the reload token.
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

            <WorkspaceBanner />

            {/* Stats */}
            <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
                <IAStatsCard
                    title="Total Instances"
                    value={stats.total}
                    description="All time managed"
                    icon={Server}
                />
                <IAStatsCard
                    title="Active"
                    value={stats.active}
                    description="Operational"
                    icon={Activity}
                    valueClassName="text-green-600"
                />
                <IAStatsCard
                    title="Provisioning"
                    value={stats.provisioning}
                    icon={RefreshIcon}
                    valueClassName="text-blue-600"
                />
                <IAStatsCard
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
                        onArchive={openArchiveModal}
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

            <ArchiveInstanceModal
                open={isArchiveOpen}
                onClose={() => {
                    setIsArchiveOpen(false);
                    setInstanceToArchive(null);
                }}
                instanceId={instanceToArchive}
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


