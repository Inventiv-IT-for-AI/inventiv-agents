"use client";

import { formatDistanceToNow, parseISO } from "date-fns";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Badge } from "@/components/ui/badge";
import { CopyButton } from "@/components/shared/CopyButton";
import type { Instance } from "@/lib/types";

type InstanceDetailsModalProps = {
  open: boolean;
  onClose: () => void;
  instance: Instance | null;
};

export function InstanceDetailsModal({
  open,
  onClose,
  instance,
}: InstanceDetailsModalProps) {
  return (
    <Dialog open={open} onOpenChange={onClose}>
      <DialogContent className="sm:max-w-[600px]">
        <DialogHeader>
          <DialogTitle>Instance Details</DialogTitle>
        </DialogHeader>
        {instance && (
          <div className="grid gap-6 py-4">
            <div className="grid grid-cols-2 gap-4">
              <div>
                <h4 className="font-semibold mb-2 text-sm text-muted-foreground">
                  Identity
                </h4>
                <div className="space-y-1 text-sm">
                  <div className="flex justify-between border-b pb-1">
                    <span>ID</span>
                    <span className="font-mono text-xs">
                      {instance.id.split("-")[0]}...
                    </span>
                  </div>
                  <div className="flex justify-between border-b pb-1">
                    <span>Provider</span>
                    <span className="font-medium">{instance.provider_name}</span>
                  </div>
                  <div className="flex justify-between border-b pb-1">
                    <span>Region</span>
                    <span>{instance.region}</span>
                  </div>
                  <div className="flex justify-between border-b pb-1">
                    <span>Zone</span>
                    <span>{instance.zone}</span>
                  </div>
                </div>
              </div>
              <div>
                <h4 className="font-semibold mb-2 text-sm text-muted-foreground">
                  Specs & Status
                </h4>
                <div className="space-y-1 text-sm">
                  <div className="flex justify-between border-b pb-1">
                    <span>Type</span>
                    <span className="font-medium">{instance.instance_type}</span>
                  </div>
                  <div className="flex justify-between border-b pb-1">
                    <span>VRAM</span>
                    <span>{instance.gpu_vram ? `${instance.gpu_vram} GB` : "-"}</span>
                  </div>
                  <div className="flex justify-between border-b pb-1">
                    <span>Rate</span>
                    <span>${instance.cost_per_hour ?? "-"}/hr</span>
                  </div>
                  <div className="flex justify-between border-b pb-1">
                    <span>Total Cost</span>
                    <span className="font-bold text-green-600">
                      ${instance.total_cost?.toFixed(4) ?? "0.0000"}
                    </span>
                  </div>
                  <div className="flex justify-between border-b pb-1">
                    <span>Status</span>
                    <Badge variant="outline">{instance.status}</Badge>
                  </div>
                  <div className="flex justify-between border-b pb-1">
                    <span>Created</span>
                    <span>
                      {formatDistanceToNow(parseISO(instance.created_at), {
                        addSuffix: true,
                      })}
                    </span>
                  </div>
                </div>
              </div>
            </div>
            <div>
              <h4 className="font-semibold mb-2 text-sm text-muted-foreground">
                Network
              </h4>
              <div className="p-3 bg-muted rounded-md font-mono text-sm flex justify-between items-center">
                <span>{instance.ip_address || "No Public IP"}</span>
                {instance.ip_address && <CopyButton text={instance.ip_address} />}
              </div>
            </div>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}


