"use client";

import { formatDistanceToNow, parseISO } from "date-fns";
import { Button } from "@/components/ui/button";
import { Table, TableHeader, TableRow, TableHead, TableBody, TableCell } from "@/components/ui/table";
import { Badge } from "@/components/ui/badge";
import { Eye, Archive } from "lucide-react";
import { CopyButton } from "@/components/shared/CopyButton";
import type { Instance } from "@/lib/types";

type InstanceTableProps = {
  instances: Instance[];
  onViewDetails: (instance: Instance) => void;
  onTerminate: (id: string) => void;
  onArchive: (id: string) => void;
};

export function InstanceTable({
  instances,
  onViewDetails,
  onTerminate,
  onArchive,
}: InstanceTableProps) {
  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>ID</TableHead>
          <TableHead>Provider</TableHead>
          <TableHead>Region</TableHead>
          <TableHead>Zone</TableHead>
          <TableHead>Type</TableHead>
          <TableHead>Cost</TableHead>
          <TableHead>Status</TableHead>
          <TableHead>Created</TableHead>
          <TableHead>IP Address</TableHead>
          <TableHead className="text-right">Actions</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {instances.map((instance) => (
          <TableRow key={instance.id}>
            <TableCell className="font-mono text-xs">
              {instance.id.split("-")[0]}...
            </TableCell>
            <TableCell>{instance.provider_name}</TableCell>
            <TableCell>{instance.region}</TableCell>
            <TableCell>{instance.zone}</TableCell>
            <TableCell>{instance.instance_type}</TableCell>
            <TableCell className="font-mono">
              {instance.total_cost !== undefined
                ? `$${instance.total_cost.toFixed(4)}`
                : "-"}
            </TableCell>
            <TableCell>
              <Badge
                variant={
                  instance.status.toLowerCase() === "ready"
                    ? "default"
                    : instance.status.toLowerCase() === "terminated"
                      ? "destructive"
                      : "secondary"
                }
              >
                {instance.status}
              </Badge>
            </TableCell>
            <TableCell className="whitespace-nowrap text-muted-foreground">
              {formatDistanceToNow(parseISO(instance.created_at), {
                addSuffix: true,
              })}
            </TableCell>
            <TableCell className="font-mono text-sm">
              {instance.ip_address ? (
                <div className="flex items-center">
                  {instance.ip_address}
                  <CopyButton text={instance.ip_address} />
                </div>
              ) : (
                <span className="text-muted-foreground">-</span>
              )}
            </TableCell>
            <TableCell className="text-right">
              <div className="flex justify-end space-x-2">
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={() => onViewDetails(instance)}
                >
                  <Eye className="h-4 w-4" />
                </Button>
                {instance.status.toLowerCase() !== "terminated" && (
                  <Button
                    variant="destructive"
                    size="sm"
                    onClick={() => onTerminate(instance.id)}
                  >
                    Terminate
                  </Button>
                )}
                {instance.status.toLowerCase() === "terminated" && (
                  <Button
                    variant="secondary"
                    size="icon"
                    onClick={() => onArchive(instance.id)}
                    title="Archive"
                  >
                    <Archive className="h-4 w-4" />
                  </Button>
                )}
              </div>
            </TableCell>
          </TableRow>
        ))}
        {instances.length === 0 && (
          <TableRow>
            <TableCell colSpan={10} className="text-center h-24 text-muted-foreground">
              No instances found.
            </TableCell>
          </TableRow>
        )}
      </TableBody>
    </Table>
  );
}


