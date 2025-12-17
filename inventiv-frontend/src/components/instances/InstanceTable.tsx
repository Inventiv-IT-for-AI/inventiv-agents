"use client";

import { formatDistanceToNow, parseISO } from "date-fns";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Eye, Archive, Wrench } from "lucide-react";
import { CopyButton } from "@/components/shared/CopyButton";
import type { Instance } from "@/lib/types";
import { displayOrDash, formatEur } from "@/lib/utils";
import { InventivDataTable, type InventivDataTableColumn } from "@/components/shared/InventivDataTable";
import { useMemo } from "react";
type InstanceTableProps = {
  instances: Instance[];
  onViewDetails: (instance: Instance) => void;
  onTerminate: (id: string) => void;
  onReinstall: (id: string) => void;
  onArchive: (id: string) => void;
};

export function InstanceTable({
  instances,
  onViewDetails,
  onTerminate,
  onReinstall,
  onArchive,
}: InstanceTableProps) {
  
  const columns = useMemo<InventivDataTableColumn<Instance>[]>(() => {
    return [
      {
        id: "id",
        label: "ID",
        width: 140,
        sortable: true,
        getSortValue: (r) => r.id,
        cell: ({ row }) => (
          <span className="font-mono text-xs">{row.id.split("-")[0]}...</span>
        ),
      },
      {
        id: "provider",
        label: "Provider",
        width: 140,
        sortable: true,
        getSortValue: (r) => r.provider_name ?? "",
        cell: ({ row }) => displayOrDash(row.provider_name),
      },
      {
        id: "region",
        label: "Region",
        width: 160,
        sortable: true,
        getSortValue: (r) => r.region ?? "",
        cell: ({ row }) => displayOrDash(row.region),
      },
      {
        id: "zone",
        label: "Zone",
        width: 140,
        sortable: true,
        getSortValue: (r) => r.zone ?? "",
        cell: ({ row }) => displayOrDash(row.zone),
      },
      {
        id: "type",
        label: "Type",
        width: 220,
        sortable: true,
        getSortValue: (r) => r.instance_type ?? "",
        cell: ({ row }) => displayOrDash(row.instance_type),
      },
      {
        id: "cost",
        label: "Cost",
        width: 120,
        align: "right",
        sortable: true,
        getSortValue: (r) => (typeof r.total_cost === "number" ? r.total_cost : null),
        cell: ({ row }) => (
          <span className="font-mono">
            {typeof row.total_cost === "number" ? formatEur(row.total_cost, { minFrac: 4, maxFrac: 4 }) : "-"}
          </span>
        ),
      },
      {
        id: "status",
        label: "Status",
        width: 140,
        sortable: true,
        getSortValue: (r) => r.status ?? "",
        cell: ({ row }) => (
          <Badge
            variant={
              row.status.toLowerCase() === "ready"
                ? "default"
                : row.status.toLowerCase() === "terminated"
                  ? "destructive"
                  : "secondary"
            }
          >
            {row.status}
          </Badge>
        ),
      },
      {
        id: "created",
        label: "Created",
        width: 170,
        sortable: true,
        getSortValue: (r) => new Date(r.created_at),
        cell: ({ row }) => (
          <span className="whitespace-nowrap text-muted-foreground">
            {formatDistanceToNow(parseISO(row.created_at), { addSuffix: true })}
          </span>
        ),
      },
      {
        id: "ip",
        label: "IP Address",
        width: 200,
        sortable: true,
        getSortValue: (r) => r.ip_address ?? "",
        cell: ({ row }) =>
          row.ip_address ? (
            <div className="flex items-center gap-1 font-mono text-sm">
              <span className="truncate">{row.ip_address}</span>
              <CopyButton text={row.ip_address} />
            </div>
          ) : (
            <span className="text-muted-foreground">-</span>
          ),
      },
      {
        id: "actions",
        label: "Actions",
        width: 280,
        align: "right",
        disableReorder: true,
        sortable: false,
        cell: ({ row }) => (
          <div className="flex justify-end items-center gap-2">
            <Button
              variant="ghost"
              size="icon"
              onClick={(e) => {
                e.stopPropagation();
                onViewDetails(row);
              }}
              title="Voir détails"
            >
              <Eye className="h-4 w-4" />
            </Button>
            {row.status.toLowerCase() !== "terminated" ? (
              <>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={(e) => {
                    e.stopPropagation();
                    onReinstall(row.id);
                  }}
                  disabled={!row.ip_address || ["terminating"].includes(row.status.toLowerCase())}
                  title={!row.ip_address ? "IP manquante" : "Réinstaller le worker"}
                >
                  <Wrench className="h-4 w-4 mr-2" />
                  Reinstall
                </Button>
                <Button
                  variant="destructive"
                  size="sm"
                  onClick={(e) => {
                    e.stopPropagation();
                    onTerminate(row.id);
                  }}
                >
                  Terminer
                </Button>
              </>
            ) : (
              <Button
                variant="secondary"
                size="icon"
                onClick={(e) => {
                  e.stopPropagation();
                  onArchive(row.id);
                }}
                title="Archive"
              >
                <Archive className="h-4 w-4" />
              </Button>
            )}
          </div>
        ),
      },
    ];
  }, [onArchive, onReinstall, onTerminate, onViewDetails]);

  return (
    <InventivDataTable<Instance>
      listId="instances:table"
      title="Instances"
      autoHeight={true}
      height={300}
      rowHeight={56}
      columns={columns}
      rows={instances}
      getRowKey={(r) => r.id}
      onRowClick={onViewDetails}
    />
  );
}


