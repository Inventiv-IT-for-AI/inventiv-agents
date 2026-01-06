import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useMemo, useState } from "react";
import type { InstanceStorageInfo } from "@/lib/types";
import { displayOrDash } from "@/lib/utils";
import { formatDistanceToNow } from "date-fns";
import { fr } from "date-fns/locale";
import { HardDrive, Trash2, CheckCircle2, Clock, AlertCircle } from "lucide-react";

interface InstanceVolumesHistoryProps {
  storages?: InstanceStorageInfo[];
}

type FilterType = "all" | "active" | "deleted" | "reconciled";

function getStatusBadge(status: string, deletedAt: string | null | undefined, reconciledAt: string | null | undefined) {
  if (reconciledAt) {
    return (
      <Badge variant="outline" className="text-[10px] bg-muted text-muted-foreground">
        <CheckCircle2 className="h-3 w-3 mr-1" />
        Réconcilié
      </Badge>
    );
  }
  if (deletedAt) {
    return (
      <Badge variant="outline" className="text-[10px] bg-orange-50 text-orange-700 border-orange-200">
        <Trash2 className="h-3 w-3 mr-1" />
        Supprimé
      </Badge>
    );
  }
  if (status === "deleting") {
    return (
      <Badge variant="outline" className="text-[10px] bg-yellow-50 text-yellow-700 border-yellow-200">
        <Clock className="h-3 w-3 mr-1" />
        En suppression
      </Badge>
    );
  }
  return (
    <Badge variant="outline" className="text-[10px] bg-green-50 text-green-700 border-green-200">
      <HardDrive className="h-3 w-3 mr-1" />
      Actif
    </Badge>
  );
}

function formatTimestamp(ts: string | null | undefined): string {
  if (!ts) return "-";
  try {
    return formatDistanceToNow(new Date(ts), { addSuffix: true, locale: fr });
  } catch {
    return "-";
  }
}

function formatFullTimestamp(ts: string | null | undefined): string {
  if (!ts) return "-";
  try {
    return new Date(ts).toLocaleString("fr-FR", {
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
  } catch {
    return "-";
  }
}

export function InstanceVolumesHistory({ storages = [] }: InstanceVolumesHistoryProps) {
  const [filter, setFilter] = useState<FilterType>("all");

  const filteredStorages = useMemo(() => {
    if (filter === "all") return storages;
    if (filter === "active") return storages.filter((s) => !s.deleted_at);
    if (filter === "deleted") return storages.filter((s) => s.deleted_at && !s.reconciled_at);
    if (filter === "reconciled") return storages.filter((s) => s.reconciled_at);
    return storages;
  }, [storages, filter]);

  const activeCount = storages.filter((s) => !s.deleted_at).length;
  const deletedCount = storages.filter((s) => s.deleted_at && !s.reconciled_at).length;
  const reconciledCount = storages.filter((s) => s.reconciled_at).length;

  if (storages.length === 0) {
    return (
      <Card>
        <CardHeader>
          <CardTitle className="text-sm">Historique des Volumes</CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-sm text-muted-foreground">Aucun volume trouvé pour cette instance.</p>
        </CardContent>
      </Card>
    );
  }

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center justify-between">
          <CardTitle className="text-sm">Historique des Volumes</CardTitle>
          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            <span>{activeCount} actif{activeCount > 1 ? "s" : ""}</span>
            {deletedCount > 0 && <span>• {deletedCount} supprimé{deletedCount > 1 ? "s" : ""}</span>}
            {reconciledCount > 0 && <span>• {reconciledCount} réconcilié{reconciledCount > 1 ? "s" : ""}</span>}
          </div>
        </div>
        <div className="flex items-center gap-2 mt-2">
          <Tabs value={filter} onValueChange={(v) => setFilter(v as FilterType)} className="w-full">
            <TabsList className="h-8">
              <TabsTrigger value="all" className="text-[10px] px-2">Tous</TabsTrigger>
              <TabsTrigger value="active" className="text-[10px] px-2">Actifs</TabsTrigger>
              <TabsTrigger value="deleted" className="text-[10px] px-2">Supprimés</TabsTrigger>
              <TabsTrigger value="reconciled" className="text-[10px] px-2">Réconciliés</TabsTrigger>
            </TabsList>
          </Tabs>
        </div>
      </CardHeader>
      <CardContent>
        <div className="space-y-3">
          {filteredStorages.length === 0 ? (
            <p className="text-sm text-muted-foreground text-center py-4">Aucun volume correspondant au filtre sélectionné.</p>
          ) : (
            filteredStorages.map((storage) => (
              <div
                key={storage.id}
                className="border rounded-lg p-3 space-y-2 bg-card hover:bg-accent/50 transition-colors"
              >
                <div className="flex items-start justify-between gap-2">
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 mb-1">
                      <span className="font-medium text-sm truncate">{storage.name || storage.provider_volume_id}</span>
                      {storage.is_boot && (
                        <Badge variant="outline" className="text-[9px]">
                          Boot
                        </Badge>
                      )}
                      {getStatusBadge(storage.status, storage.deleted_at, storage.reconciled_at)}
                    </div>
                    <div className="text-xs text-muted-foreground space-y-1">
                      <div className="flex items-center gap-2">
                        <span className="font-medium">Type:</span>
                        <span className="font-mono">{storage.volume_type}</span>
                        {storage.size_gb && (
                          <>
                            <span>•</span>
                            <span className="font-medium">Taille:</span>
                            <span>{storage.size_gb} GB</span>
                          </>
                        )}
                      </div>
                      <div className="flex items-center gap-2">
                        <span className="font-medium">ID:</span>
                        <span className="font-mono text-[10px] truncate">{storage.provider_volume_id}</span>
                      </div>
                    </div>
                  </div>
                </div>

                <div className="grid grid-cols-2 gap-x-4 gap-y-1 text-xs text-muted-foreground pt-2 border-t">
                  <div>
                    <span className="font-medium">Créé:</span>{" "}
                    <span title={formatFullTimestamp(storage.created_at)}>{formatTimestamp(storage.created_at)}</span>
                  </div>
                  {storage.attached_at && (
                    <div>
                      <span className="font-medium">Attaché:</span>{" "}
                      <span title={formatFullTimestamp(storage.attached_at)}>{formatTimestamp(storage.attached_at)}</span>
                    </div>
                  )}
                  {storage.deleted_at && (
                    <div>
                      <span className="font-medium">Supprimé:</span>{" "}
                      <span title={formatFullTimestamp(storage.deleted_at)}>{formatTimestamp(storage.deleted_at)}</span>
                    </div>
                  )}
                  {storage.reconciled_at && (
                    <div>
                      <span className="font-medium">Réconcilié:</span>{" "}
                      <span title={formatFullTimestamp(storage.reconciled_at)}>{formatTimestamp(storage.reconciled_at)}</span>
                    </div>
                  )}
                  {storage.last_reconciliation && !storage.reconciled_at && (
                    <div>
                      <span className="font-medium">Dernière réconciliation:</span>{" "}
                      <span title={formatFullTimestamp(storage.last_reconciliation)}>{formatTimestamp(storage.last_reconciliation)}</span>
                    </div>
                  )}
                  {storage.delete_on_terminate && (
                    <div className="col-span-2">
                      <span className="font-medium">Suppression automatique:</span> <span>Oui</span>
                    </div>
                  )}
                  {storage.error_message && (
                    <div className="col-span-2 flex items-start gap-1 text-red-600">
                      <AlertCircle className="h-3 w-3 mt-0.5 flex-shrink-0" />
                      <span className="text-[10px]">{storage.error_message}</span>
                    </div>
                  )}
                </div>
              </div>
            ))
          )}
        </div>
      </CardContent>
    </Card>
  );
}

