"use client";

import { useEffect, useState } from "react";
import { apiUrl } from "@/lib/api";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Table, TableHeader, TableRow, TableHead, TableBody, TableCell } from "@/components/ui/table";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Pencil, Check, Settings2 } from "lucide-react";
import { ManageZonesModal } from "@/components/ManageZonesModal";

// --- Types ---

type Region = { id: string; name: string; code: string | null; is_active: boolean };
type Zone = { id: string; name: string; code: string | null; is_active: boolean };
type InstanceType = {
    id: string;
    name: string;
    code: string | null;
    is_active: boolean;
    cost_per_hour: number | null;
    gpu_count: number;
    vram_per_gpu_gb: number;
};

export default function SettingsPage() {
    const [regions, setRegions] = useState<Region[]>([]);
    const [zones, setZones] = useState<Zone[]>([]);
    const [types, setTypes] = useState<InstanceType[]>([]);

    const [editingEntity, setEditingEntity] = useState<any>(null);
    const [entityType, setEntityType] = useState<'region' | 'zone' | 'type' | null>(null);
    const [isEditOpen, setIsEditOpen] = useState(false);

    // Manage Zones Modal State
    const [isManageZonesOpen, setIsManageZonesOpen] = useState(false);
    const [selectedInstanceType, setSelectedInstanceType] = useState<InstanceType | null>(null);

    const [formData, setFormData] = useState({
        code: "",
        name: "",
        is_active: true,
        cost_per_hour: ""
    });

    const fetchData = async () => {
        try {
            const [resRegions, resZones, resTypes] = await Promise.all([
                fetch(apiUrl("regions")),
                fetch(apiUrl("zones")),
                fetch(apiUrl("instance_types"))
            ]);

            if (resRegions.ok) setRegions(await resRegions.json());
            if (resZones.ok) setZones(await resZones.json());
            if (resTypes.ok) setTypes(await resTypes.json());

        } catch (err) {
            console.error("Failed to fetch settings data", err);
        }
    };

    useEffect(() => {
        fetchData();
    }, []);

    const handleEdit = (entity: any, type: 'region' | 'zone' | 'type') => {
        setEditingEntity(entity);
        setEntityType(type);
        setFormData({
            code: entity.code || "",
            name: entity.name || "",
            is_active: entity.is_active,
            cost_per_hour: entity.cost_per_hour ? entity.cost_per_hour.toString() : ""
        });
        setIsEditOpen(true);
    };

    const handleSave = async () => {
        if (!editingEntity || !entityType) return;

        const url = apiUrl(`${entityType === 'type' ? 'instance_types' : entityType + 's'}/${editingEntity.id}`);

        const payload: any = {
            code: formData.code,
            name: formData.name,
            is_active: formData.is_active
        };

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
                fetchData(); // Refresh
            } else {
                alert("Failed to save");
            }
        } catch (err) {
            console.error("Save failed", err);
        }
    };

    const toggleActive = async (entity: any, type: 'region' | 'zone' | 'type') => {
        // Quick toggle without modal
        const url = apiUrl(`${type === 'type' ? 'instance_types' : type + 's'}/${entity.id}`);
        try {
            await fetch(url, {
                method: "PUT",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ is_active: !entity.is_active })
            });
            fetchData();
        } catch (err) {
            console.error("Toggle failed", err);
        }
    };

    return (
        <div className="p-8 space-y-8">
            <div>
                <h1 className="text-3xl font-bold tracking-tight">Settings</h1>
                <p className="text-muted-foreground">Manage catalog and configuration.</p>
            </div>

            <Tabs defaultValue="regions" className="w-full">
                <TabsList>
                    <TabsTrigger value="regions">Regions</TabsTrigger>
                    <TabsTrigger value="zones">Zones</TabsTrigger>
                    <TabsTrigger value="types">Instance Types</TabsTrigger>
                </TabsList>

                {/* REGIONS */}
                <TabsContent value="regions">
                    <Card>
                        <CardHeader><CardTitle>Regions</CardTitle></CardHeader>
                        <CardContent>
                            <Table>
                                <TableHeader>
                                    <TableRow>
                                        <TableHead>Name</TableHead>
                                        <TableHead>Code</TableHead>
                                        <TableHead>Status</TableHead>
                                        <TableHead className="text-right">Actions</TableHead>
                                    </TableRow>
                                </TableHeader>
                                <TableBody>
                                    {regions.map(r => (
                                        <TableRow key={r.id}>
                                            <TableCell className="font-medium">{r.name}</TableCell>
                                            <TableCell className="font-mono text-xs">{r.code}</TableCell>
                                            <TableCell>
                                                <Switch checked={r.is_active} onCheckedChange={() => toggleActive(r, 'region')} />
                                            </TableCell>
                                            <TableCell className="text-right">
                                                <Button variant="ghost" size="icon" onClick={() => handleEdit(r, 'region')}>
                                                    <Pencil className="h-4 w-4" />
                                                </Button>
                                            </TableCell>
                                        </TableRow>
                                    ))}
                                </TableBody>
                            </Table>
                        </CardContent>
                    </Card>
                </TabsContent>

                {/* ZONES */}
                <TabsContent value="zones">
                    <Card>
                        <CardHeader><CardTitle>Zones</CardTitle></CardHeader>
                        <CardContent>
                            <Table>
                                <TableHeader>
                                    <TableRow>
                                        <TableHead>Name</TableHead>
                                        <TableHead>Code</TableHead>
                                        <TableHead>Status</TableHead>
                                        <TableHead className="text-right">Actions</TableHead>
                                    </TableRow>
                                </TableHeader>
                                <TableBody>
                                    {zones.map(z => (
                                        <TableRow key={z.id}>
                                            <TableCell className="font-medium">{z.name}</TableCell>
                                            <TableCell className="font-mono text-xs">{z.code}</TableCell>
                                            <TableCell>
                                                <Switch checked={z.is_active} onCheckedChange={() => toggleActive(z, 'zone')} />
                                            </TableCell>
                                            <TableCell className="text-right">
                                                <Button variant="ghost" size="icon" onClick={() => handleEdit(z, 'zone')}>
                                                    <Pencil className="h-4 w-4" />
                                                </Button>
                                            </TableCell>
                                        </TableRow>
                                    ))}
                                </TableBody>
                            </Table>
                        </CardContent>
                    </Card>
                </TabsContent>

                {/* INSTANCE TYPES */}
                <TabsContent value="types">
                    <Card>
                        <CardHeader><CardTitle>Instance Types</CardTitle></CardHeader>
                        <CardContent>
                            <Table>
                                <TableHeader>
                                    <TableRow>
                                        <TableHead>Name</TableHead>
                                        <TableHead>Code</TableHead>
                                        <TableHead>Specs</TableHead>
                                        <TableHead>Cost/Hr</TableHead>
                                        <TableHead>Status</TableHead>
                                        <TableHead className="text-right">Actions</TableHead>
                                    </TableRow>
                                </TableHeader>
                                <TableBody>
                                    {types.map(t => (
                                        <TableRow key={t.id}>
                                            <TableCell className="font-medium">{t.name}</TableCell>
                                            <TableCell className="font-mono text-xs">{t.code}</TableCell>
                                            <TableCell className="text-xs text-muted-foreground">{t.gpu_count}x GPU, {t.vram_per_gpu_gb}GB VRAM</TableCell>
                                            <TableCell>${t.cost_per_hour}</TableCell>
                                            <TableCell>
                                                <Switch checked={t.is_active} onCheckedChange={() => toggleActive(t, 'type')} />
                                            </TableCell>
                                            <TableCell className="text-right">
                                                <div className="flex justify-end gap-2">
                                                    <Button
                                                        variant="outline"
                                                        size="sm"
                                                        onClick={() => {
                                                            setSelectedInstanceType(t);
                                                            setIsManageZonesOpen(true);
                                                        }}
                                                        title="Manage Zones"
                                                    >
                                                        <Settings2 className="h-4 w-4 mr-1" />
                                                        Manage Zones
                                                    </Button>
                                                    <Button variant="ghost" size="icon" onClick={() => handleEdit(t, 'type')}>
                                                        <Pencil className="h-4 w-4" />
                                                    </Button>
                                                </div>
                                            </TableCell>
                                        </TableRow>
                                    ))}
                                </TableBody>
                            </Table>
                        </CardContent>
                    </Card>
                </TabsContent>
            </Tabs>

            <Dialog open={isEditOpen} onOpenChange={setIsEditOpen}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle>Edit {entityType === 'type' ? 'Instance Type' : entityType === 'region' ? 'Region' : 'Zone'}</DialogTitle>
                    </DialogHeader>
                    <div className="grid gap-4 py-4">
                        <div className="grid grid-cols-4 items-center gap-4">
                            <Label htmlFor="name" className="text-right">Name</Label>
                            <Input id="name" value={formData.name} onChange={(e) => setFormData({ ...formData, name: e.target.value })} className="col-span-3" />
                        </div>
                        <div className="grid grid-cols-4 items-center gap-4">
                            <Label htmlFor="code" className="text-right">Code</Label>
                            <Input id="code" value={formData.code} onChange={(e) => setFormData({ ...formData, code: e.target.value })} className="col-span-3" />
                        </div>
                        {entityType === 'type' && (
                            <div className="grid grid-cols-4 items-center gap-4">
                                <Label htmlFor="cost" className="text-right">Cost ($/hr)</Label>
                                <Input id="cost" type="number" step="0.0001" value={formData.cost_per_hour} onChange={(e) => setFormData({ ...formData, cost_per_hour: e.target.value })} className="col-span-3" />
                            </div>
                        )}
                        <div className="grid grid-cols-4 items-center gap-4">
                            <Label htmlFor="active" className="text-right">Active</Label>
                            <Switch id="active" checked={formData.is_active} onCheckedChange={(c) => setFormData({ ...formData, is_active: c })} />
                        </div>
                    </div>
                    <DialogFooter>
                        <Button onClick={handleSave}>Save changes</Button>
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
