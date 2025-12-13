"use client";

import { useEffect, useState } from "react";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Check, X, Loader2 } from "lucide-react";
import { apiUrl } from "@/lib/api";

type Zone = {
    id: string;
    name: string;
    code: string;
    is_active: boolean;
};

type ManageZonesModalProps = {
    open: boolean;
    onClose: () => void;
    instanceType: {
        id: string;
        name: string;
        code: string | null;
    } | null;
};

export function ManageZonesModal({ open, onClose, instanceType }: ManageZonesModalProps) {
    const [allZones, setAllZones] = useState<Zone[]>([]);
    const [linkedZoneIds, setLinkedZoneIds] = useState<Set<string>>(new Set());
    const [loading, setLoading] = useState(false);
    const [actionInProgress, setActionInProgress] = useState<string | null>(null);

    useEffect(() => {
        if (!open || !instanceType) return;

        const fetchData = async () => {
            setLoading(true);
            try {
                // Fetch all zones
                const zonesRes = await fetch(apiUrl("zones"));
                if (zonesRes.ok) {
                    const zones: Zone[] = await zonesRes.json();
                    setAllZones(zones.filter(z => z.is_active));
                }

                // Fetch linked zones for this instance type
                const linkedRes = await fetch(apiUrl(`instance_types/${instanceType.id}/zones`));
                if (linkedRes.ok) {
                    const linked: Zone[] = await linkedRes.json();
                    setLinkedZoneIds(new Set(linked.map(z => z.id)));
                }
            } catch (err) {
                console.error("Failed to fetch zones data", err);
            } finally {
                setLoading(false);
            }
        };

        fetchData();
    }, [open, instanceType]);

    const handleToggleZone = async (zoneId: string, isLinked: boolean) => {
        if (!instanceType) return;

        setActionInProgress(zoneId);
        try {
            const endpoint = isLinked
                ? apiUrl(`instance_types/${instanceType.id}/zones/${zoneId}`)
                : apiUrl(`instance_types/${instanceType.id}/zones`);

            const method = isLinked ? "DELETE" : "POST";
            const body = isLinked ? undefined : JSON.stringify({ zone_id: zoneId });

            const res = await fetch(endpoint, {
                method,
                headers: body ? { "Content-Type": "application/json" } : undefined,
                body,
            });

            if (res.ok) {
                // Update local state
                setLinkedZoneIds(prev => {
                    const newSet = new Set(prev);
                    if (isLinked) {
                        newSet.delete(zoneId);
                    } else {
                        newSet.add(zoneId);
                    }
                    return newSet;
                });
            } else {
                const errorText = await res.text();
                alert(`Failed to ${isLinked ? 'unlink' : 'link'} zone: ${errorText}`);
            }
        } catch (err) {
            console.error("Failed to toggle zone", err);
            alert("Error updating zone association");
        } finally {
            setActionInProgress(null);
        }
    };

    if (!instanceType) return null;

    return (
        <Dialog open={open} onOpenChange={onClose}>
            <DialogContent className="sm:max-w-[600px]">
                <DialogHeader>
                    <DialogTitle>
                        Manage Zones for <span className="font-mono text-primary">{instanceType.name}</span>
                    </DialogTitle>
                </DialogHeader>

                {loading ? (
                    <div className="flex items-center justify-center py-8">
                        <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
                    </div>
                ) : (
                    <div className="py-4">
                        <p className="text-sm text-muted-foreground mb-4">
                            Select which zones support this instance type. Users will only be able to deploy this instance type in linked zones.
                        </p>

                        <div className="space-y-2 max-h-[400px] overflow-y-auto">
                            {allZones.length === 0 ? (
                                <div className="text-center text-muted-foreground py-8">
                                    No active zones found
                                </div>
                            ) : (
                                allZones.map(zone => {
                                    const isLinked = linkedZoneIds.has(zone.id);
                                    const isProcessing = actionInProgress === zone.id;

                                    return (
                                        <div
                                            key={zone.id}
                                            className="flex items-center justify-between p-3 border rounded-lg hover:bg-muted/50 transition-colors"
                                        >
                                            <div className="flex items-center gap-3">
                                                <div
                                                    className={`w-5 h-5 rounded-full flex items-center justify-center transition-colors ${isLinked
                                                        ? "bg-green-500 text-white"
                                                        : "bg-muted border-2 border-muted-foreground/30"
                                                        }`}
                                                >
                                                    {isLinked && <Check className="h-3 w-3" />}
                                                </div>
                                                <div>
                                                    <div className="font-medium">{zone.name}</div>
                                                    <div className="text-xs text-muted-foreground font-mono">
                                                        {zone.code}
                                                    </div>
                                                </div>
                                            </div>

                                            <Button
                                                size="sm"
                                                variant={isLinked ? "destructive" : "default"}
                                                onClick={() => handleToggleZone(zone.id, isLinked)}
                                                disabled={isProcessing}
                                            >
                                                {isProcessing ? (
                                                    <Loader2 className="h-4 w-4 animate-spin" />
                                                ) : isLinked ? (
                                                    <>
                                                        <X className="h-4 w-4 mr-1" />
                                                        Unlink
                                                    </>
                                                ) : (
                                                    <>
                                                        <Check className="h-4 w-4 mr-1" />
                                                        Link
                                                    </>
                                                )}
                                            </Button>
                                        </div>
                                    );
                                })
                            )}
                        </div>

                        <div className="mt-4 pt-4 border-t">
                            <div className="flex items-center gap-2 text-sm">
                                <Badge variant="outline">
                                    {linkedZoneIds.size} / {allZones.length} zones linked
                                </Badge>
                            </div>
                        </div>
                    </div>
                )}

                <DialogFooter>
                    <Button onClick={onClose}>Close</Button>
                </DialogFooter>
            </DialogContent>
        </Dialog>
    );
}
