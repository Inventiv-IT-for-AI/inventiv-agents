"use client";

import { useMemo, useState, useEffect, type ChangeEvent } from "react";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Search, Microchip } from "lucide-react";
import { apiUrl } from "@/lib/api";
import { Provider, Region, Zone, InstanceType, LlmModel } from "@/lib/types";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { formatEur } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { IARequestAccepted } from "ia-widgets";
import { IAAlert, IAAlertDescription, IAAlertTitle } from "ia-designsys";

type CreateInstanceModalProps = {
    open: boolean;
    onClose: () => void;
    onSuccess: () => void;
    providers: Provider[];
    regions: Region[];
    allZones: Zone[];
};

export function CreateInstanceModal({
    open,
    onClose,
    onSuccess,
    providers,
    regions,
    allZones,
}: CreateInstanceModalProps) {
    const [deployStep, setDeployStep] = useState<"form" | "submitting" | "success">("form");
    const [errorMsg, setErrorMsg] = useState<string | null>(null);
    type Combo = {
        key: string; // `${zoneId}:${typeId}`
        provider: Provider | null;
        region: Region | null;
        zone: Zone;
        type: InstanceType;
    };
    const [baseCombos, setBaseCombos] = useState<Combo[]>([]);

    const [selectedProviderCode, setSelectedProviderCode] = useState<string>("all");
    const [selectedRegionId, setSelectedRegionId] = useState<string>("all");
    const [selectedZoneId, setSelectedZoneId] = useState<string>("all");
    const [selectedComboKey, setSelectedComboKey] = useState<string>("");
    const [typeQuery, setTypeQuery] = useState("");
    const [filterGpuCount, setFilterGpuCount] = useState<string>("all"); // "all" | number as string
    const [filterVramPerGpu, setFilterVramPerGpu] = useState<string>("all"); // "all" | number as string

    const [models, setModels] = useState<LlmModel[]>([]);
    const [selectedModelId, setSelectedModelId] = useState<string>("");
    const selectedModel = useMemo(() => {
        if (!selectedModelId) return null;
        return models.find((m) => m.id === selectedModelId) ?? null;
    }, [models, selectedModelId]);

    const selectedProviderId = useMemo(() => {
        if (selectedProviderCode === "all") return "";
        const p = providers.find((pp) => pp.code === selectedProviderCode);
        return p?.id ?? "";
    }, [providers, selectedProviderCode]);

    const selectedCombo = useMemo(() => {
        if (!selectedComboKey) return null;
        return baseCombos.find((c) => c.key === selectedComboKey) ?? null;
    }, [baseCombos, selectedComboKey]);
    // Selection is stored as a complete (provider/region/zone/type) combo.
    const zones = useMemo(() => {
        // Hierarchical restriction:
        // - Provider -> Regions -> Zones
        // - Region -> Zones
        let out = allZones.filter((z) => z.is_active);

        if (selectedProviderId) {
            const regionIds = new Set(
                regions.filter((r) => r.is_active && r.provider_id === selectedProviderId).map((r) => r.id)
            );
            out = out.filter((z) => regionIds.has(z.region_id ?? ""));
        }
        if (selectedRegionId !== "all") {
            out = out.filter((z) => z.region_id === selectedRegionId);
        }
        return out;
    }, [allZones, regions, selectedProviderId, selectedRegionId]);

    const onProviderChange = (code: string) => {
        setSelectedProviderCode(code);
        setSelectedRegionId("all");
        setSelectedZoneId("all");
        setSelectedComboKey("");
    };

    const onRegionChange = (id: string) => {
        setSelectedRegionId(id);
        setSelectedZoneId("all");
        setSelectedComboKey("");
    };

    const onZoneChange = (id: string) => {
        setSelectedZoneId(id);
        setSelectedComboKey("");
    };

    const formatMoney = (n: number) => formatEur(n, { maxFrac: 4 });

    const filteredCombos = useMemo(() => {
        const q = typeQuery.trim().toLowerCase();
        const gpuFilter = filterGpuCount === "all" ? null : Number(filterGpuCount);
        const vramFilter = filterVramPerGpu === "all" ? null : Number(filterVramPerGpu);
        return baseCombos.filter((c) => {
            const t = c.type;
            const hay = `${t.name ?? ""} ${t.code ?? ""}`.toLowerCase();
            if (q && !hay.includes(q)) return false;
            const g = t.gpu_count ?? 0;
            const v = t.vram_per_gpu_gb ?? 0;
            if (gpuFilter != null && g !== gpuFilter) return false;
            if (vramFilter != null && v < vramFilter) return false;
            return true;
        });
    }, [baseCombos, filterGpuCount, filterVramPerGpu, typeQuery]);

    const gpuCountOptions = useMemo(() => {
        const set = new Set<number>();
        for (const c of baseCombos) {
            const g = c.type.gpu_count ?? 0;
            if (g > 0) set.add(g);
        }
        return Array.from(set).sort((a, b) => a - b);
    }, [baseCombos]);

    const vramOptions = useMemo(() => {
        const set = new Set<number>();
        for (const c of baseCombos) {
            const v = c.type.vram_per_gpu_gb ?? 0;
            if (v > 0) set.add(v);
        }
        return Array.from(set).sort((a, b) => a - b);
    }, [baseCombos]);

    // Build complete Provider/Region/Zone/Type combinations from backend mappings.
    // This ensures selection is never "missing zone".
    const typesByZoneRef = useMemo(() => new Map<string, InstanceType[]>(), []);

    useEffect(() => {
        if (!open) return;
        let cancelled = false;

        const candidateZones: Zone[] = (() => {
            if (selectedZoneId !== "all") {
                const z = zones.find((zz) => zz.id === selectedZoneId);
                return z ? [z] : [];
            }
            return zones;
        })();

        const qs = selectedProviderCode !== "all" ? `?provider_code=${encodeURIComponent(selectedProviderCode)}` : "";

        const run = async () => {
            // Models will be fetched based on selected instance type (see useEffect below)

            const combos: Combo[] = [];
            for (const z of candidateZones) {
                const cached = typesByZoneRef.get(z.id);
                let types = cached;
                if (!types) {
                    const res = await fetch(apiUrl(`zones/${z.id}/instance_types${qs}`));
                    types = res.ok ? ((await res.json()) as InstanceType[]) : [];
                    typesByZoneRef.set(z.id, types);
                }
                for (const t of types) {
                    if (!t.is_active) continue;
                    if (selectedProviderId && t.provider_id !== selectedProviderId) continue;
                    const p = providers.find((pp) => pp.id === t.provider_id) ?? null;
                    const r = regions.find((rr) => rr.id === z.region_id) ?? null;
                    combos.push({ key: `${z.id}:${t.id}`, provider: p, region: r, zone: z, type: t });
                }
            }
            combos.sort((a, b) => {
                const ag = a.type.gpu_count ?? 0;
                const bg = b.type.gpu_count ?? 0;
                if (bg !== ag) return bg - ag;
                const av = a.type.vram_per_gpu_gb ?? 0;
                const bv = b.type.vram_per_gpu_gb ?? 0;
                if (bv !== av) return bv - av;
                const n = (a.type.name || "").localeCompare(b.type.name || "");
                if (n !== 0) return n;
                return (a.zone.code || "").localeCompare(b.zone.code || "");
            });

            if (cancelled) return;
            setBaseCombos(combos);
            if (selectedComboKey && !combos.some((c) => c.key === selectedComboKey)) setSelectedComboKey("");
        };

        void run().catch((e) => console.error("Failed to build combos", e));
        return () => {
            cancelled = true;
        };
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [open, selectedProviderCode, selectedProviderId, selectedRegionId, selectedZoneId, zones, providers, regions, typesByZoneRef]);

    // Fetch compatible models when an instance type is selected
    useEffect(() => {
        if (!open) return;
        let cancelled = false;

        const fetchModels = async () => {
            if (selectedCombo?.type?.id) {
                // Fetch only models compatible with the selected instance type
                const mres = await fetch(apiUrl(`instance_types/${selectedCombo.type.id}/models`));
                const mdata = mres.ok ? ((await mres.json()) as LlmModel[]) : [];
                if (!cancelled) {
                    setModels(mdata);
                    // Reset selected model if it's not in the compatible list
                    if (selectedModelId && !mdata.some((m) => m.id === selectedModelId)) {
                        setSelectedModelId(mdata.length > 0 ? mdata[0].id : "");
                    } else if (!selectedModelId && mdata.length > 0) {
                        setSelectedModelId(mdata[0].id);
                    }
                }
            } else {
                // No instance type selected: fetch all active models
                const mres = await fetch(apiUrl("models?active=true"));
                const mdata = mres.ok ? ((await mres.json()) as LlmModel[]) : [];
                if (!cancelled) {
                    setModels(mdata);
                    if (!selectedModelId && mdata.length > 0) setSelectedModelId(mdata[0].id);
                }
            }
        };

        void fetchModels().catch((e) => console.error("Failed to fetch models", e));
        return () => {
            cancelled = true;
        };
    }, [open, selectedCombo?.type?.id, selectedModelId]);

    const handleDeploy = async () => {
        setErrorMsg(null);
        if (!selectedCombo) {
            setErrorMsg("Merci de sélectionner un Provider/Region/Zone + un type d’instance.");
            return;
        }
        if (!selectedModelId) {
            setErrorMsg("Merci de sélectionner un modèle.");
            return;
        }

        setDeployStep("submitting");
        try {
            const zoneForDeploy = selectedCombo.zone;
            const typeForDeploy = selectedCombo.type;
            const providerCodeForDeploy = selectedCombo.provider?.code ?? undefined;

            const res = await fetch(apiUrl("deployments"), {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    provider_code: providerCodeForDeploy,
                    zone: zoneForDeploy?.code || "",
                    instance_type: typeForDeploy?.code || "",
                    model_id: selectedModelId,
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
                setDeployStep("form");
                setErrorMsg(String(msg));
            }
        } catch (e) {
            console.error(e);
            setDeployStep("form");
            setErrorMsg("Erreur lors de la création de l’instance.");
        }
    };

    const handleClose = () => {
        setDeployStep("form");
        setErrorMsg(null);
        setSelectedProviderCode("all");
        setSelectedRegionId("all");
        setSelectedZoneId("all");
        setSelectedComboKey("");
        setSelectedModelId("");
        onClose();
    };

    return (
        <Dialog open={open} onOpenChange={handleClose}>
            <DialogContent showCloseButton={false} className="w-[calc(100vw-2rem)] max-w-5xl sm:max-w-5xl">
                <DialogHeader>
                    <DialogTitle>Create New Instance</DialogTitle>
                    <DialogDescription>
                        Configure your GPU instance parameters.
                    </DialogDescription>
                </DialogHeader>

                {deployStep === "success" ? (
                    <IARequestAccepted title="Demande de création prise en compte" tone="success" />
                ) : (
                    <div className="grid gap-6 py-4">
                        {errorMsg ? (
                            <IAAlert variant="destructive">
                                <IAAlertTitle>Impossible de créer l’instance</IAAlertTitle>
                                <IAAlertDescription>{errorMsg}</IAAlertDescription>
                            </IAAlert>
                        ) : null}
                        {/* Filters + Types cards */}
                        <div className="grid gap-3">
                            <div className="flex flex-col gap-2">
                                <div className="relative">
                                    <Search className="h-4 w-4 text-muted-foreground absolute left-3 top-1/2 -translate-y-1/2" />
                                    <Input
                                        className="pl-9"
                                        placeholder="Rechercher (ex: H100, L40S, RENDER...)"
                                        value={typeQuery}
                                        onChange={(e: ChangeEvent<HTMLInputElement>) => setTypeQuery(e.target.value)}
                                        disabled={deployStep === "submitting"}
                                    />
                                </div>

                                {/* Provider/Region/Zone aligned next to GPU/VRAM, under the SearchBox */}
                                <div className="flex flex-wrap items-center gap-2">
                                    <Select value={selectedProviderCode} onValueChange={onProviderChange}>
                                        <SelectTrigger className="w-[190px]">
                                            <SelectValue placeholder="Provider : tous" />
                                        </SelectTrigger>
                                        <SelectContent>
                                            <SelectItem value="all">Provider : tous</SelectItem>
                                            {providers.filter((p) => p.is_active ?? true).map((p) => (
                                                <SelectItem key={p.id} value={p.code}>
                                                    {p.name}
                                                </SelectItem>
                                            ))}
                                        </SelectContent>
                                    </Select>

                                    <Select
                                        value={selectedRegionId}
                                        onValueChange={onRegionChange}
                                        disabled={deployStep === "submitting"}
                                    >
                                        <SelectTrigger className="w-[190px]">
                                            <SelectValue placeholder="Region : toutes" />
                                        </SelectTrigger>
                                        <SelectContent>
                                            <SelectItem value="all">Region : toutes</SelectItem>
                                            {regions
                                                .filter((r) => r.is_active && (!selectedProviderId || r.provider_id === selectedProviderId))
                                                .map((r) => (
                                                <SelectItem key={r.id} value={r.id}>
                                                    {r.name}
                                                </SelectItem>
                                            ))}
                                        </SelectContent>
                                    </Select>

                                    <Select
                                        value={selectedZoneId}
                                        onValueChange={onZoneChange}
                                        disabled={deployStep === "submitting"}
                                    >
                                        <SelectTrigger className="w-[190px]">
                                            <SelectValue placeholder="Zone : toutes" />
                                        </SelectTrigger>
                                        <SelectContent>
                                            <SelectItem value="all">Zone : toutes</SelectItem>
                                            {zones.map((z) => (
                                                <SelectItem key={z.id} value={z.id}>
                                                    {z.name}
                                                </SelectItem>
                                            ))}
                                        </SelectContent>
                                    </Select>

                                    <Select value={filterGpuCount} onValueChange={setFilterGpuCount}>
                                        <SelectTrigger className="w-[140px]">
                                            <SelectValue placeholder="GPU : tout" />
                                        </SelectTrigger>
                                        <SelectContent>
                                            <SelectItem value="all">GPU : tout</SelectItem>
                                            {gpuCountOptions.map((g) => (
                                                <SelectItem key={g} value={String(g)}>{g} GPU</SelectItem>
                                            ))}
                                        </SelectContent>
                                    </Select>

                                    <Select value={filterVramPerGpu} onValueChange={setFilterVramPerGpu}>
                                        <SelectTrigger className="w-[170px]">
                                            <SelectValue placeholder="VRAM : toutes" />
                                        </SelectTrigger>
                                        <SelectContent>
                                            <SelectItem value="all">VRAM : toutes</SelectItem>
                                            {vramOptions.map((v) => (
                                                <SelectItem key={v} value={String(v)}>{`≥ ${v} GB/GPU`}</SelectItem>
                                            ))}
                                        </SelectContent>
                                    </Select>
                                </div>
                            </div>

                            <ScrollArea className="h-[420px] rounded-md border [&>[data-radix-scroll-area-scrollbar][data-orientation=vertical]]:bg-muted/30">
                                <div className="p-2 space-y-2">
                                    {filteredCombos.length === 0 ? (
                                        <div className="text-sm text-muted-foreground p-4">Aucun type ne correspond aux filtres.</div>
                                    ) : (
                                        filteredCombos.map((c) => {
                                            const t = c.type;
                                            const gpus = t.gpu_count ?? 0;
                                            const vram = t.vram_per_gpu_gb ?? 0;
                                            const perHour = typeof t.cost_per_hour === "number" ? t.cost_per_hour : null;
                                            const isSelected = c.key === selectedComboKey;
                                            const gpuIconsCount = gpus > 0 ? Math.min(gpus, 8) : 0;
                                            return (
                                                <button
                                                    key={c.key}
                                                    type="button"
                                                    onClick={() => setSelectedComboKey(c.key)}
                                                    className={`w-full text-left rounded-md border p-3 hover:bg-muted/30 transition-colors ${
                                                        isSelected ? "border-primary bg-muted/20 ring-2 ring-primary/30" : "border-border bg-background"
                                                    }`}
                                                >
                                                    <div className="flex items-start justify-between gap-3">
                                                        <div className="min-w-0 flex-1">
                                                            <div className="flex items-center justify-between gap-3">
                                                                <div className="font-medium truncate">{t.name}</div>
                                                                <div className="flex items-center gap-1 shrink-0">
                                                                    <Badge variant="outline" className="text-[11px] font-mono">
                                                                        {c.provider?.name ?? "-"}
                                                                    </Badge>
                                                                    <Badge variant="outline" className="text-[11px] font-mono">
                                                                        {c.region?.name ?? "-"}
                                                                    </Badge>
                                                                    <Badge variant="outline" className="text-[11px] font-mono">
                                                                        {c.zone?.name ?? "-"}
                                                                    </Badge>
                                                                </div>
                                                            </div>

                                                            <div className="text-xs text-muted-foreground flex flex-wrap items-center gap-x-2 gap-y-1 mt-1">
                                                                <span className="inline-flex items-center gap-0.5">
                                                                    {gpuIconsCount > 0 ? (
                                                                        <>
                                                                            {Array.from({ length: gpuIconsCount }).map((_, i) => (
                                                                                <Microchip key={i} className="h-4 w-4 text-emerald-600" />
                                                                            ))}
                                                                            {gpus > 8 ? (
                                                                                <span className="ml-1 font-mono text-[11px] text-muted-foreground/90">×{gpus}</span>
                                                                            ) : null}
                                                                            <span className="sr-only">{gpus} GPU</span>
                                                                        </>
                                                                    ) : (
                                                                        <span className="text-muted-foreground/70">—</span>
                                                                    )}
                                                                </span>
                                                                <span>
                                                                    {gpus} GPU • {vram}GB VRAM/GPU{gpus && vram ? ` • ${gpus * vram}GB total` : ""}
                                                                </span>
                                                                {t.cpu_count ? <span>• {t.cpu_count} vCPU</span> : null}
                                                                {t.ram_gb ? <span>• {t.ram_gb}GB RAM</span> : null}
                                                            </div>

                                                            <div className="mt-1 text-xs text-muted-foreground font-mono flex flex-wrap items-center gap-x-3 gap-y-1">
                                                                {perHour != null ? (
                                                                    <>
                                                                        <span>
                                                                            <span className="opacity-70">min</span>{" "}
                                                                            <span className="text-foreground">{formatMoney(perHour / 60)}</span>
                                                                        </span>
                                                                        <span>
                                                                            <span className="opacity-70">h</span>{" "}
                                                                            <span className="text-foreground font-semibold">{formatMoney(perHour)}</span>
                                                                        </span>
                                                                        <span>
                                                                            <span className="opacity-70">j</span>{" "}
                                                                            <span className="text-foreground">{formatMoney(perHour * 24)}</span>
                                                                        </span>
                                                                        <span>
                                                                            <span className="opacity-70">mois</span>{" "}
                                                                            <span className="text-foreground">{formatMoney(perHour * 24 * 30)}</span>
                                                                        </span>
                                                                    </>
                                                                ) : (
                                                                    <span className="text-xs text-muted-foreground">Prix indisponible</span>
                                                                )}
                                                            </div>
                                                        </div>

                                                        <div className="shrink-0 text-right">
                                                            {isSelected ? (
                                                                <div className="text-xs font-medium text-primary">Sélectionné</div>
                                                            ) : null}
                                                        </div>
                                                    </div>
                                                </button>
                                            );
                                        })
                                    )}
                                </div>
                            </ScrollArea>
                        </div>

                        {/* Model selector (bottom) */}
                        <div className="grid gap-2">
                            <Label>Model</Label>
                            <Select
                                value={selectedModelId}
                                onValueChange={setSelectedModelId}
                                disabled={deployStep === "submitting" || models.length === 0}
                            >
                                <SelectTrigger className="w-full">
                                    <SelectValue
                                        placeholder={models.length === 0 ? "No models available (create one in Models)" : "Select a model"}
                                    />
                                </SelectTrigger>
                                <SelectContent>
                                    {models.map((m) => (
                                        <SelectItem key={m.id} value={m.id}>
                                            {m.name} — {m.model_id}
                                        </SelectItem>
                                    ))}
                                </SelectContent>
                            </Select>
                            {selectedModel ? (
                                <div className="text-xs text-muted-foreground">
                                    VRAM req: <span className="font-mono text-foreground">{selectedModel.required_vram_gb}GB</span>
                                    {" • "}ctx: <span className="font-mono text-foreground">{selectedModel.context_length}</span>
                                    {selectedModel.data_volume_gb ? (
                                        <>
                                            {" • "}disk:{" "}
                                            <span className="font-mono text-foreground">{selectedModel.data_volume_gb}GB</span>
                                        </>
                                    ) : null}
                                </div>
                            ) : null}
                        </div>
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
                                disabled={deployStep === "submitting" || !selectedComboKey || !selectedModelId || models.length === 0}
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
