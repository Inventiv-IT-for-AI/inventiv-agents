"use client";

import { useMemo, useState, useEffect } from "react";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { CheckCircle, Search } from "lucide-react";
import { apiUrl } from "@/lib/api";
import { Provider, Region, Zone, InstanceType } from "@/lib/types";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { formatEur } from "@/lib/utils";

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

    const [selectedProviderCode, setSelectedProviderCode] = useState<string>("");
    const [selectedRegionId, setSelectedRegionId] = useState<string>("");
    const [selectedZoneId, setSelectedZoneId] = useState<string>("");
    const [selectedTypeId, setSelectedTypeId] = useState<string>("");
    const [isTypePickerOpen, setIsTypePickerOpen] = useState(false);
    const [typeQuery, setTypeQuery] = useState("");

    const selectedProviderId = useMemo(() => {
        const p = providers.find((pp) => pp.code === selectedProviderCode);
        return p?.id ?? "";
    }, [providers, selectedProviderCode]);

    const selectedType = instanceTypes.find((t) => t.id === selectedTypeId);

    const formatMoney = (n: number) => formatEur(n, { maxFrac: 4 });

    const selectedTypePricing = useMemo(() => {
        if (!selectedType || typeof selectedType.cost_per_hour !== "number") return null;
        const perHour = selectedType.cost_per_hour;
        return {
            perMin: perHour / 60,
            perHour,
            perDay: perHour * 24,
            perMonth: perHour * 24 * 30,
        };
    }, [selectedType]);

    const filteredTypesForPicker = useMemo(() => {
        const q = typeQuery.trim().toLowerCase();
        if (!q) return instanceTypes;
        return instanceTypes.filter((t) => {
            const hay = `${t.name ?? ""} ${t.code ?? ""}`.toLowerCase();
            return hay.includes(q);
        });
    }, [instanceTypes, typeQuery]);

    // Initialize provider when modal opens
    useEffect(() => {
        if (open && providers.length > 0) {
            // eslint-disable-next-line react-hooks/set-state-in-effect
            setSelectedProviderCode(providers[0].code);
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
    }, [selectedProviderCode, selectedProviderId, open, allZones, initialInstanceTypes]);

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
                const qs = selectedProviderCode ? `?provider_code=${encodeURIComponent(selectedProviderCode)}` : "";
                const res = await fetch(apiUrl(`zones/${selectedZoneId}/instance_types${qs}`));
                if (res.ok) {
                    const data: InstanceType[] = await res.json();
                    // Merge zone-filtered types with full provider catalog.
                    // Reason: availability mappings (instance_type_zones / legacy) can be incomplete,
                    // but we still want to show all configured types to the user.
                    const base = initialInstanceTypes.filter((t) => !selectedProviderId || t.provider_id === selectedProviderId);
                    const byId = new Map<string, InstanceType>();
                    for (const t of base) byId.set(t.id, t);
                    for (const t of data) byId.set(t.id, { ...byId.get(t.id), ...t });

                    const merged = Array.from(byId.values()).sort((a, b) => {
                        const ag = (a.gpu_count ?? 0);
                        const bg = (b.gpu_count ?? 0);
                        if (bg !== ag) return bg - ag;
                        const av = (a.vram_per_gpu_gb ?? 0);
                        const bv = (b.vram_per_gpu_gb ?? 0);
                        if (bv !== av) return bv - av;
                        return (a.name || "").localeCompare(b.name || "");
                    });

                    setInstanceTypes(merged);
                    // Reset instance type selection when zone changes
                    setSelectedTypeId("");
                }
            } catch (err) {
                console.error("Failed to fetch instance types for zone", err);
            }
        };
        fetchTypesForZone();
    }, [selectedZoneId, initialInstanceTypes, selectedProviderId, selectedProviderCode]);

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
                    provider_code: selectedProviderCode || undefined,
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
        setSelectedProviderCode("");
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
                            <Select value={selectedProviderCode} onValueChange={setSelectedProviderCode}>
                                <SelectTrigger className="col-span-3">
                                    <SelectValue />
                                </SelectTrigger>
                                <SelectContent>
                                    {providers.map((p) => (
                                        <SelectItem key={p.id} value={p.code}>
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
                                disabled={!selectedProviderCode}
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
                            <Button
                                type="button"
                                variant="outline"
                                className="col-span-3 justify-between"
                                onClick={() => setIsTypePickerOpen(true)}
                                disabled={!selectedZoneId || deployStep === "submitting"}
                            >
                                <span className="truncate">
                                    {selectedType
                                        ? `${selectedType.name} — ${selectedType.gpu_count ?? "-"} GPU • ${selectedType.vram_per_gpu_gb ?? "-"}GB VRAM/GPU`
                                        : "Choisir un type"}
                                </span>
                                <Search className="h-4 w-4 opacity-60" />
                            </Button>
                        </div>

                        {selectedType && (
                            <div className="grid grid-cols-4 items-start gap-4 bg-muted/50 p-3 rounded-md">
                                <Label className="text-right text-muted-foreground">Détails</Label>
                                <div className="col-span-3 space-y-1">
                                    <div className="text-sm">
                                        <span className="font-medium">{selectedType.name}</span>{" "}
                                        <span className="text-muted-foreground">
                                            ({selectedType.gpu_count ?? "-"} GPU,{" "}
                                            {selectedType.vram_per_gpu_gb ?? "-"}GB VRAM/GPU
                                            {typeof selectedType.vram_per_gpu_gb === "number" && typeof selectedType.gpu_count === "number"
                                                ? ` — ${(selectedType.vram_per_gpu_gb * selectedType.gpu_count)}GB total`
                                                : ""}
                                            )
                                        </span>
                                    </div>
                                    {selectedTypePricing ? (
                                        <div className="text-xs text-muted-foreground font-mono">
                                            {formatMoney(selectedTypePricing.perMin)}/min • {formatMoney(selectedTypePricing.perHour)}/h • {formatMoney(selectedTypePricing.perDay)}/j • {formatMoney(selectedTypePricing.perMonth)}/mois
                                        </div>
                                    ) : (
                                        <div className="text-xs text-muted-foreground">Prix indisponible</div>
                                    )}
                                </div>
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

            {/* Instance Type Picker */}
            <Dialog open={isTypePickerOpen} onOpenChange={setIsTypePickerOpen}>
                <DialogContent showCloseButton={false} className="sm:max-w-[720px]">
                    <DialogHeader>
                        <DialogTitle>Choisir un type d&apos;instance</DialogTitle>
                        <DialogDescription>
                            Rechercher et sélectionner le type le plus adapté (GPU, VRAM, coût).
                        </DialogDescription>
                    </DialogHeader>

                    <div className="flex items-center gap-2">
                        <Input
                            placeholder="Rechercher (ex: H100, L40S, RENDER...)"
                            value={typeQuery}
                            onChange={(e) => setTypeQuery(e.target.value)}
                        />
                    </div>

                    <ScrollArea className="h-[420px] rounded-md border">
                        <div className="p-2 space-y-2">
                            {filteredTypesForPicker.length === 0 ? (
                                <div className="text-sm text-muted-foreground p-4">Aucun type ne correspond à la recherche.</div>
                            ) : (
                                filteredTypesForPicker.map((t) => {
                                    const gpus = t.gpu_count ?? 0;
                                    const vram = t.vram_per_gpu_gb ?? 0;
                                    const perHour = typeof t.cost_per_hour === "number" ? t.cost_per_hour : null;
                                    const isSelected = t.id === selectedTypeId;
                                    return (
                                        <button
                                            key={t.id}
                                            type="button"
                                            onClick={() => {
                                                setSelectedTypeId(t.id);
                                                setIsTypePickerOpen(false);
                                            }}
                                            className={`w-full text-left rounded-md border p-3 hover:bg-muted/30 transition-colors ${
                                                isSelected ? "border-primary bg-muted/20" : "border-border bg-background"
                                            }`}
                                        >
                                            <div className="flex items-start justify-between gap-3">
                                                <div className="min-w-0">
                                                    <div className="font-medium truncate">{t.name}</div>
                                                    <div className="text-xs text-muted-foreground">
                                                        {gpus} GPU • {vram}GB VRAM/GPU{gpus && vram ? ` • ${gpus * vram}GB total` : ""}
                                                        {t.cpu_count ? ` • ${t.cpu_count} vCPU` : ""}
                                                        {t.ram_gb ? ` • ${t.ram_gb}GB RAM` : ""}
                                                    </div>
                                                </div>
                                                <div className="text-right shrink-0">
                                                    {perHour != null ? (
                                                        <div className="text-xs font-mono text-muted-foreground">
                                                            {formatMoney(perHour / 60)}/min
                                                            <div className="text-sm font-semibold text-foreground">{formatMoney(perHour)}/h</div>
                                                            <div>{formatMoney(perHour * 24)}/j</div>
                                                            <div>{formatMoney(perHour * 24 * 30)}/mois</div>
                                                        </div>
                                                    ) : (
                                                        <div className="text-xs text-muted-foreground">Prix indisponible</div>
                                                    )}
                                                </div>
                                            </div>
                                        </button>
                                    );
                                })
                            )}
                        </div>
                    </ScrollArea>

                    <DialogFooter className="sm:justify-between">
                        <Button
                            variant="outline"
                            onClick={() => {
                                setIsTypePickerOpen(false);
                                setTypeQuery("");
                            }}
                        >
                            Fermer
                        </Button>
                        <Button
                            onClick={() => {
                                setIsTypePickerOpen(false);
                                setTypeQuery("");
                            }}
                            disabled={!selectedTypeId}
                        >
                            Sélectionner
                        </Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>
        </Dialog>
    );
}
