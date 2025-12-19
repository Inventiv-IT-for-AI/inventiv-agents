"use client";

export const dynamic = "force-dynamic";

import { Suspense, useEffect, useMemo, useState, type ChangeEvent } from "react";
import { useRouter } from "next/navigation";
import { useSearchParams } from "next/navigation";
import { apiUrl } from "@/lib/api";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Pencil, Settings2, Info } from "lucide-react";
import { ManageZonesModal } from "@/components/settings/ManageZonesModal";
import type { Provider, ProviderParams, Region, Zone, InstanceType, LlmModel } from "@/lib/types";
import { type IADataTableColumn, type DataTableSortState, type LoadRangeResult } from "ia-widgets";
import { formatEur } from "@/lib/utils";
import { AIToggle } from "ia-widgets";
import { ProvidersTab, RegionsTab, ZonesTab, InstanceTypesTab, ModelsTab, GlobalParamsTab } from "@/components/settings/tabs";
import { IAAlert, IAAlertDescription, IAAlertTitle } from "ia-designsys";
import { WorkspaceBanner } from "@/components/shared/WorkspaceBanner";

function InfoHint({ text }: { text: string }) {
    const [open, setOpen] = useState(false);
    useEffect(() => {
        if (!open) return;
        const onDoc = (e: MouseEvent | TouchEvent) => {
            const t = e.target as HTMLElement | null;
            if (!t) return;
            if (t.closest("[data-infohint-root='true']")) return;
            setOpen(false);
        };
        document.addEventListener("mousedown", onDoc);
        document.addEventListener("touchstart", onDoc);
        return () => {
            document.removeEventListener("mousedown", onDoc);
            document.removeEventListener("touchstart", onDoc);
        };
    }, [open]);

    return (
        <span data-infohint-root="true" className="relative inline-flex items-center">
            <button
                type="button"
                onClick={() => setOpen((v) => !v)}
                className="ml-2 inline-flex h-5 w-5 items-center justify-center rounded hover:bg-muted focus:outline-none focus:ring-2 focus:ring-sky-500"
                aria-label="Help"
                title={text}
            >
                <Info className="h-4 w-4 text-muted-foreground" />
            </button>
            <span
                className={[
                    "absolute right-0 top-6 z-50 w-[360px] rounded-md border bg-popover p-3 text-xs text-popover-foreground shadow-md",
                    "opacity-0 pointer-events-none transition-opacity",
                    open ? "opacity-100 pointer-events-auto" : "group-hover:opacity-100",
                ].join(" ")}
            >
                {text}
            </span>
        </span>
    );
}
function SettingsPageInner() {
    const router = useRouter();
    const [me, setMe] = useState<{ role: string } | null>(null);
    const isAdmin = me?.role === "admin";
    const [activeTab, setActiveTab] = useState<"providers" | "regions" | "zones" | "types" | "models" | "global_params">("providers");
    const [refreshTick, setRefreshTick] = useState({ providers: 0, regions: 0, zones: 0, types: 0, models: 0, global_params: 0 });
    const [providersSort, setProvidersSort] = useState<DataTableSortState>(null);
    const [regionsSort, setRegionsSort] = useState<DataTableSortState>(null);
    const [zonesSort, setZonesSort] = useState<DataTableSortState>(null);
    const [typesSort, setTypesSort] = useState<DataTableSortState>(null);
    const [modelsSort, setModelsSort] = useState<DataTableSortState>(null);

    type EntityType = "provider" | "region" | "zone" | "type" | "model";
    type RefreshKey = "providers" | "regions" | "zones" | "types" | "models" | "global_params";
    const refreshKeyFor = (t: EntityType): RefreshKey => {
        switch (t) {
            case "provider":
                return "providers";
            case "region":
                return "regions";
            case "zone":
                return "zones";
            case "type":
                return "types";
            case "model":
                return "models";
        }
    };

    const [editingEntity, setEditingEntity] = useState<Provider | Region | Zone | InstanceType | LlmModel | null>(null);
    const [entityType, setEntityType] = useState<EntityType | null>(null);
    const [isEditOpen, setIsEditOpen] = useState(false);
    const [providersList, setProvidersList] = useState<Provider[]>([]);
    const [regionsList, setRegionsList] = useState<Region[]>([]);
    const [providerParamsById, setProviderParamsById] = useState<Record<string, ProviderParams>>({});
    const [settingsDefs, setSettingsDefs] = useState<Record<string, { min?: number; max?: number; defInt?: number; defBool?: boolean; defText?: string; desc?: string }>>({});

    const descFor = useMemo(() => {
        return (key: string, fallback: string) => (settingsDefs[key]?.desc?.trim() ? String(settingsDefs[key]?.desc) : fallback);
    }, [settingsDefs]);

    const [globalSettingsLoading, setGlobalSettingsLoading] = useState(false);
    const [globalStaleSeconds, setGlobalStaleSeconds] = useState<string>("");
    const [saveNotice, setSaveNotice] = useState<{ variant: "default" | "destructive"; title: string; description?: string } | null>(null);

    const searchParams = useSearchParams();
    useEffect(() => {
        // Settings is admin-only
        fetch(apiUrl("/auth/me"))
            .then((r) => (r.ok ? r.json() : Promise.reject()))
            .then((u) => setMe(u))
            .catch(() => {
                setMe(null);
                router.replace("/login");
            });

        const tab = (searchParams.get("tab") || "").toLowerCase();
        const isTab = (
            v: string
        ): v is "providers" | "regions" | "zones" | "types" | "models" | "global_params" =>
            v === "providers" || v === "regions" || v === "zones" || v === "types" || v === "models" || v === "global_params";
        if (isTab(tab)) setActiveTab(tab);
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, []);

    useEffect(() => {
        if (!isAdmin) return;
        // Load settings definitions once (used for placeholders + min/max + defaults).
        fetch(apiUrl("settings/definitions"))
            .then((r) => (r.ok ? r.json() : []))
            .then((rows) => {
                const map: Record<string, { min?: number; max?: number; defInt?: number; defBool?: boolean; defText?: string; desc?: string }> = {};
                for (const r of Array.isArray(rows) ? rows : []) {
                    const row = (r ?? null) as Record<string, unknown> | null;
                    const key = row?.key;
                    if (typeof key !== "string" || !key) continue;
                    map[key] = {
                        min: typeof row.min_int === "number" ? row.min_int : undefined,
                        max: typeof row.max_int === "number" ? row.max_int : undefined,
                        defInt: typeof row.default_int === "number" ? row.default_int : undefined,
                        defBool: typeof row.default_bool === "boolean" ? row.default_bool : undefined,
                        defText: typeof row.default_text === "string" ? row.default_text : undefined,
                        desc: typeof row.description === "string" ? row.description : undefined,
                    };
                }
                setSettingsDefs(map);
            })
            .catch(() => null);
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, []);

    useEffect(() => {
        if (!isAdmin) return;
        if (activeTab !== "providers") return;
        fetch(apiUrl("providers/params"))
            .then((r) => (r.ok ? r.json() : []))
            .then((rows) => {
                const map: Record<string, ProviderParams> = {};
                for (const row of rows as ProviderParams[]) {
                    map[row.provider_id] = row;
                }
                setProviderParamsById(map);
            })
            .catch(() => null);
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [activeTab, refreshTick.providers]);

    useEffect(() => {
        if (!isAdmin) return;
        // Preload providers/regions lists for create forms (small catalogs).
        const load = async () => {
            const [pRes, rRes] = await Promise.all([fetch(apiUrl("providers")), fetch(apiUrl("regions"))]);
            if (pRes.ok) setProvidersList((await pRes.json()) as Provider[]);
            if (rRes.ok) setRegionsList((await rRes.json()) as Region[]);
        };
        void load().catch(() => null);
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, []);

    // Manage Zones Modal State
    const [isManageZonesOpen, setIsManageZonesOpen] = useState(false);
    const [selectedInstanceType, setSelectedInstanceType] = useState<InstanceType | null>(null);

    const [formData, setFormData] = useState({
        code: "",
        name: "",
        description: "",
        is_active: true,
        // Provider params (stored as strings; empty => default)
        worker_instance_startup_timeout_s: "",
        instance_startup_timeout_s: "",
        worker_ssh_bootstrap_timeout_s: "",
        worker_health_port: "",
        worker_vllm_port: "",
        worker_data_volume_gb_default: "",
        worker_expose_ports: "default" as "default" | "true" | "false",
        worker_vllm_mode: "default" as "default" | "mono" | "multi",
        worker_vllm_image: "",
        cost_per_hour: "",
        model_id: "",
        required_vram_gb: "",
        context_length: "",
        data_volume_gb: "",
        provider_id: "",
        region_id: "",
        gpu_count: "",
        vram_per_gpu_gb: "",
        cpu_count: "",
        ram_gb: "",
        bandwidth_bps: "",
    });

    type SearchResponse<T> = {
        offset: number;
        limit: number;
        total_count: number;
        filtered_count: number;
        rows: T[];
    };

    const handleEdit = (entity: Provider | Region | Zone | InstanceType | LlmModel, type: EntityType) => {
        setEditingEntity(entity);
        setEntityType(type);
        const pParams = type === "provider" ? providerParamsById[(entity as Provider).id] : undefined;
        setFormData({
            code: (type === "model" ? "" : (entity as Provider | Region | Zone | InstanceType).code) ?? "",
            name: entity.name || "",
            description: type === 'provider' ? (((entity as Provider).description ?? "") || "") : "",
            is_active: (() => {
                const v = (entity as unknown as { is_active?: unknown }).is_active;
                return typeof v === "boolean" ? v : false;
            })(),
            worker_instance_startup_timeout_s: pParams?.worker_instance_startup_timeout_s != null ? String(pParams.worker_instance_startup_timeout_s) : "",
            instance_startup_timeout_s: pParams?.instance_startup_timeout_s != null ? String(pParams.instance_startup_timeout_s) : "",
            worker_ssh_bootstrap_timeout_s: pParams?.worker_ssh_bootstrap_timeout_s != null ? String(pParams.worker_ssh_bootstrap_timeout_s) : "",
            worker_health_port: pParams?.worker_health_port != null ? String(pParams.worker_health_port) : "",
            worker_vllm_port: pParams?.worker_vllm_port != null ? String(pParams.worker_vllm_port) : "",
            worker_data_volume_gb_default: pParams?.worker_data_volume_gb_default != null ? String(pParams.worker_data_volume_gb_default) : "",
            worker_expose_ports: pParams?.worker_expose_ports == null ? "default" : (pParams.worker_expose_ports ? "true" : "false"),
            worker_vllm_mode:
                pParams?.worker_vllm_mode == null
                    ? "default"
                    : ((pParams.worker_vllm_mode as unknown) as "default" | "mono" | "multi"),
            worker_vllm_image: pParams?.worker_vllm_image != null ? String(pParams.worker_vllm_image) : "",
            cost_per_hour:
                type === 'type' && (entity as InstanceType).cost_per_hour != null
                    ? String((entity as InstanceType).cost_per_hour)
                    : ""
            ,
            model_id: type === "model" ? ((entity as LlmModel).model_id ?? "") : "",
            required_vram_gb: type === "model" ? String((entity as LlmModel).required_vram_gb ?? 0) : "",
            context_length: type === "model" ? String((entity as LlmModel).context_length ?? 0) : "",
            data_volume_gb: type === "model" ? ((entity as LlmModel).data_volume_gb != null ? String((entity as LlmModel).data_volume_gb) : "") : "",
            provider_id: type === "region" ? String((entity as Region).provider_id ?? "") : (type === "type" ? String((entity as InstanceType).provider_id ?? "") : ""),
            region_id: type === "zone" ? String((entity as Zone).region_id ?? "") : "",
            gpu_count: type === "type" ? String((entity as InstanceType).gpu_count ?? 0) : "",
            vram_per_gpu_gb: type === "type" ? String((entity as InstanceType).vram_per_gpu_gb ?? 0) : "",
            cpu_count: type === "type" ? String((entity as InstanceType).cpu_count ?? 0) : "",
            ram_gb: type === "type" ? String((entity as InstanceType).ram_gb ?? 0) : "",
            bandwidth_bps: type === "type" ? String((entity as InstanceType).bandwidth_bps ?? 0) : "",
        });
        setIsEditOpen(true);
    };

    const openCreate = (type: EntityType) => {
        setEditingEntity(null);
        setEntityType(type);
        setFormData({
            code: "",
            name: "",
            description: "",
            is_active: true,
            worker_instance_startup_timeout_s: "",
            instance_startup_timeout_s: "",
            worker_ssh_bootstrap_timeout_s: "",
            worker_health_port: "",
            worker_vllm_port: "",
            worker_data_volume_gb_default: "",
            worker_expose_ports: "default",
            worker_vllm_mode: "default",
            worker_vllm_image: "",
            cost_per_hour: "",
            model_id: "",
            required_vram_gb: "0",
            context_length: "0",
            data_volume_gb: "",
            provider_id: "",
            region_id: "",
            gpu_count: "1",
            vram_per_gpu_gb: "24",
            cpu_count: "8",
            ram_gb: "32",
            bandwidth_bps: "1000000000",
        });
        setIsEditOpen(true);
    };

    const handleSave = async () => {
        setSaveNotice(null);
        if (!entityType) return;

        const isModel = entityType === "model";
        const isCreate = !editingEntity;
        if (isCreate && !isModel && entityType !== "provider" && entityType !== "region" && entityType !== "zone" && entityType !== "type") return;

        const base = entityType === "type" ? "instance_types" : `${entityType}s`;
        const entityId = (editingEntity as unknown as { id?: unknown } | null)?.id;
        const url = isCreate ? apiUrl(base) : apiUrl(`${base}/${String(entityId ?? "")}`);

        const payload: Record<string, unknown> = { is_active: formData.is_active };

        if (entityType === 'provider') {
            payload.code = formData.code.trim();
            payload.name = formData.name.trim();
            payload.description = formData.description;
        }

        if (entityType === 'region') {
            payload.provider_id = formData.provider_id;
            payload.code = formData.code.trim();
            payload.name = formData.name.trim();
        }

        if (entityType === 'zone') {
            payload.region_id = formData.region_id;
            payload.code = formData.code.trim();
            payload.name = formData.name.trim();
        }

        if (entityType === 'type') {
            payload.provider_id = formData.provider_id;
            payload.code = formData.code.trim();
            payload.name = formData.name.trim();
            payload.gpu_count = Number(formData.gpu_count || 0);
            payload.vram_per_gpu_gb = Number(formData.vram_per_gpu_gb || 0);
            payload.cpu_count = Number(formData.cpu_count || 0);
            payload.ram_gb = Number(formData.ram_gb || 0);
            payload.bandwidth_bps = Number(formData.bandwidth_bps || 0);
            payload.cost_per_hour = formData.cost_per_hour ? parseFloat(formData.cost_per_hour) : null;
        }

        if (entityType === "model") {
            payload.name = formData.name.trim();
            payload.model_id = formData.model_id.trim();
            payload.required_vram_gb = Number(formData.required_vram_gb || 0);
            payload.context_length = Number(formData.context_length || 0);
            payload.data_volume_gb = formData.data_volume_gb.trim() ? Number(formData.data_volume_gb.trim()) : null;
        }

        try {
            const res = await fetch(url, {
                method: isCreate ? "POST" : "PUT",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify(payload)
            });

            if (res.ok) {
                // Enrich provider CRUD: persist provider-scoped params in the same flow.
                if (entityType === "provider") {
                    const providerId: string | null = isCreate
                        ? ((await res.json()) as Provider).id
                        : (((editingEntity as unknown as { id?: unknown } | null)?.id as string | undefined) ?? null);
                    if (providerId) {
                        const toNum = (s: string) => (s.trim() === "" ? null : Number(s.trim()));
                        const pPayload: Record<string, unknown> = {
                            worker_instance_startup_timeout_s: toNum(formData.worker_instance_startup_timeout_s),
                            instance_startup_timeout_s: toNum(formData.instance_startup_timeout_s),
                            worker_ssh_bootstrap_timeout_s: toNum(formData.worker_ssh_bootstrap_timeout_s),
                            worker_health_port: toNum(formData.worker_health_port),
                            worker_vllm_port: toNum(formData.worker_vllm_port),
                            worker_data_volume_gb_default: toNum(formData.worker_data_volume_gb_default),
                            worker_expose_ports:
                                formData.worker_expose_ports === "default" ? null : formData.worker_expose_ports === "true",
                            worker_vllm_mode: formData.worker_vllm_mode === "default" ? null : formData.worker_vllm_mode,
                            worker_vllm_image: formData.worker_vllm_image.trim() === "" ? null : formData.worker_vllm_image.trim(),
                        };
                        const pRes = await fetch(apiUrl(`providers/${providerId}/params`), {
                            method: "PUT",
                            headers: { "Content-Type": "application/json" },
                            body: JSON.stringify(pPayload),
                        });
                        if (!pRes.ok) {
                            setSaveNotice({
                                variant: "default",
                                title: "Provider enregistré",
                                description: "Mais la mise à jour des paramètres provider a échoué.",
                            });
                        }
                    }
                }
                setIsEditOpen(false);
                const k = refreshKeyFor(entityType);
                setRefreshTick((s) => ({ ...s, [k]: s[k] + 1 }));
            } else {
                setSaveNotice({ variant: "destructive", title: "Échec de l’enregistrement", description: "Merci de réessayer." });
            }
        } catch (err) {
            console.error("Save failed", err);
            setSaveNotice({ variant: "destructive", title: "Erreur", description: "Erreur lors de l’enregistrement." });
        }
    };

    const toggleActive = async (entityId: string, type: 'provider' | 'region' | 'zone' | 'type', nextActive: boolean) => {
        const url = apiUrl(`${type === 'type' ? 'instance_types' : type + 's'}/${entityId}`);
        try {
            await fetch(url, {
                method: "PUT",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ is_active: nextActive })
            });
            const k = refreshKeyFor(type);
            setRefreshTick((s) => ({ ...s, [k]: s[k] + 1 }));
        } catch (err) {
            console.error("Toggle failed", err);
        }
    };

    const toggleModelActive = async (entityId: string, nextActive: boolean) => {
        const url = apiUrl(`models/${entityId}`);
        try {
            await fetch(url, {
                method: "PUT",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ is_active: nextActive })
            });
            setRefreshTick((s) => ({ ...s, models: s.models + 1 }));
        } catch (err) {
            console.error("Toggle failed", err);
        }
    };

    const openCreateModel = () => {
        openCreate("model");
    };

    // Provider params are edited within Provider CRUD (create/edit modal + provider list columns).

    const providerColumns: IADataTableColumn<Provider>[] = [
        { id: "name", label: "Name", width: 220, sortable: true, cell: ({ row }) => <span className="font-medium">{row.name}</span> },
        { id: "code", label: "Code", width: 160, sortable: true, cell: ({ row }) => <span className="font-mono text-xs">{row.code}</span> },
        {
            id: "p_worker_startup_timeout",
            label: "Worker startup timeout",
            width: 260,
            defaultHidden: true,
            cell: ({ row }) => {
                const v = providerParamsById[row.id]?.worker_instance_startup_timeout_s;
                return v == null ? <span className="text-muted-foreground text-sm">default</span> : <span className="font-mono text-xs">{v}s</span>;
            },
        },
        {
            id: "p_instance_startup_timeout",
            label: "Instance startup timeout",
            width: 220,
            defaultHidden: true,
            cell: ({ row }) => {
                const v = providerParamsById[row.id]?.instance_startup_timeout_s;
                return v == null ? <span className="text-muted-foreground text-sm">default</span> : <span className="font-mono text-xs">{v}s</span>;
            },
        },
        {
            id: "p_ssh_bootstrap_timeout",
            label: "SSH bootstrap timeout",
            width: 240,
            defaultHidden: true,
            cell: ({ row }) => {
                const v = providerParamsById[row.id]?.worker_ssh_bootstrap_timeout_s;
                return v == null ? <span className="text-muted-foreground text-sm">default</span> : <span className="font-mono text-xs">{v}s</span>;
            },
        },
        {
            id: "p_ports",
            label: "Ports",
            width: 150,
            defaultHidden: true,
            cell: ({ row }) => {
                const p = providerParamsById[row.id];
                const hp = p?.worker_health_port ?? null;
                const vp = p?.worker_vllm_port ?? null;
                if (hp == null && vp == null) return <span className="text-muted-foreground text-sm">default</span>;
                return <span className="font-mono text-xs">{vp ?? "?"}/{hp ?? "?"}</span>;
            },
        },
        {
            id: "p_expose_ports",
            label: "Expose ports",
            width: 190,
            defaultHidden: true,
            cell: ({ row }) => {
                const v = providerParamsById[row.id]?.worker_expose_ports;
                return v == null ? <span className="text-muted-foreground text-sm">default</span> : <span className="text-sm">{v ? "true" : "false"}</span>;
            },
        },
        {
            id: "p_vllm_mode",
            label: "vLLM mode",
            width: 170,
            defaultHidden: true,
            cell: ({ row }) => {
                const v = providerParamsById[row.id]?.worker_vllm_mode;
                return v == null ? <span className="text-muted-foreground text-sm">default</span> : <span className="font-mono text-xs">{v}</span>;
            },
        },
        {
            id: "p_vllm_image",
            label: "vLLM image",
            width: 260,
            defaultHidden: true,
            cellClassName: "truncate",
            cell: ({ row }) => {
                const v = providerParamsById[row.id]?.worker_vllm_image;
                return v == null ? <span className="text-muted-foreground text-sm">default</span> : <span className="font-mono text-xs">{v}</span>;
            },
        },
        {
            id: "description",
            label: "Description",
            width: 420,
            cellClassName: "truncate",
            cell: ({ row }) => <span className="text-sm text-muted-foreground">{row.description ?? ""}</span>,
        },
        {
            id: "active",
            label: "Active",
            width: 110,
            sortable: true,
            cell: ({ row }) => (
                <div className="flex items-center justify-center">
                    <AIToggle
                        checked={!!row.is_active}
                        onCheckedChange={(v: boolean) => void toggleActive(row.id, "provider", v)}
                        aria-label={`Toggle provider ${row.name}`}
                    />
                </div>
            ),
        },
        {
            id: "actions",
            label: "Actions",
            width: 140,
            align: "right",
            disableReorder: true,
            cell: ({ row }) => (
                <div className="flex justify-end gap-2" onClick={(e) => e.stopPropagation()}>
                    <Button variant="outline" size="sm" onClick={() => handleEdit(row, "provider")}>
                        <Pencil className="h-4 w-4 mr-2" />
                        Modifier
                    </Button>
                </div>
            ),
        },
    ];

    // (removed) providerParamsColumns (standalone tab removed)

    type RegionRow = Region & { provider_name?: string; provider_code?: string | null };
    type ZoneRow = Zone & {
        provider_name?: string;
        provider_code?: string | null;
        region_name?: string;
        region_code?: string | null;
    };

    const regionColumns: IADataTableColumn<RegionRow>[] = [
        {
            id: "provider",
            label: "Provider",
            width: 180,
            cell: ({ row }) => (
                <div className="min-w-0">
                    <div className="font-medium truncate">{row.provider_name ?? "-"}</div>
                    <div className="text-xs text-muted-foreground font-mono truncate">
                        {row.provider_code ?? row.provider_id ?? ""}
                    </div>
                </div>
            ),
        },
        { id: "name", label: "Name", width: 260, sortable: true, cell: ({ row }) => <span className="font-medium">{row.name}</span> },
        { id: "code", label: "Code", width: 180, sortable: true, cell: ({ row }) => <span className="font-mono text-xs">{row.code}</span> },
        {
            id: "active",
            label: "Active",
            width: 110,
            sortable: true,
            cell: ({ row }) => (
                <div className="flex items-center justify-center">
                    <AIToggle
                        checked={!!row.is_active}
                        onCheckedChange={(v: boolean) => void toggleActive(row.id, "region", v)}
                        aria-label={`Toggle region ${row.name}`}
                    />
                </div>
            ),
        },
        {
            id: "actions",
            label: "Actions",
            width: 140,
            align: "right",
            disableReorder: true,
            cell: ({ row }) => (
                <div className="flex justify-end gap-2" onClick={(e) => e.stopPropagation()}>
                    <Button variant="outline" size="sm" onClick={() => handleEdit(row, "region")}>
                        <Pencil className="h-4 w-4 mr-2" />
                        Modifier
                    </Button>
                </div>
            ),
        },
    ];

    const zoneColumns: IADataTableColumn<ZoneRow>[] = [
        {
            id: "provider",
            label: "Provider",
            width: 180,
            cell: ({ row }) => (
                <div className="min-w-0">
                    <div className="font-medium truncate">{row.provider_name ?? "-"}</div>
                    <div className="text-xs text-muted-foreground font-mono truncate">
                        {row.provider_code ?? row.provider_id ?? ""}
                    </div>
                </div>
            ),
        },
        {
            id: "region",
            label: "Region",
            width: 200,
            cell: ({ row }) => (
                <div className="min-w-0">
                    <div className="font-medium truncate">{row.region_name ?? "-"}</div>
                    <div className="text-xs text-muted-foreground font-mono truncate">{row.region_code ?? ""}</div>
                </div>
            ),
        },
        { id: "name", label: "Name", width: 260, sortable: true, cell: ({ row }) => <span className="font-medium">{row.name}</span> },
        { id: "code", label: "Code", width: 180, sortable: true, cell: ({ row }) => <span className="font-mono text-xs">{row.code}</span> },
        {
            id: "active",
            label: "Active",
            width: 110,
            sortable: true,
            cell: ({ row }) => (
                <div className="flex items-center justify-center">
                    <AIToggle
                        checked={!!row.is_active}
                        onCheckedChange={(v: boolean) => void toggleActive(row.id, "zone", v)}
                        aria-label={`Toggle zone ${row.name}`}
                    />
                </div>
            ),
        },
        {
            id: "actions",
            label: "Actions",
            width: 140,
            align: "right",
            disableReorder: true,
            cell: ({ row }) => (
                <div className="flex justify-end gap-2" onClick={(e) => e.stopPropagation()}>
                    <Button variant="outline" size="sm" onClick={() => handleEdit(row, "zone")}>
                        <Pencil className="h-4 w-4 mr-2" />
                        Modifier
                    </Button>
                </div>
            ),
        },
    ];

    type InstanceTypeRow = InstanceType & { provider_name?: string; provider_code?: string | null };

    const typeColumns: IADataTableColumn<InstanceTypeRow>[] = [
        {
            id: "provider",
            label: "Provider",
            width: 180,
            cell: ({ row }) => (
                <div className="min-w-0">
                    <div className="font-medium truncate">{row.provider_name ?? "-"}</div>
                    <div className="text-xs text-muted-foreground font-mono truncate">
                        {row.provider_code ?? row.provider_id ?? ""}
                    </div>
                </div>
            ),
        },
        { id: "name", label: "Name", width: 260, sortable: true, cell: ({ row }) => <span className="font-medium">{row.name}</span> },
        { id: "code", label: "Code", width: 180, sortable: true, cell: ({ row }) => <span className="font-mono text-xs">{row.code}</span> },
        {
            id: "specs",
            label: "Specs",
            width: 260,
            cell: ({ row }) => (
                <span className="text-xs text-muted-foreground">
                    {row.gpu_count ?? 0}x GPU, {row.vram_per_gpu_gb ?? "-"}GB VRAM
                </span>
            ),
        },
        { id: "cost", label: "Cost/Hr", width: 120, align: "right", sortable: true, cell: ({ row }) => <span>{row.cost_per_hour != null ? `${formatEur(row.cost_per_hour, { minFrac: 4, maxFrac: 4 })}/h` : "-"}</span> },
        {
            id: "active",
            label: "Active",
            width: 110,
            sortable: true,
            cell: ({ row }) => (
                <div className="flex items-center justify-center">
                    <AIToggle
                        checked={!!row.is_active}
                        onCheckedChange={(v: boolean) => void toggleActive(row.id, "type", v)}
                        aria-label={`Toggle instance type ${row.name}`}
                    />
                </div>
            ),
        },
        {
            id: "actions",
            label: "Actions",
            width: 340,
            align: "right",
            disableReorder: true,
            cell: ({ row }) => (
                <div className="flex justify-end gap-2" onClick={(e) => e.stopPropagation()}>
                    <Button
                        variant="outline"
                        size="sm"
                        onClick={() => {
                            setSelectedInstanceType(row as unknown as InstanceType);
                            setIsManageZonesOpen(true);
                        }}
                        title="Manage Zones"
                    >
                        <Settings2 className="h-4 w-4 mr-1" />
                        Manage Zones
                    </Button>
                    <Button variant="outline" size="sm" onClick={() => handleEdit(row, "type")}>
                        <Pencil className="h-4 w-4 mr-2" />
                        Modifier
                    </Button>
                </div>
            ),
        },
    ];

    type ModelRow = LlmModel;
    const [models, setModels] = useState<ModelRow[]>([]);
    const [modelsLoading, setModelsLoading] = useState(false);
    const refreshModels = async () => {
        setModelsLoading(true);
        try {
            const params = new URLSearchParams();
            const by = modelsSort
                ? ({
                    name: "name",
                    model_id: "model_id",
                    required_vram_gb: "required_vram_gb",
                    context_length: "context_length",
                    data_volume_gb: "data_volume_gb",
                    active: "is_active",
                } as Record<string, string>)[modelsSort.columnId]
                : null;
            if (by) {
                params.set("order_by", by);
                params.set("order_dir", modelsSort!.direction);
            }
            const res = await fetch(apiUrl(`models?${params.toString()}`));
            if (res.ok) {
                const data = (await res.json()) as ModelRow[];
                setModels(data);
            }
        } finally {
            setModelsLoading(false);
        }
    };

    useEffect(() => {
        if (activeTab !== "models") return;
        void refreshModels().catch(() => null);
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [activeTab, refreshTick.models, modelsSort]);

    // provider params are loaded in the "providers" tab effect (providerParamsById)

    useEffect(() => {
        if (activeTab !== "global_params") return;
        setGlobalSettingsLoading(true);
        fetch(apiUrl("settings/global"))
            .then((r) => (r.ok ? r.json() : []))
            .then((rows) => {
                const stale = (Array.isArray(rows) ? rows : []).find((x) => {
                    const r = (x ?? null) as Record<string, unknown> | null;
                    return r?.key === "OPENAI_WORKER_STALE_SECONDS";
                }) as { value_int?: number } | undefined;
                if (stale && stale.value_int != null) setGlobalStaleSeconds(String(stale.value_int));
                else setGlobalStaleSeconds("");
            })
            .catch(() => null)
            .finally(() => setGlobalSettingsLoading(false));
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [activeTab, refreshTick.global_params]);

    const modelColumns: IADataTableColumn<ModelRow>[] = [
        { id: "name", label: "Name", width: 260, sortable: true, cell: ({ row }) => <span className="font-medium">{row.name}</span> },
        { id: "model_id", label: "HF Model ID", width: 360, sortable: true, cell: ({ row }) => <span className="font-mono text-xs">{row.model_id}</span> },
        { id: "required_vram_gb", label: "VRAM (GB)", width: 120, align: "right", sortable: true, cell: ({ row }) => <span className="tabular-nums">{row.required_vram_gb}</span> },
        { id: "context_length", label: "Ctx", width: 120, align: "right", sortable: true, cell: ({ row }) => <span className="tabular-nums">{row.context_length}</span> },
        { id: "data_volume_gb", label: "Disk (GB)", width: 140, align: "right", sortable: true, cell: ({ row }) => <span className="tabular-nums">{row.data_volume_gb ?? "-"}</span> },
        {
            id: "active",
            label: "Active",
            width: 110,
            sortable: true,
            cell: ({ row }) => (
                <div className="flex items-center justify-center">
                    <AIToggle
                        checked={!!row.is_active}
                        onCheckedChange={(v: boolean) => void toggleModelActive(row.id, v)}
                        aria-label={`Toggle model ${row.name}`}
                    />
                </div>
            ),
        },
        {
            id: "actions",
            label: "Actions",
            width: 140,
            align: "right",
            disableReorder: true,
            cell: ({ row }) => (
                <div className="flex justify-end gap-2" onClick={(e) => e.stopPropagation()}>
                    <Button variant="outline" size="sm" onClick={() => handleEdit(row, "model")}>
                        <Pencil className="h-4 w-4 mr-2" />
                        Modifier
                    </Button>
                </div>
            ),
        },
    ];

    const loadProviders = async (offset: number, limit: number): Promise<LoadRangeResult<Provider>> => {
        const params = new URLSearchParams();
        params.set("offset", String(offset));
        params.set("limit", String(limit));
        const by = providersSort ? ({ name: "name", code: "code", active: "is_active" } as Record<string, string>)[providersSort.columnId] : null;
        if (by) {
            params.set("order_by", by);
            params.set("order_dir", providersSort!.direction);
        }
        const res = await fetch(apiUrl(`providers/search?${params.toString()}`));
        const data: SearchResponse<Provider> = await res.json();
        return { offset: data.offset, items: data.rows, totalCount: data.total_count, filteredCount: data.filtered_count };
    };

    const loadRegions = async (offset: number, limit: number): Promise<LoadRangeResult<RegionRow>> => {
        const params = new URLSearchParams();
        params.set("offset", String(offset));
        params.set("limit", String(limit));
        const by = regionsSort ? ({ name: "name", code: "code", active: "is_active" } as Record<string, string>)[regionsSort.columnId] : null;
        if (by) {
            params.set("order_by", by);
            params.set("order_dir", regionsSort!.direction);
        }
        const res = await fetch(apiUrl(`regions/search?${params.toString()}`));
        const data: SearchResponse<RegionRow> = await res.json();
        return { offset: data.offset, items: data.rows, totalCount: data.total_count, filteredCount: data.filtered_count };
    };

    const loadZones = async (offset: number, limit: number): Promise<LoadRangeResult<ZoneRow>> => {
        const params = new URLSearchParams();
        params.set("offset", String(offset));
        params.set("limit", String(limit));
        const by = zonesSort ? ({ name: "name", code: "code", active: "is_active" } as Record<string, string>)[zonesSort.columnId] : null;
        if (by) {
            params.set("order_by", by);
            params.set("order_dir", zonesSort!.direction);
        }
        const res = await fetch(apiUrl(`zones/search?${params.toString()}`));
        const data: SearchResponse<ZoneRow> = await res.json();
        return { offset: data.offset, items: data.rows, totalCount: data.total_count, filteredCount: data.filtered_count };
    };

    const loadTypes = async (offset: number, limit: number): Promise<LoadRangeResult<InstanceType>> => {
        const params = new URLSearchParams();
        params.set("offset", String(offset));
        params.set("limit", String(limit));
        const by = typesSort
            ? ({ name: "name", code: "code", cost: "cost_per_hour", active: "is_active" } as Record<string, string>)[typesSort.columnId]
            : null;
        if (by) {
            params.set("order_by", by);
            params.set("order_dir", typesSort!.direction);
        }
        const res = await fetch(apiUrl(`instance_types/search?${params.toString()}`));
        const data: SearchResponse<InstanceTypeRow> = await res.json();
        return { offset: data.offset, items: data.rows, totalCount: data.total_count, filteredCount: data.filtered_count };
    };

    // API Keys moved to /api-keys

    if (me && !isAdmin) {
        return (
            <div className="p-6">
                <div className="text-2xl font-semibold">Settings</div>
                <div className="mt-2 text-sm text-muted-foreground">
                    Access denied. Settings are available to admin users only.
                </div>
            </div>
        );
    }

    return (
        <div className="p-8 space-y-8">
            <div>
                <h1 className="text-3xl font-bold tracking-tight">Settings</h1>
                <p className="text-muted-foreground">Manage catalog and configuration.</p>
            </div>

            <WorkspaceBanner />

            {saveNotice ? (
                <IAAlert variant={saveNotice.variant}>
                    <IAAlertTitle>{saveNotice.title}</IAAlertTitle>
                    {saveNotice.description ? <IAAlertDescription>{saveNotice.description}</IAAlertDescription> : null}
                </IAAlert>
            ) : null}

                <Tabs value={activeTab} onValueChange={(v: string) => setActiveTab(v as typeof activeTab)} className="w-full">
                <TabsList>
                    <TabsTrigger value="providers">Providers</TabsTrigger>
                    <TabsTrigger value="regions">Regions</TabsTrigger>
                    <TabsTrigger value="zones">Zones</TabsTrigger>
                    <TabsTrigger value="types">Instance Types</TabsTrigger>
                    <TabsTrigger value="models">Models</TabsTrigger>
                    <TabsTrigger value="global_params">Global Params</TabsTrigger>
                </TabsList>

                {/* PROVIDERS */}
                <TabsContent value="providers">
                    <ProvidersTab<Provider>
                        refreshTick={refreshTick.providers}
                        sort={providersSort}
                        setSort={setProvidersSort}
                        columns={providerColumns}
                        loadRange={loadProviders}
                        onCreate={() => openCreate("provider")}
                    />
                </TabsContent>

                {/* REGIONS */}
                <TabsContent value="regions">
                    <RegionsTab<RegionRow>
                        refreshTick={refreshTick.regions}
                        sort={regionsSort}
                        setSort={setRegionsSort}
                        columns={regionColumns}
                        loadRange={loadRegions}
                        onCreate={() => openCreate("region")}
                    />
                </TabsContent>

                {/* ZONES */}
                <TabsContent value="zones">
                    <ZonesTab<ZoneRow>
                        refreshTick={refreshTick.zones}
                        sort={zonesSort}
                        setSort={setZonesSort}
                        columns={zoneColumns}
                        loadRange={loadZones}
                        onCreate={() => openCreate("zone")}
                    />
                </TabsContent>

                {/* INSTANCE TYPES */}
                <TabsContent value="types">
                    <InstanceTypesTab<InstanceTypeRow>
                        refreshTick={refreshTick.types}
                        sort={typesSort}
                        setSort={setTypesSort}
                        columns={typeColumns}
                        loadRange={loadTypes}
                        onCreate={() => openCreate("type")}
                    />
                </TabsContent>

                {/* MODELS */}
                <TabsContent value="models">
                    <ModelsTab<ModelRow>
                        refreshTick={refreshTick.models}
                        sort={modelsSort}
                        setSort={setModelsSort}
                        columns={modelColumns}
                        rows={models}
                        loading={modelsLoading}
                        onCreate={openCreateModel}
                    />
                </TabsContent>

                {/* GLOBAL PARAMS */}
                <TabsContent value="global_params">
                    <GlobalParamsTab
                        globalStaleSeconds={globalStaleSeconds}
                        setGlobalStaleSeconds={setGlobalStaleSeconds}
                        globalSettingsLoading={globalSettingsLoading}
                        settingsDefs={settingsDefs}
                        onSaved={() => setRefreshTick((t) => ({ ...t, global_params: t.global_params + 1 }))}
                    />
                </TabsContent>

                {/* API Keys moved to /api-keys */}
            </Tabs>

            {/* (removed) standalone Provider Params dialog (params are edited in Provider CRUD) */}

            {/* API Keys moved to /api-keys */}

            <Dialog
                open={isEditOpen}
                onOpenChange={(open: boolean) => {
                    setIsEditOpen(open);
                    if (!open) {
                        setEditingEntity(null);
                        setEntityType(null);
                    }
                }}
            >
                <DialogContent
                    showCloseButton={false}
                    className="sm:max-w-[900px] max-h-[85vh] overflow-y-auto"
                >
                    <DialogHeader>
                        <DialogTitle>
                            {editingEntity ? "Modifier" : "Ajouter"}{" "}
                            {entityType === 'model'
                                ? 'le modèle'
                                : entityType === 'type'
                                    ? 'le type d’instance'
                                    : entityType === 'region'
                                        ? 'la région'
                                        : entityType === 'zone'
                                            ? 'la zone'
                                            : 'le provider'}
                        </DialogTitle>
                    </DialogHeader>
                    <div className="grid gap-4 py-4">
                        <div className="grid grid-cols-4 items-center gap-4">
                            <Label htmlFor="name" className="text-right">Nom</Label>
                            <Input id="name" value={formData.name} onChange={(e: ChangeEvent<HTMLInputElement>) => setFormData({ ...formData, name: e.target.value })} className="col-span-3" />
                        </div>
                        {entityType !== "model" && (
                            <div className="grid grid-cols-4 items-center gap-4">
                                <Label htmlFor="code" className="text-right">Code</Label>
                                <Input id="code" value={formData.code} onChange={(e: ChangeEvent<HTMLInputElement>) => setFormData({ ...formData, code: e.target.value })} className="col-span-3" />
                            </div>
                        )}
                        {entityType === "region" && (
                            <div className="grid grid-cols-4 items-center gap-4">
                                <Label className="text-right">Provider</Label>
                                <div className="col-span-3">
                                    <Select value={formData.provider_id} onValueChange={(v: string) => setFormData({ ...formData, provider_id: v })}>
                                        <SelectTrigger>
                                            <SelectValue placeholder="Select provider" />
                                        </SelectTrigger>
                                        <SelectContent>
                                            {providersList.map((p) => (
                                                <SelectItem key={p.id} value={p.id}>
                                                    {p.name} ({p.code})
                                                </SelectItem>
                                            ))}
                                        </SelectContent>
                                    </Select>
                                </div>
                            </div>
                        )}
                        {entityType === "zone" && (
                            <div className="grid grid-cols-4 items-center gap-4">
                                <Label className="text-right">Region</Label>
                                <div className="col-span-3">
                                    <Select value={formData.region_id} onValueChange={(v: string) => setFormData({ ...formData, region_id: v })}>
                                        <SelectTrigger>
                                            <SelectValue placeholder="Select region" />
                                        </SelectTrigger>
                                        <SelectContent>
                                            {regionsList.map((r) => (
                                                <SelectItem key={r.id} value={r.id}>
                                                    {r.name} ({r.code})
                                                </SelectItem>
                                            ))}
                                        </SelectContent>
                                    </Select>
                                </div>
                            </div>
                        )}
                        {entityType === 'provider' && (
                            <>
                                <div className="grid grid-cols-4 items-center gap-4">
                                    <Label htmlFor="description" className="text-right">Description</Label>
                                    <Input
                                        id="description"
                                        value={formData.description}
                                        onChange={(e: ChangeEvent<HTMLInputElement>) => setFormData({ ...formData, description: e.target.value })}
                                        className="col-span-3"
                                    />
                                </div>

                                <div className="pt-2 text-sm font-medium">Provider Params (optional)</div>
                                <div className="text-xs text-muted-foreground pb-2">
                                    Leave empty / “default” to use env → built-in defaults. Values are validated in DB.
                                </div>

                                <div className="grid grid-cols-6 items-start gap-4">
                                    <Label className="col-span-3 text-right leading-tight">
                                        <span className="group inline-flex items-center justify-end">
                                            Worker startup timeout
                                            <InfoHint text={descFor("WORKER_INSTANCE_STARTUP_TIMEOUT_S", "BOOTING→STARTUP_FAILED timeout for worker instances (includes image pulls + model download/load).")} />
                                        </span>
                                    </Label>
                                    <Input
                                        value={formData.worker_instance_startup_timeout_s}
                                        onChange={(e: ChangeEvent<HTMLInputElement>) => setFormData({ ...formData, worker_instance_startup_timeout_s: e.target.value })}
                                        placeholder={settingsDefs["WORKER_INSTANCE_STARTUP_TIMEOUT_S"]?.defInt != null ? `default (${settingsDefs["WORKER_INSTANCE_STARTUP_TIMEOUT_S"]?.defInt}s)` : "default"}
                                        className="col-span-3"
                                    />
                                </div>

                                <div className="grid grid-cols-6 items-start gap-4">
                                    <Label className="col-span-3 text-right leading-tight">
                                        <span className="group inline-flex items-center justify-end">
                                            Instance startup timeout
                                            <InfoHint text={descFor("INSTANCE_STARTUP_TIMEOUT_S", "BOOTING→STARTUP_FAILED timeout for non-worker instances.")} />
                                        </span>
                                    </Label>
                                    <Input
                                        value={formData.instance_startup_timeout_s}
                                        onChange={(e: ChangeEvent<HTMLInputElement>) => setFormData({ ...formData, instance_startup_timeout_s: e.target.value })}
                                        placeholder={settingsDefs["INSTANCE_STARTUP_TIMEOUT_S"]?.defInt != null ? `default (${settingsDefs["INSTANCE_STARTUP_TIMEOUT_S"]?.defInt}s)` : "default"}
                                        className="col-span-3"
                                    />
                                </div>

                                <div className="grid grid-cols-6 items-start gap-4">
                                    <Label className="col-span-3 text-right leading-tight">
                                        <span className="group inline-flex items-center justify-end">
                                            SSH bootstrap timeout
                                            <InfoHint text={descFor("WORKER_SSH_BOOTSTRAP_TIMEOUT_S", "SSH bootstrap timeout for worker auto-install.")} />
                                        </span>
                                    </Label>
                                    <Input
                                        value={formData.worker_ssh_bootstrap_timeout_s}
                                        onChange={(e: ChangeEvent<HTMLInputElement>) => setFormData({ ...formData, worker_ssh_bootstrap_timeout_s: e.target.value })}
                                        placeholder={settingsDefs["WORKER_SSH_BOOTSTRAP_TIMEOUT_S"]?.defInt != null ? `default (${settingsDefs["WORKER_SSH_BOOTSTRAP_TIMEOUT_S"]?.defInt}s)` : "default"}
                                        className="col-span-3"
                                    />
                                </div>

                                <div className="grid grid-cols-6 items-start gap-4">
                                    <Label className="col-span-3 text-right leading-tight">
                                        <span className="group inline-flex items-center justify-end">
                                            Health port
                                            <InfoHint text={descFor("WORKER_HEALTH_PORT", "Worker health server port (agent /readyz).")} />
                                        </span>
                                    </Label>
                                    <Input
                                        value={formData.worker_health_port}
                                        onChange={(e: ChangeEvent<HTMLInputElement>) => setFormData({ ...formData, worker_health_port: e.target.value })}
                                        placeholder={settingsDefs["WORKER_HEALTH_PORT"]?.defInt != null ? `default (${settingsDefs["WORKER_HEALTH_PORT"]?.defInt})` : "default"}
                                        className="col-span-3"
                                    />
                                </div>

                                <div className="grid grid-cols-6 items-start gap-4">
                                    <Label className="col-span-3 text-right leading-tight">
                                        <span className="group inline-flex items-center justify-end">
                                            vLLM port
                                            <InfoHint text={descFor("WORKER_VLLM_PORT", "vLLM OpenAI-compatible port on the worker.")} />
                                        </span>
                                    </Label>
                                    <Input
                                        value={formData.worker_vllm_port}
                                        onChange={(e: ChangeEvent<HTMLInputElement>) => setFormData({ ...formData, worker_vllm_port: e.target.value })}
                                        placeholder={settingsDefs["WORKER_VLLM_PORT"]?.defInt != null ? `default (${settingsDefs["WORKER_VLLM_PORT"]?.defInt})` : "default"}
                                        className="col-span-3"
                                    />
                                </div>

                                <div className="grid grid-cols-6 items-start gap-4">
                                    <Label className="col-span-3 text-right leading-tight">
                                        <span className="group inline-flex items-center justify-end">
                                            Default data volume (GB)
                                            <InfoHint text={descFor("WORKER_DATA_VOLUME_GB_DEFAULT", "Fallback data volume size when model has no explicit recommendation.")} />
                                        </span>
                                    </Label>
                                    <Input
                                        value={formData.worker_data_volume_gb_default}
                                        onChange={(e: ChangeEvent<HTMLInputElement>) => setFormData({ ...formData, worker_data_volume_gb_default: e.target.value })}
                                        placeholder={settingsDefs["WORKER_DATA_VOLUME_GB_DEFAULT"]?.defInt != null ? `default (${settingsDefs["WORKER_DATA_VOLUME_GB_DEFAULT"]?.defInt}GB)` : "default"}
                                        className="col-span-3"
                                    />
                                </div>

                                <div className="grid grid-cols-6 items-start gap-4">
                                    <Label className="col-span-3 text-right leading-tight">
                                        <span className="group inline-flex items-center justify-end">
                                            Expose ports
                                            <InfoHint text={descFor("WORKER_EXPOSE_PORTS", "Provider security group opens inbound worker ports (dev convenience).")} />
                                        </span>
                                    </Label>
                                    <div className="col-span-3 flex items-center gap-3">
                                        <Button
                                            type="button"
                                            variant="outline"
                                            size="sm"
                                            onClick={() => setFormData({ ...formData, worker_expose_ports: "default" })}
                                            disabled={formData.worker_expose_ports === "default"}
                                        >
                                            Default
                                        </Button>
                                        <AIToggle
                                            checked={
                                                formData.worker_expose_ports === "default"
                                                    ? (settingsDefs["WORKER_EXPOSE_PORTS"]?.defBool ?? true)
                                                    : formData.worker_expose_ports === "true"
                                            }
                                            onCheckedChange={(c) => setFormData({ ...formData, worker_expose_ports: c ? "true" : "false" })}
                                            aria-label="Toggle expose ports"
                                        />
                                        <span className="text-xs text-muted-foreground">
                                            {formData.worker_expose_ports === "default" ? "using default" : "override"}
                                        </span>
                                    </div>
                                </div>

                                <div className="grid grid-cols-6 items-start gap-4">
                                    <Label className="col-span-3 text-right leading-tight">
                                        <span className="group inline-flex items-center justify-end">
                                            vLLM mode
                                            <InfoHint text={descFor("WORKER_VLLM_MODE", "vLLM mode: mono|multi (multi = 1 vLLM per GPU behind HAProxy).")} />
                                        </span>
                                    </Label>
                                    <div className="col-span-3">
                                        <Select
                                            value={formData.worker_vllm_mode}
                                            onValueChange={(v: string) =>
                                                setFormData({ ...formData, worker_vllm_mode: v as "default" | "mono" | "multi" })
                                            }
                                        >
                                            <SelectTrigger>
                                                <SelectValue placeholder="default" />
                                            </SelectTrigger>
                                            <SelectContent>
                                                <SelectItem value="default">default</SelectItem>
                                                <SelectItem value="mono">mono</SelectItem>
                                                <SelectItem value="multi">multi</SelectItem>
                                            </SelectContent>
                                        </Select>
                                    </div>
                                </div>

                                <div className="grid grid-cols-6 items-start gap-4">
                                    <Label className="col-span-3 text-right leading-tight">
                                        <span className="group inline-flex items-center justify-end">
                                            vLLM image
                                            <InfoHint text={descFor("WORKER_VLLM_IMAGE", "Docker image for vLLM OpenAI server.")} />
                                        </span>
                                    </Label>
                                    <Input
                                        value={formData.worker_vllm_image}
                                        onChange={(e: ChangeEvent<HTMLInputElement>) => setFormData({ ...formData, worker_vllm_image: e.target.value })}
                                        placeholder={settingsDefs["WORKER_VLLM_IMAGE"]?.defText != null ? `default (${settingsDefs["WORKER_VLLM_IMAGE"]?.defText})` : "default"}
                                        className="col-span-3 font-mono text-xs"
                                    />
                                </div>
                            </>
                        )}
                        {entityType === 'type' && (
                            <>
                                <div className="grid grid-cols-4 items-center gap-4">
                                    <Label className="text-right">Provider</Label>
                                    <div className="col-span-3">
                                        <Select value={formData.provider_id} onValueChange={(v: string) => setFormData({ ...formData, provider_id: v })}>
                                            <SelectTrigger>
                                                <SelectValue placeholder="Select provider" />
                                            </SelectTrigger>
                                            <SelectContent>
                                                {providersList.map((p) => (
                                                    <SelectItem key={p.id} value={p.id}>
                                                        {p.name} ({p.code})
                                                    </SelectItem>
                                                ))}
                                            </SelectContent>
                                        </Select>
                                    </div>
                                </div>
                                <div className="grid grid-cols-4 items-center gap-4">
                                    <Label htmlFor="gpu_count" className="text-right">GPU count</Label>
                                    <Input id="gpu_count" value={formData.gpu_count} onChange={(e: ChangeEvent<HTMLInputElement>) => setFormData({ ...formData, gpu_count: e.target.value })} className="col-span-3" />
                                </div>
                                <div className="grid grid-cols-4 items-center gap-4">
                                    <Label htmlFor="vram_per_gpu_gb" className="text-right">VRAM/GPU</Label>
                                    <Input id="vram_per_gpu_gb" value={formData.vram_per_gpu_gb} onChange={(e: ChangeEvent<HTMLInputElement>) => setFormData({ ...formData, vram_per_gpu_gb: e.target.value })} className="col-span-3" />
                                </div>
                                <div className="grid grid-cols-4 items-center gap-4">
                                    <Label htmlFor="cpu_count" className="text-right">vCPU</Label>
                                    <Input id="cpu_count" value={formData.cpu_count} onChange={(e: ChangeEvent<HTMLInputElement>) => setFormData({ ...formData, cpu_count: e.target.value })} className="col-span-3" />
                                </div>
                                <div className="grid grid-cols-4 items-center gap-4">
                                    <Label htmlFor="ram_gb" className="text-right">RAM (GB)</Label>
                                    <Input id="ram_gb" value={formData.ram_gb} onChange={(e: ChangeEvent<HTMLInputElement>) => setFormData({ ...formData, ram_gb: e.target.value })} className="col-span-3" />
                                </div>
                                <div className="grid grid-cols-4 items-center gap-4">
                                    <Label htmlFor="bandwidth_bps" className="text-right">Bandwidth (bps)</Label>
                                    <Input id="bandwidth_bps" value={formData.bandwidth_bps} onChange={(e: ChangeEvent<HTMLInputElement>) => setFormData({ ...formData, bandwidth_bps: e.target.value })} className="col-span-3" />
                                </div>
                                <div className="grid grid-cols-4 items-center gap-4">
                                    <Label htmlFor="cost" className="text-right">Coût ($/h)</Label>
                                    <Input id="cost" type="number" step="0.0001" value={formData.cost_per_hour} onChange={(e: ChangeEvent<HTMLInputElement>) => setFormData({ ...formData, cost_per_hour: e.target.value })} className="col-span-3" />
                                </div>
                            </>
                        )}
                        {entityType === 'model' && (
                            <>
                                <div className="grid grid-cols-4 items-center gap-4">
                                    <Label htmlFor="model_id" className="text-right">HF model_id</Label>
                                    <Input id="model_id" value={formData.model_id} onChange={(e: ChangeEvent<HTMLInputElement>) => setFormData({ ...formData, model_id: e.target.value })} className="col-span-3" />
                                </div>
                                <div className="grid grid-cols-4 items-center gap-4">
                                    <Label htmlFor="required_vram_gb" className="text-right">VRAM (GB)</Label>
                                    <Input id="required_vram_gb" value={formData.required_vram_gb} onChange={(e: ChangeEvent<HTMLInputElement>) => setFormData({ ...formData, required_vram_gb: e.target.value })} className="col-span-3" />
                                </div>
                                <div className="grid grid-cols-4 items-center gap-4">
                                    <Label htmlFor="context_length" className="text-right">Context</Label>
                                    <Input id="context_length" value={formData.context_length} onChange={(e: ChangeEvent<HTMLInputElement>) => setFormData({ ...formData, context_length: e.target.value })} className="col-span-3" />
                                </div>
                                <div className="grid grid-cols-4 items-center gap-4">
                                    <Label htmlFor="data_volume_gb" className="text-right">Disk GB</Label>
                                    <Input id="data_volume_gb" value={formData.data_volume_gb} onChange={(e: ChangeEvent<HTMLInputElement>) => setFormData({ ...formData, data_volume_gb: e.target.value })} className="col-span-3" />
                                </div>
                            </>
                        )}
                        <div className="grid grid-cols-4 items-center gap-4">
                            <Label htmlFor="active" className="text-right">Actif</Label>
                            <AIToggle checked={formData.is_active} onCheckedChange={(c) => setFormData({ ...formData, is_active: c })} aria-label="Toggle active" />
                        </div>
                    </div>
                    <DialogFooter className="sm:justify-between">
                        <Button
                            variant="outline"
                            onClick={() => {
                                setIsEditOpen(false);
                                setEditingEntity(null);
                                setEntityType(null);
                            }}
                        >
                            Annuler
                        </Button>
                        <Button onClick={handleSave}>Enregistrer</Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>

            {/* Manage Zones Modal */}
            <ManageZonesModal
                open={isManageZonesOpen}
                onClose={() => {
                    setIsManageZonesOpen(false);
                    setSelectedInstanceType(null);
                }}
                instanceType={selectedInstanceType}
            />
        </div>
    );
}

export default function SettingsPage() {
    return (
        <Suspense fallback={<div className="p-8 text-sm text-muted-foreground">Loading…</div>}>
            <SettingsPageInner />
        </Suspense>
    );
}


