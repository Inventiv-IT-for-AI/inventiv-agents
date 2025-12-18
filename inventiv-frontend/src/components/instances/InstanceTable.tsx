"use client";

import { formatDistanceToNow, parseISO } from "date-fns";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Eye, Archive, Wrench } from "lucide-react";
import { CopyButton } from "@/components/shared/CopyButton";
import type { Instance } from "@/lib/types";
import { displayOrDash, formatEur } from "@/lib/utils";
import { apiUrl } from "@/lib/api";
import type { LoadRangeResult } from "@/components/shared/VirtualizedRemoteList";
import { InventivDataTable, type DataTableSortState, type InventivDataTableColumn } from "@/components/shared/InventivDataTable";
import { useCallback, useMemo, useState } from "react";
type InstanceTableProps = {
  onViewDetails: (instance: Instance) => void;
  onTerminate: (id: string) => void;
  onReinstall: (id: string) => void;
  onArchive: (id: string) => void;
  refreshKey?: string;
};

export function InstanceTable({
  onViewDetails,
  onTerminate,
  onReinstall,
  onArchive,
  refreshKey,
}: InstanceTableProps) {
  const [sort, setSort] = useState<DataTableSortState>(null);

  type InstancesSearchResponse = {
    offset: number;
    limit: number;
    total_count: number;
    filtered_count: number;
    rows: Instance[];
  };

  const loadRange = useCallback(
    async (offset: number, limit: number): Promise<LoadRangeResult<Instance>> => {
      const params = new URLSearchParams();
      params.set("archived", "false");
      params.set("offset", String(offset));
      params.set("limit", String(limit));
      if (sort) {
        const by = (
          {
            provider: "provider",
            region: "region",
            zone: "zone",
            type: "type",
            cost: "total_cost",
            status: "status",
            created: "created_at",
          } as Record<string, string>
        )[sort.columnId];
        if (by) {
          params.set("sort_by", by);
          params.set("sort_dir", sort.direction);
        }
      }
      const res = await fetch(apiUrl(`instances/search?${params.toString()}`));
      if (!res.ok) throw new Error(`instances/search failed (${res.status})`);
      const data = (await res.json()) as InstancesSearchResponse;
      return { offset: data.offset, items: data.rows, totalCount: data.total_count, filteredCount: data.filtered_count };
    },
    [sort]
  );

  const columns = useMemo<InventivDataTableColumn<Instance>[]>(() => {
    return [
      {
        id: "id",
        label: "ID",
        width: 140,
        sortable: false,
        cell: ({ row }) => (
          <span className="font-mono text-xs">{row.id.split("-")[0]}...</span>
        ),
      },
      {
        id: "provider",
        label: "Provider",
        width: 140,
        sortable: true,
        cell: ({ row }) => displayOrDash(row.provider_name),
      },
      {
        id: "region",
        label: "Region",
        width: 160,
        sortable: true,
        cell: ({ row }) => displayOrDash(row.region),
      },
      {
        id: "zone",
        label: "Zone",
        width: 140,
        sortable: true,
        cell: ({ row }) => displayOrDash(row.zone),
      },
      {
        id: "type",
        label: "Type",
        width: 220,
        sortable: true,
        cell: ({ row }) => displayOrDash(row.instance_type),
      },
      {
        id: "storage",
        label: "Storage",
        width: 240,
        sortable: false,
        cell: ({ row }) => {
          const count = row.storage_count ?? 0;
          const sizes = (row.storage_sizes_gb ?? []).filter((n) => typeof n === "number" && n > 0);
          if (count <= 0) return <span className="text-muted-foreground">-</span>;
          const label = `${count} storages${sizes.length ? ` (${sizes.map((s) => `${s}GB`).join(", ")})` : ""}`;
          return <span className="font-medium">{label}</span>;
        },
      },
      {
        id: "cost",
        label: "Cost",
        width: 120,
        align: "right",
        sortable: true,
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
        sortable: false,
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
              title="Voir actions"
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
      reloadToken={refreshKey}
      autoHeight={true}
      height={300}
      rowHeight={56}
      columns={columns}
      loadRange={loadRange}
      sortState={sort}
      onSortChange={setSort}
      sortingMode="server"
      onRowClick={onViewDetails}
    />
  );
}


