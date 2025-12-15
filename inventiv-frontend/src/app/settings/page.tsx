"use client";

import { useState } from "react";
import { apiUrl } from "@/lib/api";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { Pencil, Settings2, Power } from "lucide-react";
import { ManageZonesModal } from "@/components/settings/ManageZonesModal";
import type { Provider, Region, Zone, InstanceType } from "@/lib/types";
import { VirtualizedDataTable, type DataTableColumn } from "@/components/shared/VirtualizedDataTable";
import type { LoadRangeResult } from "@/components/shared/VirtualizedRemoteList";
import { formatEur } from "@/lib/utils";

export default function SettingsPage() {
    const [activeTab, setActiveTab] = useState<"providers" | "regions" | "zones" | "types">("regions");
    const [refreshTick, setRefreshTick] = useState({ providers: 0, regions: 0, zones: 0, types: 0 });

    type EntityType = "provider" | "region" | "zone" | "type";
    type RefreshKey = "providers" | "regions" | "zones" | "types";
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
        }
    };

    const [confirmToggle, setConfirmToggle] = useState<{
        type: EntityType;
        id: string;
        name: string;
        nextActive: boolean;
    } | null>(null);

    const [editingEntity, setEditingEntity] = useState<Provider | Region | Zone | InstanceType | null>(null);
    const [entityType, setEntityType] = useState<EntityType | null>(null);
    const [isEditOpen, setIsEditOpen] = useState(false);

    // Manage Zones Modal State
    const [isManageZonesOpen, setIsManageZonesOpen] = useState(false);
    const [selectedInstanceType, setSelectedInstanceType] = useState<InstanceType | null>(null);

    const [formData, setFormData] = useState({
        code: "",
        name: "",
        description: "",
        is_active: true,
        cost_per_hour: ""
    });

    type SearchResponse<T> = {
        offset: number;
        limit: number;
        total_count: number;
        filtered_count: number;
        rows: T[];
    };

    const handleEdit = (entity: Provider | Region | Zone | InstanceType, type: 'provider' | 'region' | 'zone' | 'type') => {
        setEditingEntity(entity);
        setEntityType(type);
        setFormData({
            code: entity.code ?? "",
            name: entity.name || "",
            description: type === 'provider' ? (((entity as Provider).description ?? "") || "") : "",
            is_active: entity.is_active ?? false,
            cost_per_hour:
                type === 'type' && (entity as InstanceType).cost_per_hour != null
                    ? String((entity as InstanceType).cost_per_hour)
                    : ""
        });
        setIsEditOpen(true);
    };

    const handleSave = async () => {
        if (!editingEntity || !entityType) return;

        const url = apiUrl(`${entityType === 'type' ? 'instance_types' : entityType + 's'}/${editingEntity.id}`);

        const payload: { code?: string; name?: string; description?: string; is_active?: boolean; cost_per_hour?: number | null } = {
            code: formData.code,
            name: formData.name,
            is_active: formData.is_active
        };

        if (entityType === 'provider') {
            payload.description = formData.description;
        }

        if (entityType === 'type') {
            payload.cost_per_hour = formData.cost_per_hour ? parseFloat(formData.cost_per_hour) : null;
        }

        try {
            const res = await fetch(url, {
                method: "PUT",
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

    const renderStatusChip = (isActive?: boolean | null) => {
        const active = !!isActive;
        return (
            <Badge className={active ? "bg-green-600 hover:bg-green-700 text-white" : "bg-gray-200 hover:bg-gray-300 text-gray-700"}>
                {active ? "active" : "inactive"}
            </Badge>
        );
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
            label: "Status",
            width: 120,
            cell: ({ row }) => renderStatusChip(row.is_active),
        },
        {
            id: "actions",
            label: "Actions",
            width: 240,
            align: "right",
            disableReorder: true,
            cell: ({ row }) => (
                <div className="flex justify-end gap-2" onClick={(e) => e.stopPropagation()}>
                    <Button
                        variant="outline"
                        size="sm"
                        onClick={() => setConfirmToggle({ type: "provider", id: row.id, name: row.name, nextActive: !row.is_active })}
                    >
                        <Power className="h-4 w-4 mr-1" />
                        {row.is_active ? "Désactiver" : "Activer"}
                    </Button>
                    <Button variant="ghost" size="icon" onClick={() => handleEdit(row, 'provider')}>
                        <Pencil className="h-4 w-4" />
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
            label: "Status",
            width: 120,
            cell: ({ row }) => renderStatusChip(row.is_active),
        },
        {
            id: "actions",
            label: "Actions",
            width: 240,
            align: "right",
            disableReorder: true,
            cell: ({ row }) => (
                <div className="flex justify-end gap-2" onClick={(e) => e.stopPropagation()}>
                    <Button
                        variant="outline"
                        size="sm"
                        onClick={() => setConfirmToggle({ type: "region", id: row.id, name: row.name, nextActive: !row.is_active })}
                    >
                        <Power className="h-4 w-4 mr-1" />
                        {row.is_active ? "Désactiver" : "Activer"}
                    </Button>
                    <Button variant="ghost" size="icon" onClick={() => handleEdit(row, 'region')}>
                        <Pencil className="h-4 w-4" />
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
            label: "Status",
            width: 120,
            cell: ({ row }) => renderStatusChip(row.is_active),
        },
        {
            id: "actions",
            label: "Actions",
            width: 240,
            align: "right",
            disableReorder: true,
            cell: ({ row }) => (
                <div className="flex justify-end gap-2" onClick={(e) => e.stopPropagation()}>
                    <Button
                        variant="outline"
                        size="sm"
                        onClick={() => setConfirmToggle({ type: "zone", id: row.id, name: row.name, nextActive: !row.is_active })}
                    >
                        <Power className="h-4 w-4 mr-1" />
                        {row.is_active ? "Désactiver" : "Activer"}
                    </Button>
                    <Button variant="ghost" size="icon" onClick={() => handleEdit(row, 'zone')}>
                        <Pencil className="h-4 w-4" />
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
            label: "Status",
            width: 120,
            cell: ({ row }) => renderStatusChip(row.is_active),
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
                    <Button
                        variant="outline"
                        size="sm"
                        onClick={() => setConfirmToggle({ type: "type", id: row.id, name: row.name, nextActive: !row.is_active })}
                    >
                        <Power className="h-4 w-4 mr-1" />
                        {row.is_active ? "Désactiver" : "Activer"}
                    </Button>
                    <Button variant="ghost" size="icon" onClick={() => handleEdit(row, 'type')}>
                        <Pencil className="h-4 w-4" />
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
                </TabsList>

                {/* PROVIDERS */}
                <TabsContent value="providers">
                    <Card>
                        <CardContent>
                            <VirtualizedDataTable<Provider>
                                listId="settings:providers"
                                title="Providers"
                                dataKey={String(refreshTick.providers)}
                                height={420}
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
                                height={420}
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
                                height={420}
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
                                height={420}
                                rowHeight={52}
                                columns={typeColumns}
                                loadRange={loadTypes}
                            />
                        </CardContent>
                    </Card>
                </TabsContent>
            </Tabs>

            <Dialog open={!!confirmToggle} onOpenChange={(open) => { if (!open) setConfirmToggle(null); }}>
                <DialogContent showCloseButton={false} className="sm:max-w-[520px]">
                    <DialogHeader>
                        <DialogTitle>
                            {confirmToggle?.nextActive ? "Activer" : "Désactiver"}{" "}
                            {confirmToggle?.type === "type"
                                ? "le type d’instance"
                                : confirmToggle?.type === "region"
                                    ? "la région"
                                    : confirmToggle?.type === "zone"
                                        ? "la zone"
                                        : "le provider"}
                        </DialogTitle>
                    </DialogHeader>
                    <div className="text-sm text-muted-foreground">
                        Confirmer l’action pour <span className="font-medium text-foreground">{confirmToggle?.name}</span>.
                    </div>
                    <DialogFooter className="sm:justify-between">
                        <Button variant="outline" onClick={() => setConfirmToggle(null)}>Annuler</Button>
                        <Button
                            onClick={async () => {
                                if (!confirmToggle) return;
                                await toggleActive(confirmToggle.id, confirmToggle.type, confirmToggle.nextActive);
                                setConfirmToggle(null);
                            }}
                        >
                            Confirmer
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
                            Modifier{" "}
                            {entityType === 'type'
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
                        <div className="grid grid-cols-4 items-center gap-4">
                            <Label htmlFor="code" className="text-right">Code</Label>
                            <Input id="code" value={formData.code} onChange={(e) => setFormData({ ...formData, code: e.target.value })} className="col-span-3" />
                        </div>
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
                            <div className="grid grid-cols-4 items-center gap-4">
                                <Label htmlFor="cost" className="text-right">Coût ($/h)</Label>
                                <Input id="cost" type="number" step="0.0001" value={formData.cost_per_hour} onChange={(e) => setFormData({ ...formData, cost_per_hour: e.target.value })} className="col-span-3" />
                            </div>
                        )}
                        <div className="grid grid-cols-4 items-center gap-4">
                            <Label htmlFor="active" className="text-right">Actif</Label>
                            <Switch id="active" checked={formData.is_active} onCheckedChange={(c) => setFormData({ ...formData, is_active: c })} />
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
