"use client";

import { useEffect, useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Check, X, Loader2 } from "lucide-react";
import { apiUrl } from "@/lib/api";
import type { Zone, InstanceType } from "@/lib/types";

type InstanceTypeZoneAssociation = {
  instance_type_id: string;
  zone_id: string;
  is_available: boolean;
  zone_name: string;
  zone_code: string;
};

type ManageZonesModalProps = {
  open: boolean;
  onClose: () => void;
  instanceType: Pick<InstanceType, "id" | "name" | "code" | "provider_id"> | null;
};

export function ManageZonesModal({
  open,
  onClose,
  instanceType,
}: ManageZonesModalProps) {
  const [allZones, setAllZones] = useState<Zone[]>([]);
  const [linkedZoneIds, setLinkedZoneIds] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(false);
  const [actionInProgress, setActionInProgress] = useState<string | null>(null);

  useEffect(() => {
    if (!open || !instanceType) return;

    const fetchData = async () => {
      setLoading(true);
      try {
        // Fetch zones for the same provider only (domain: instance types are provider-scoped)
        const qs = new URLSearchParams();
        if (instanceType.provider_id) qs.set("provider_id", instanceType.provider_id);
        qs.set("is_active", "true");
        qs.set("offset", "0");
        qs.set("limit", "500");
        const zonesRes = await fetch(apiUrl(`zones/search?${qs.toString()}`));
        if (zonesRes.ok) {
          const data = (await zonesRes.json()) as { rows?: Zone[] };
          const zones: Zone[] = data.rows ?? [];
          setAllZones(zones.filter((z) => z.is_active));
        }

        // Fetch linked zones for this instance type
        const linkedRes = await fetch(
          apiUrl(`instance_types/${instanceType.id}/zones`)
        );
        if (linkedRes.ok) {
          const linked: InstanceTypeZoneAssociation[] = await linkedRes.json();
          setLinkedZoneIds(new Set(linked.map((a) => a.zone_id)));
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
      // Backend contract: PUT /instance_types/:id/zones with full replacement list
      const next = new Set(linkedZoneIds);
      if (isLinked) next.delete(zoneId);
      else next.add(zoneId);

      const res = await fetch(apiUrl(`instance_types/${instanceType.id}/zones`), {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ zone_ids: Array.from(next) }),
      });

      if (res.ok) {
        setLinkedZoneIds(next);
      } else {
        const errorText = await res.text();
        alert(`Failed to ${isLinked ? "unlink" : "link"} zone: ${errorText}`);
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
      <DialogContent showCloseButton={false} className="sm:max-w-[600px]">
        <DialogHeader>
          <DialogTitle>
            Gérer les zones pour{" "}
            <span className="font-mono text-primary">{instanceType.name}</span>
          </DialogTitle>
        </DialogHeader>

        {loading ? (
          <div className="flex items-center justify-center py-8">
            <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
          </div>
        ) : (
          <div className="py-4">
            <p className="text-sm text-muted-foreground mb-4">
              Sélectionnez les zones où ce type d’instance est disponible. Les utilisateurs ne pourront créer
              une instance de ce type que dans les zones associées.
            </p>

            <div className="space-y-2 max-h-[400px] overflow-y-auto">
              {allZones.length === 0 ? (
                <div className="text-center text-muted-foreground py-8">
                  Aucune zone active trouvée
                </div>
              ) : (
                allZones.map((zone) => {
                  const isLinked = linkedZoneIds.has(zone.id);
                  const isProcessing = actionInProgress === zone.id;

                  return (
                    <div
                      key={zone.id}
                      className="flex items-center justify-between p-3 border rounded-lg hover:bg-muted/50 transition-colors"
                    >
                      <div className="flex items-center gap-3">
                        <div
                          className={`w-5 h-5 rounded-full flex items-center justify-center transition-colors ${
                            isLinked
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
                            Retirer
                          </>
                        ) : (
                          <>
                            <Check className="h-4 w-4 mr-1" />
                            Ajouter
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
                  {linkedZoneIds.size} / {allZones.length} zones associées
                </Badge>
              </div>
            </div>
          </div>
        )}

        <DialogFooter className="sm:justify-between">
          <Button variant="outline" onClick={onClose}>
            Fermer
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}


