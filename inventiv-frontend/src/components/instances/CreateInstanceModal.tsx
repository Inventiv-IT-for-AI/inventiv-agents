"use client";

import { useState, useEffect } from "react";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { CheckCircle } from "lucide-react";
import { apiUrl } from "@/lib/api";
import { Provider, Region, Zone, InstanceType } from "@/lib/types";

type CreateInstanceModalProps = {
    open: boolean;
    onClose: () => void;
    onSuccess: () => void;
    providers: Provider[];
    regions: Region[];
    allZones: Zone[];
    initialInstanceTypes: InstanceType[];
};

export function CreateInstanceModal({
    open,
    onClose,
    onSuccess,
    providers,
    regions,
    allZones,
    initialInstanceTypes,
}: CreateInstanceModalProps) {
    const [deployStep, setDeployStep] = useState<"form" | "submitting" | "success">("form");
    const [zones, setZones] = useState<Zone[]>([]);
    const [instanceTypes, setInstanceTypes] = useState<InstanceType[]>(initialInstanceTypes);

    const [selectedProviderId, setSelectedProviderId] = useState<string>("");
    const [selectedRegionId, setSelectedRegionId] = useState<string>("");
    const [selectedZoneId, setSelectedZoneId] = useState<string>("");
    const [selectedTypeId, setSelectedTypeId] = useState<string>("");

    const selectedType = instanceTypes.find((t) => t.id === selectedTypeId);

    // Initialize provider when modal opens
    useEffect(() => {
        if (open && providers.length > 0) {
            // eslint-disable-next-line react-hooks/set-state-in-effect
            setSelectedProviderId(providers[0].id);
        }
    }, [open, providers]);

    // Reset region/zone/type when provider changes
    useEffect(() => {
        if (!open) return;
        // eslint-disable-next-line react-hooks/set-state-in-effect
        setSelectedRegionId("");
        setSelectedZoneId("");
        setSelectedTypeId("");
        setZones(allZones);
        setInstanceTypes(initialInstanceTypes.filter((t) => !selectedProviderId || t.provider_id === selectedProviderId));
    }, [selectedProviderId, open, allZones, initialInstanceTypes]);

    // Filter zones by selected region
    useEffect(() => {
        if (!selectedRegionId) {
            // eslint-disable-next-line react-hooks/set-state-in-effect
            setZones(allZones);
            return;
        }

        const region = regions.find((r) => r.id === selectedRegionId);
        if (region) {
            const filteredZones = allZones.filter((z) => z.code.startsWith(region.code));
            setZones(filteredZones);

            // Reset zone and type selections when region changes
            setSelectedZoneId("");
            setSelectedTypeId("");
        }
    }, [selectedRegionId, regions, allZones]);

    // Filter instance types by selected zone
    useEffect(() => {
        if (!selectedZoneId) {
            // eslint-disable-next-line react-hooks/set-state-in-effect
            setInstanceTypes(initialInstanceTypes.filter((t) => !selectedProviderId || t.provider_id === selectedProviderId));
            return;
        }

        const fetchTypesForZone = async () => {
            try {
                const qs = selectedProviderId ? `?provider_id=${encodeURIComponent(selectedProviderId)}` : "";
                const res = await fetch(apiUrl(`zones/${selectedZoneId}/instance_types${qs}`));
                if (res.ok) {
                    const data: InstanceType[] = await res.json();
                    setInstanceTypes(data);
                    // Reset instance type selection when zone changes
                    setSelectedTypeId("");
                }
            } catch (err) {
                console.error("Failed to fetch instance types for zone", err);
            }
        };
        fetchTypesForZone();
    }, [selectedZoneId, initialInstanceTypes, selectedProviderId]);

    const handleDeploy = async () => {
        if (!selectedZoneId || !selectedTypeId) {
            alert("Please select all required fields");
            return;
        }

        setDeployStep("submitting");
        try {
            const selectedZone = zones.find((z) => z.id === selectedZoneId);
            const selectedType = instanceTypes.find((t) => t.id === selectedTypeId);

            const res = await fetch(apiUrl("deployments"), {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    provider_id: selectedProviderId || undefined,
                    zone: selectedZone?.code || "",
                    instance_type: selectedType?.code || "",
                }),
            });

            const data = await res.json().catch(() => null);

            if (res.ok && data?.status === "accepted") {
                setDeployStep("success");
                setTimeout(() => {
                    handleClose();
                    onSuccess();
                }, 2000);
            } else {
                const msg = data?.message || data?.status || "Deployment failed!";
                alert(msg);
                handleClose();
            }
        } catch (e) {
            console.error(e);
            alert("Error deploying instance.");
            handleClose();
        }
    };

    const handleClose = () => {
        setDeployStep("form");
        setSelectedRegionId("");
        setSelectedZoneId("");
        setSelectedTypeId("");
        onClose();
    };

    return (
        <Dialog open={open} onOpenChange={handleClose}>
            <DialogContent showCloseButton={false} className="sm:max-w-[500px]">
                <DialogHeader>
                    <DialogTitle>Create New Instance</DialogTitle>
                    <DialogDescription>
                        Configure your GPU instance parameters.
                    </DialogDescription>
                </DialogHeader>

                {deployStep === "success" ? (
                    <div className="flex flex-col items-center justify-center py-6 space-y-4 text-green-600 animate-in fade-in zoom-in duration-300">
                        <CheckCircle className="h-16 w-16" />
                        <span className="text-xl font-bold">Demande de création prise en compte</span>
                    </div>
                ) : (
                    <div className="grid gap-4 py-4">
                        <div className="grid grid-cols-4 items-center gap-4">
                            <Label className="text-right">Provider</Label>
                            <Select value={selectedProviderId} onValueChange={setSelectedProviderId}>
                                <SelectTrigger className="col-span-3">
                                    <SelectValue />
                                </SelectTrigger>
                                <SelectContent>
                                    {providers.map((p) => (
                                        <SelectItem key={p.id} value={p.id}>
                                            {p.name}
                                        </SelectItem>
                                    ))}
                                </SelectContent>
                            </Select>
                        </div>

                        <div className="grid grid-cols-4 items-center gap-4">
                            <Label className="text-right">Region</Label>
                            <Select
                                value={selectedRegionId}
                                onValueChange={(val) => setSelectedRegionId(val)}
                                disabled={!selectedProviderId}
                            >
                                <SelectTrigger className="col-span-3">
                                    <SelectValue placeholder="Select region" />
                                </SelectTrigger>
                                <SelectContent>
                                    {regions
                                        .filter((r) => !selectedProviderId || r.provider_id === selectedProviderId)
                                        .map((r) => (
                                        <SelectItem key={r.id} value={r.id}>
                                            {r.name}
                                        </SelectItem>
                                    ))}
                                </SelectContent>
                            </Select>
                        </div>

                        <div className="grid grid-cols-4 items-center gap-4">
                            <Label className="text-right">Zone</Label>
                            <Select
                                value={selectedZoneId}
                                onValueChange={setSelectedZoneId}
                                disabled={!selectedRegionId}
                            >
                                <SelectTrigger className="col-span-3">
                                    <SelectValue placeholder="Select zone" />
                                </SelectTrigger>
                                <SelectContent>
                                    {zones.map((z) => (
                                        <SelectItem key={z.id} value={z.id}>
                                            {z.name}
                                        </SelectItem>
                                    ))}
                                </SelectContent>
                            </Select>
                        </div>

                        <div className="grid grid-cols-4 items-center gap-4">
                            <Label className="text-right">Instance Type</Label>
                            <Select value={selectedTypeId} onValueChange={setSelectedTypeId}>
                                <SelectTrigger className="col-span-3">
                                    <SelectValue placeholder="Select type" />
                                </SelectTrigger>
                                <SelectContent>
                                    {instanceTypes.map((t) => (
                                        <SelectItem key={t.id} value={t.id}>
                                            {t.name} {t.cost_per_hour && `(${t.cost_per_hour}$/h)`}
                                        </SelectItem>
                                    ))}
                                </SelectContent>
                            </Select>
                        </div>

                        {selectedType && selectedType.cost_per_hour && (
                            <div className="grid grid-cols-4 items-center gap-4 bg-muted/50 p-3 rounded-md">
                                <Label className="text-right text-muted-foreground">Cost</Label>
                                <p className="col-span-3 font-semibold">
                                    ${selectedType.cost_per_hour}/hour
                                </p>
                            </div>
                        )}
                    </div>
                )}

                <DialogFooter>
                    {deployStep === "success" ? (
                        <Button variant="outline" onClick={handleClose}>
                            Fermer
                        </Button>
                    ) : (
                        <div className="flex w-full flex-col-reverse gap-2 sm:flex-row sm:justify-between">
                            <Button
                                variant="outline"
                                onClick={handleClose}
                                disabled={deployStep === "submitting"}
                            >
                                Annuler
                            </Button>
                            <Button
                                type="submit"
                                onClick={handleDeploy}
                                disabled={deployStep === "submitting"}
                            >
                                {deployStep === "submitting" ? "Créer..." : "Créer"}
                            </Button>
                        </div>
                    )}
                </DialogFooter>
            </DialogContent>
        </Dialog>
    );
}
