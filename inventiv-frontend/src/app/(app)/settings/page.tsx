"use client";

import { useEffect, useState } from "react";
import { useSearchParams } from "next/navigation";
import { apiUrl } from "@/lib/api";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Pencil, Settings2, Plus } from "lucide-react";
import { ManageZonesModal } from "@/components/settings/ManageZonesModal";
import type { Provider, Region, Zone, InstanceType, LlmModel, ApiKey } from "@/lib/types";
import { VirtualizedDataTable, type DataTableColumn } from "@/components/shared/VirtualizedDataTable";
import type { LoadRangeResult } from "@/components/shared/VirtualizedRemoteList";
import { formatEur } from "@/lib/utils";
import { ActiveToggle } from "@/components/shared/ActiveToggle";
import { CopyButton } from "@/components/shared/CopyButton";
export default function SettingsPage() {
    const [activeTab, setActiveTab] = useState<"providers" | "regions" | "zones" | "types" | "models" | "api_keys">("providers");
    const [refreshTick, setRefreshTick] = useState({ providers: 0, regions: 0, zones: 0, types: 0, models: 0, api_keys: 0 });

    type EntityType = "provider" | "region" | "zone" | "type" | "model";
    type RefreshKey = "providers" | "regions" | "zones" | "types" | "models" | "api_keys";
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

    const searchParams = useSearchParams();
    useEffect(() => {
        const tab = (searchParams.get("tab") || "").toLowerCase();
        if (tab === "providers" || tab === "regions" || tab === "zones" || tab === "types" || tab === "models" || tab === "api_keys") {
            setActiveTab(tab as any);
        }
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, []);

    useEffect(() => {
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
        setFormData({
            code: (type === "model" ? "" : (entity as Provider | Region | Zone | InstanceType).code) ?? "",
            name: entity.name || "",
            description: type === 'provider' ? (((entity as Provider).description ?? "") || "") : "",
            is_active: (entity as any).is_active ?? false,
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
        if (!entityType) return;

        const isModel = entityType === "model";
        const isCreate = !editingEntity;
        if (isCreate && !isModel && entityType !== "provider" && entityType !== "region" && entityType !== "zone" && entityType !== "type") return;

        const base = entityType === "type" ? "instance_types" : `${entityType}s`;
        const url = isCreate ? apiUrl(base) : apiUrl(`${base}/${(editingEntity as any).id}`);

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
                setIsEditOpen(false);
                const k = refreshKeyFor(entityType);
                setRefreshTick((s) => ({ ...s, [k]: s[k] + 1 }));
            } else {
                alert("Failed to save");
            }
        } catch (err) {
            console.error("Save failed", err);
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

    const providerColumns: DataTableColumn<Provider>[] = [
        { id: "name", label: "Name", width: 220, cell: ({ row }) => <span className="font-medium">{row.name}</span> },
        { id: "code", label: "Code", width: 160, cell: ({ row }) => <span className="font-mono text-xs">{row.code}</span> },
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
            cell: ({ row }) => (
                <div className="flex items-center justify-center">
                    <ActiveToggle
                        checked={!!row.is_active}
                        onCheckedChange={(v) => void toggleActive(row.id, "provider", v)}
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

    type RegionRow = Region & { provider_name?: string; provider_code?: string | null };
    type ZoneRow = Zone & {
        provider_name?: string;
        provider_code?: string | null;
        region_name?: string;
        region_code?: string | null;
    };

    const regionColumns: DataTableColumn<RegionRow>[] = [
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
        { id: "name", label: "Name", width: 260, cell: ({ row }) => <span className="font-medium">{row.name}</span> },
        { id: "code", label: "Code", width: 180, cell: ({ row }) => <span className="font-mono text-xs">{row.code}</span> },
        {
            id: "active",
            label: "Active",
            width: 110,
            cell: ({ row }) => (
                <div className="flex items-center justify-center">
                    <ActiveToggle
                        checked={!!row.is_active}
                        onCheckedChange={(v) => void toggleActive(row.id, "region", v)}
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

    const zoneColumns: DataTableColumn<ZoneRow>[] = [
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
        { id: "name", label: "Name", width: 260, cell: ({ row }) => <span className="font-medium">{row.name}</span> },
        { id: "code", label: "Code", width: 180, cell: ({ row }) => <span className="font-mono text-xs">{row.code}</span> },
        {
            id: "active",
            label: "Active",
            width: 110,
            cell: ({ row }) => (
                <div className="flex items-center justify-center">
                    <ActiveToggle
                        checked={!!row.is_active}
                        onCheckedChange={(v) => void toggleActive(row.id, "zone", v)}
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

    const typeColumns: DataTableColumn<InstanceTypeRow>[] = [
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
        { id: "name", label: "Name", width: 260, cell: ({ row }) => <span className="font-medium">{row.name}</span> },
        { id: "code", label: "Code", width: 180, cell: ({ row }) => <span className="font-mono text-xs">{row.code}</span> },
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
        { id: "cost", label: "Cost/Hr", width: 120, align: "right", cell: ({ row }) => <span>{row.cost_per_hour != null ? `${formatEur(row.cost_per_hour, { minFrac: 4, maxFrac: 4 })}/h` : "-"}</span> },
        {
            id: "active",
            label: "Active",
            width: 110,
            cell: ({ row }) => (
                <div className="flex items-center justify-center">
                    <ActiveToggle
                        checked={!!row.is_active}
                        onCheckedChange={(v) => void toggleActive(row.id, "type", v)}
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
            const res = await fetch(apiUrl("models"));
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
    }, [activeTab, refreshTick.models]);

    const modelColumns: DataTableColumn<ModelRow>[] = [
        { id: "name", label: "Name", width: 260, cell: ({ row }) => <span className="font-medium">{row.name}</span> },
        { id: "model_id", label: "HF Model ID", width: 360, cell: ({ row }) => <span className="font-mono text-xs">{row.model_id}</span> },
        { id: "required_vram_gb", label: "VRAM (GB)", width: 120, align: "right", cell: ({ row }) => <span className="tabular-nums">{row.required_vram_gb}</span> },
        { id: "context_length", label: "Ctx", width: 120, align: "right", cell: ({ row }) => <span className="tabular-nums">{row.context_length}</span> },
        { id: "data_volume_gb", label: "Disk (GB)", width: 140, align: "right", cell: ({ row }) => <span className="tabular-nums">{row.data_volume_gb ?? "-"}</span> },
        {
            id: "active",
            label: "Active",
            width: 110,
            cell: ({ row }) => (
                <div className="flex items-center justify-center">
                    <ActiveToggle
                        checked={!!row.is_active}
                        onCheckedChange={(v) => void toggleModelActive(row.id, v)}
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
        const res = await fetch(apiUrl(`providers/search?offset=${offset}&limit=${limit}`));
        const data: SearchResponse<Provider> = await res.json();
        return { offset: data.offset, items: data.rows, totalCount: data.total_count, filteredCount: data.filtered_count };
    };

    const loadRegions = async (offset: number, limit: number): Promise<LoadRangeResult<RegionRow>> => {
        const res = await fetch(apiUrl(`regions/search?offset=${offset}&limit=${limit}`));
        const data: SearchResponse<RegionRow> = await res.json();
        return { offset: data.offset, items: data.rows, totalCount: data.total_count, filteredCount: data.filtered_count };
    };

    const loadZones = async (offset: number, limit: number): Promise<LoadRangeResult<ZoneRow>> => {
        const res = await fetch(apiUrl(`zones/search?offset=${offset}&limit=${limit}`));
        const data: SearchResponse<ZoneRow> = await res.json();
        return { offset: data.offset, items: data.rows, totalCount: data.total_count, filteredCount: data.filtered_count };
    };

    const loadTypes = async (offset: number, limit: number): Promise<LoadRangeResult<InstanceType>> => {
        const res = await fetch(apiUrl(`instance_types/search?offset=${offset}&limit=${limit}`));
        const data: SearchResponse<InstanceTypeRow> = await res.json();
        return { offset: data.offset, items: data.rows, totalCount: data.total_count, filteredCount: data.filtered_count };
    };

    // -----------------------------
    // API Keys (per-user, small list)
    // -----------------------------
    const [apiKeys, setApiKeys] = useState<ApiKey[]>([]);
    const [apiKeysLoading, setApiKeysLoading] = useState(false);
    const [apiKeyCreateOpen, setApiKeyCreateOpen] = useState(false);
    const [apiKeyName, setApiKeyName] = useState("");
    const [createdApiKey, setCreatedApiKey] = useState<string | null>(null);
    const [apiKeyEditOpen, setApiKeyEditOpen] = useState(false);
    const [editingApiKey, setEditingApiKey] = useState<ApiKey | null>(null);
    const [apiKeyEditName, setApiKeyEditName] = useState("");

    useEffect(() => {
        if (activeTab !== "api_keys") return;
        setApiKeysLoading(true);
        fetch(apiUrl("api_keys"))
            .then((r) => (r.ok ? r.json() : Promise.reject()))
            .then((rows: ApiKey[]) => setApiKeys(rows))
            .catch(() => null)
            .finally(() => setApiKeysLoading(false));
    }, [activeTab, refreshTick.api_keys]);

    const createApiKey = async () => {
        const name = apiKeyName.trim();
        if (!name) return;
        const res = await fetch(apiUrl("api_keys"), {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ name }),
        });
        if (!res.ok) {
            alert("Failed to create API key");
            return;
        }
        const data = (await res.json()) as { key: ApiKey; api_key: string };
        setCreatedApiKey(data.api_key);
        setApiKeyName("");
        setRefreshTick((s) => ({ ...s, api_keys: s.api_keys + 1 }));
    };

    const revokeApiKey = async (id: string) => {
        if (!confirm("Revoke this API key? It will stop working immediately.")) return;
        await fetch(apiUrl(`api_keys/${id}`), { method: "DELETE" });
        setRefreshTick((s) => ({ ...s, api_keys: s.api_keys + 1 }));
    };

    const openRenameApiKey = (k: ApiKey) => {
        setEditingApiKey(k);
        setApiKeyEditName(k.name);
        setApiKeyEditOpen(true);
    };

    const saveRenameApiKey = async () => {
        if (!editingApiKey) return;
        const name = apiKeyEditName.trim();
        if (!name) return;
        const res = await fetch(apiUrl(`api_keys/${editingApiKey.id}`), {
            method: "PUT",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ name }),
        });
        if (!res.ok) {
            alert("Failed to rename API key");
            return;
        }
        setApiKeyEditOpen(false);
        setEditingApiKey(null);
        setRefreshTick((s) => ({ ...s, api_keys: s.api_keys + 1 }));
    };

    const apiKeyColumns: DataTableColumn<ApiKey>[] = [
        { id: "name", label: "Name", width: 240, cell: ({ row }) => <span className="font-medium">{row.name}</span> },
        { id: "prefix", label: "Prefix", width: 160, cell: ({ row }) => <span className="font-mono text-xs">{row.key_prefix}</span> },
        {
            id: "created_at",
            label: "Created",
            width: 180,
            cell: ({ row }) => <span className="text-sm text-muted-foreground">{new Date(row.created_at).toLocaleString()}</span>,
        },
        {
            id: "last_used_at",
            label: "Last used",
            width: 180,
            cell: ({ row }) =>
                row.last_used_at ? (
                    <span className="text-sm text-muted-foreground">{new Date(row.last_used_at).toLocaleString()}</span>
                ) : (
                    <span className="text-sm text-muted-foreground">—</span>
                ),
        },
        {
            id: "status",
            label: "Status",
            width: 120,
            cell: ({ row }) =>
                row.revoked_at ? (
                    <span className="text-xs px-2 py-1 rounded bg-gray-200 text-gray-700">revoked</span>
                ) : (
                    <span className="text-xs px-2 py-1 rounded bg-green-200 text-green-800">active</span>
                ),
        },
        {
            id: "actions",
            label: "Actions",
            width: 170,
            cell: ({ row }) => (
                <div className="flex items-center gap-2 justify-end">
                    <Button variant="ghost" size="sm" onClick={() => openRenameApiKey(row)}>
                        <Pencil className="h-4 w-4 mr-2" />
                        Modifier
                    </Button>
                    <Button variant="destructive" size="sm" onClick={() => revokeApiKey(row.id)} disabled={!!row.revoked_at}>
                        Revoke
                    </Button>
                </div>
            ),
        },
    ];

    return (
        <div className="p-8 space-y-8">
            <div>
                <h1 className="text-3xl font-bold tracking-tight">Settings</h1>
                <p className="text-muted-foreground">Manage catalog and configuration.</p>
            </div>

            <Tabs value={activeTab} onValueChange={(v) => setActiveTab(v as typeof activeTab)} className="w-full">
                <TabsList>
                    <TabsTrigger value="providers">Providers</TabsTrigger>
                    <TabsTrigger value="regions">Regions</TabsTrigger>
                    <TabsTrigger value="zones">Zones</TabsTrigger>
                    <TabsTrigger value="types">Instance Types</TabsTrigger>
                    <TabsTrigger value="models">Models</TabsTrigger>
                    <TabsTrigger value="api_keys">API Keys</TabsTrigger>
                </TabsList>

                {/* PROVIDERS */}
                <TabsContent value="providers">
                    <Card>
                        <CardContent>
                            <VirtualizedDataTable<Provider>
                                listId="settings:providers"
                                title="Providers"
                                dataKey={String(refreshTick.providers)}
                                rightHeader={
                                    <div className="flex gap-2">
                                        <Button size="sm" onClick={() => openCreate("provider")}>
                                            <Plus className="h-4 w-4 mr-2" />
                                            Ajouter
                                        </Button>
                                    </div>
                                }
                                autoHeight={true}
                                height={300}
                                rowHeight={52}
                                columns={providerColumns}
                                loadRange={loadProviders}
                            />
                        </CardContent>
                    </Card>
                </TabsContent>

                {/* REGIONS */}
                <TabsContent value="regions">
                    <Card>
                        <CardContent>
                            <VirtualizedDataTable<Region>
                                listId="settings:regions"
                                title="Regions"
                                dataKey={String(refreshTick.regions)}
                                rightHeader={
                                    <div className="flex gap-2">
                                        <Button size="sm" onClick={() => openCreate("region")}>
                                            <Plus className="h-4 w-4 mr-2" />
                                            Ajouter
                                        </Button>
                                    </div>
                                }
                                autoHeight={true}
                                height={300}
                                rowHeight={52}
                                columns={regionColumns}
                                loadRange={loadRegions}
                            />
                        </CardContent>
                    </Card>
                </TabsContent>

                {/* ZONES */}
                <TabsContent value="zones">
                    <Card>
                        <CardContent>
                            <VirtualizedDataTable<Zone>
                                listId="settings:zones"
                                title="Zones"
                                dataKey={String(refreshTick.zones)}
                                rightHeader={
                                    <div className="flex gap-2">
                                        <Button size="sm" onClick={() => openCreate("zone")}>
                                            <Plus className="h-4 w-4 mr-2" />
                                            Ajouter
                                        </Button>
                                    </div>
                                }
                                autoHeight={true}
                                height={300}
                                rowHeight={52}
                                columns={zoneColumns}
                                loadRange={loadZones}
                            />
                        </CardContent>
                    </Card>
                </TabsContent>

                {/* INSTANCE TYPES */}
                <TabsContent value="types">
                    <Card>
                        <CardContent>
                            <VirtualizedDataTable<InstanceType>
                                listId="settings:types"
                                title="Instance Types"
                                dataKey={String(refreshTick.types)}
                                rightHeader={
                                    <div className="flex gap-2">
                                        <Button size="sm" onClick={() => openCreate("type")}>
                                            <Plus className="h-4 w-4 mr-2" />
                                            Ajouter
                                        </Button>
                                    </div>
                                }
                                autoHeight={true}
                                height={300}
                                rowHeight={52}
                                columns={typeColumns}
                                loadRange={loadTypes}
                            />
                        </CardContent>
                    </Card>
                </TabsContent>

                {/* MODELS */}
                <TabsContent value="models">
                    <Card>
                        <CardContent>
                            <VirtualizedDataTable<ModelRow>
                                listId="settings:models"
                                title="Models"
                                dataKey={String(refreshTick.models)}
                                rightHeader={
                                    <div className="flex gap-2">
                                        <Button size="sm" onClick={openCreateModel} disabled={modelsLoading}>
                                            <Plus className="h-4 w-4 mr-2" />
                                            Ajouter
                                        </Button>
                                    </div>
                                }
                                autoHeight={true}
                                height={300}
                                rowHeight={52}
                                columns={modelColumns}
                                rows={models}
                            />
                        </CardContent>
                    </Card>
                </TabsContent>

                {/* API KEYS */}
                <TabsContent value="api_keys">
                    <Card>
                        <CardContent>
                            <VirtualizedDataTable<ApiKey>
                                listId="settings:api_keys"
                                title="API Keys"
                                dataKey={String(refreshTick.api_keys)}
                                rightHeader={
                                    <div className="flex gap-2">
                                        <Button size="sm" onClick={() => { setApiKeyCreateOpen(true); setCreatedApiKey(null); }}>
                                            <Plus className="h-4 w-4 mr-2" />
                                            Ajouter
                                        </Button>
                                    </div>
                                }
                                autoHeight={true}
                                height={300}
                                rowHeight={52}
                                columns={apiKeyColumns}
                                rows={apiKeys}
                            />
                        </CardContent>
                    </Card>
                </TabsContent>
            </Tabs>

            {/* Create API Key dialog (shows secret once) */}
            <Dialog open={apiKeyCreateOpen} onOpenChange={(open) => { setApiKeyCreateOpen(open); if (!open) { setCreatedApiKey(null); setApiKeyName(''); } }}>
                <DialogContent showCloseButton={false}>
                    <DialogHeader>
                        <DialogTitle>Ajouter une API Key</DialogTitle>
                    </DialogHeader>
                    <div className="grid gap-4 py-4">
                        {!createdApiKey ? (
                            <div className="grid grid-cols-4 items-center gap-4">
                                <Label htmlFor="api_key_name" className="text-right">Nom</Label>
                                <Input
                                    id="api_key_name"
                                    value={apiKeyName}
                                    onChange={(e) => setApiKeyName(e.target.value)}
                                    className="col-span-3"
                                    placeholder="ex: prod-backend, n8n, langchain..."
                                />
                            </div>
                        ) : (
                            <div className="space-y-2">
                                <p className="text-sm text-muted-foreground">
                                    Copie ta clé maintenant. Elle ne sera affichée qu’une seule fois.
                                </p>
                                <div className="flex items-center gap-2">
                                    <Input value={createdApiKey} readOnly className="font-mono text-xs" />
                                    <CopyButton text={createdApiKey} />
                                </div>
                            </div>
                        )}
                    </div>
                    <DialogFooter>
                        <Button variant="secondary" onClick={() => setApiKeyCreateOpen(false)}>
                            Fermer
                        </Button>
                        {!createdApiKey && (
                            <Button onClick={createApiKey} disabled={!apiKeyName.trim()}>
                                Créer
                            </Button>
                        )}
                    </DialogFooter>
                </DialogContent>
            </Dialog>

            {/* Rename API Key dialog */}
            <Dialog open={apiKeyEditOpen} onOpenChange={(open) => { setApiKeyEditOpen(open); if (!open) { setEditingApiKey(null); setApiKeyEditName(''); } }}>
                <DialogContent showCloseButton={false}>
                    <DialogHeader>
                        <DialogTitle>Modifier l’API Key</DialogTitle>
                    </DialogHeader>
                    <div className="grid gap-4 py-4">
                        <div className="grid grid-cols-4 items-center gap-4">
                            <Label htmlFor="api_key_edit_name" className="text-right">Nom</Label>
                            <Input id="api_key_edit_name" value={apiKeyEditName} onChange={(e) => setApiKeyEditName(e.target.value)} className="col-span-3" />
                        </div>
                    </div>
                    <DialogFooter>
                        <Button variant="secondary" onClick={() => setApiKeyEditOpen(false)}>
                            Annuler
                        </Button>
                        <Button onClick={saveRenameApiKey} disabled={!apiKeyEditName.trim()}>
                            Enregistrer
                        </Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>

            <Dialog
                open={isEditOpen}
                onOpenChange={(open) => {
                    setIsEditOpen(open);
                    if (!open) {
                        setEditingEntity(null);
                        setEntityType(null);
                    }
                }}
            >
                <DialogContent showCloseButton={false}>
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
                            <Input id="name" value={formData.name} onChange={(e) => setFormData({ ...formData, name: e.target.value })} className="col-span-3" />
                        </div>
                        {entityType !== "model" && (
                            <div className="grid grid-cols-4 items-center gap-4">
                                <Label htmlFor="code" className="text-right">Code</Label>
                                <Input id="code" value={formData.code} onChange={(e) => setFormData({ ...formData, code: e.target.value })} className="col-span-3" />
                            </div>
                        )}
                        {entityType === "region" && (
                            <div className="grid grid-cols-4 items-center gap-4">
                                <Label className="text-right">Provider</Label>
                                <div className="col-span-3">
                                    <Select value={formData.provider_id} onValueChange={(v) => setFormData({ ...formData, provider_id: v })}>
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
                                    <Select value={formData.region_id} onValueChange={(v) => setFormData({ ...formData, region_id: v })}>
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
                            <div className="grid grid-cols-4 items-center gap-4">
                                <Label htmlFor="description" className="text-right">Description</Label>
                                <Input
                                    id="description"
                                    value={formData.description}
                                    onChange={(e) => setFormData({ ...formData, description: e.target.value })}
                                    className="col-span-3"
                                />
                            </div>
                        )}
                        {entityType === 'type' && (
                            <>
                                <div className="grid grid-cols-4 items-center gap-4">
                                    <Label className="text-right">Provider</Label>
                                    <div className="col-span-3">
                                        <Select value={formData.provider_id} onValueChange={(v) => setFormData({ ...formData, provider_id: v })}>
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
                                    <Input id="gpu_count" value={formData.gpu_count} onChange={(e) => setFormData({ ...formData, gpu_count: e.target.value })} className="col-span-3" />
                                </div>
                                <div className="grid grid-cols-4 items-center gap-4">
                                    <Label htmlFor="vram_per_gpu_gb" className="text-right">VRAM/GPU</Label>
                                    <Input id="vram_per_gpu_gb" value={formData.vram_per_gpu_gb} onChange={(e) => setFormData({ ...formData, vram_per_gpu_gb: e.target.value })} className="col-span-3" />
                                </div>
                                <div className="grid grid-cols-4 items-center gap-4">
                                    <Label htmlFor="cpu_count" className="text-right">vCPU</Label>
                                    <Input id="cpu_count" value={formData.cpu_count} onChange={(e) => setFormData({ ...formData, cpu_count: e.target.value })} className="col-span-3" />
                                </div>
                                <div className="grid grid-cols-4 items-center gap-4">
                                    <Label htmlFor="ram_gb" className="text-right">RAM (GB)</Label>
                                    <Input id="ram_gb" value={formData.ram_gb} onChange={(e) => setFormData({ ...formData, ram_gb: e.target.value })} className="col-span-3" />
                                </div>
                                <div className="grid grid-cols-4 items-center gap-4">
                                    <Label htmlFor="bandwidth_bps" className="text-right">Bandwidth (bps)</Label>
                                    <Input id="bandwidth_bps" value={formData.bandwidth_bps} onChange={(e) => setFormData({ ...formData, bandwidth_bps: e.target.value })} className="col-span-3" />
                                </div>
                                <div className="grid grid-cols-4 items-center gap-4">
                                    <Label htmlFor="cost" className="text-right">Coût ($/h)</Label>
                                    <Input id="cost" type="number" step="0.0001" value={formData.cost_per_hour} onChange={(e) => setFormData({ ...formData, cost_per_hour: e.target.value })} className="col-span-3" />
                                </div>
                            </>
                        )}
                        {entityType === 'model' && (
                            <>
                                <div className="grid grid-cols-4 items-center gap-4">
                                    <Label htmlFor="model_id" className="text-right">HF model_id</Label>
                                    <Input id="model_id" value={formData.model_id} onChange={(e) => setFormData({ ...formData, model_id: e.target.value })} className="col-span-3" />
                                </div>
                                <div className="grid grid-cols-4 items-center gap-4">
                                    <Label htmlFor="required_vram_gb" className="text-right">VRAM (GB)</Label>
                                    <Input id="required_vram_gb" value={formData.required_vram_gb} onChange={(e) => setFormData({ ...formData, required_vram_gb: e.target.value })} className="col-span-3" />
                                </div>
                                <div className="grid grid-cols-4 items-center gap-4">
                                    <Label htmlFor="context_length" className="text-right">Context</Label>
                                    <Input id="context_length" value={formData.context_length} onChange={(e) => setFormData({ ...formData, context_length: e.target.value })} className="col-span-3" />
                                </div>
                                <div className="grid grid-cols-4 items-center gap-4">
                                    <Label htmlFor="data_volume_gb" className="text-right">Disk GB</Label>
                                    <Input id="data_volume_gb" value={formData.data_volume_gb} onChange={(e) => setFormData({ ...formData, data_volume_gb: e.target.value })} className="col-span-3" />
                                </div>
                            </>
                        )}
                        <div className="grid grid-cols-4 items-center gap-4">
                            <Label htmlFor="active" className="text-right">Actif</Label>
                            <ActiveToggle checked={formData.is_active} onCheckedChange={(c) => setFormData({ ...formData, is_active: c })} aria-label="Toggle active" />
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


