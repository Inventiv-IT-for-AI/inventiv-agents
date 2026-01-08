"use client";

import { useCallback, useEffect, useState } from "react";
import { apiUrl } from "@/lib/api";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useSnackbar } from "ia-widgets";
import { IADataTable } from "ia-widgets";
import type { ColumnDef } from "@tanstack/react-table";

export type Session = {
  session_id: string;
  current_organization_id?: string | null;
  current_organization_name?: string | null;
  organization_role?: string | null;
  ip_address?: string | null;
  user_agent?: string | null;
  created_at: string;
  last_used_at: string;
  expires_at: string;
  is_current: boolean;
};

export type SessionsDialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

export function SessionsDialog({ open, onOpenChange }: SessionsDialogProps) {
  const { showSnackbar } = useSnackbar();
  const [sessions, setSessions] = useState<Session[]>([]);
  const [loading, setLoading] = useState(false);
  const [revoking, setRevoking] = useState<string | null>(null);

  const fetchSessions = useCallback(async () => {
    setLoading(true);
    try {
      const res = await fetch(apiUrl("/auth/sessions"));
      if (!res.ok) {
        if (res.status === 401) {
          showSnackbar("Session expirée", "error");
          onOpenChange(false);
          return;
        }
        showSnackbar("Erreur lors du chargement des sessions", "error");
        return;
      }
      const data = (await res.json()) as Session[];
      setSessions(Array.isArray(data) ? data : []);
    } catch (e) {
      console.error(e);
      showSnackbar("Erreur réseau", "error");
      setSessions([]);
    } finally {
      setLoading(false);
    }
  }, [showSnackbar, onOpenChange]);

  useEffect(() => {
    if (open) {
      void fetchSessions();
    }
  }, [open, fetchSessions]);

  const revokeSession = useCallback(
    async (sessionId: string) => {
      if (!confirm("Êtes-vous sûr de vouloir révoquer cette session ?")) {
        return;
      }

      setRevoking(sessionId);
      try {
        const res = await fetch(apiUrl(`/auth/sessions/${sessionId}/revoke`), {
          method: "POST",
        });
        if (!res.ok) {
          const error = await res.json().catch(() => ({ error: "Erreur inconnue" }));
          showSnackbar(
            error.message || "Erreur lors de la révocation de la session",
            "error"
          );
          return;
        }
        showSnackbar("Session révoquée avec succès", "success");
        await fetchSessions();
      } catch (e) {
        console.error(e);
        showSnackbar("Erreur réseau", "error");
      } finally {
        setRevoking(null);
      }
    },
    [fetchSessions, showSnackbar]
  );

  const formatDate = (dateStr: string) => {
    try {
      return new Date(dateStr).toLocaleString("fr-FR", {
        dateStyle: "short",
        timeStyle: "short",
      });
    } catch {
      return dateStr;
    }
  };

  const formatUserAgent = (ua: string | null | undefined) => {
    if (!ua) return "Inconnu";
    // Extract browser name from user agent
    if (ua.includes("Chrome")) return "Chrome";
    if (ua.includes("Firefox")) return "Firefox";
    if (ua.includes("Safari")) return "Safari";
    if (ua.includes("Edge")) return "Edge";
    return ua.substring(0, 50);
  };

  const columns: ColumnDef<Session>[] = [
    {
      accessorKey: "is_current",
      header: "Statut",
      cell: ({ row }) => {
        const isCurrent = row.getValue("is_current") as boolean;
        return isCurrent ? (
          <span className="text-xs font-semibold text-green-600">Session courante</span>
        ) : (
          <span className="text-xs text-muted-foreground">Active</span>
        );
      },
    },
    {
      accessorKey: "current_organization_name",
      header: "Organisation",
      cell: ({ row }) => {
        const orgName = row.getValue("current_organization_name") as string | null;
        const orgRole = row.original.organization_role;
        if (!orgName) return <span className="text-muted-foreground">Personal</span>;
        return (
          <div>
            <div className="font-medium">{orgName}</div>
            {orgRole && (
              <div className="text-xs text-muted-foreground">{orgRole}</div>
            )}
          </div>
        );
      },
    },
    {
      accessorKey: "ip_address",
      header: "IP",
      cell: ({ row }) => {
        const ip = row.getValue("ip_address") as string | null;
        return <span className="font-mono text-sm">{ip || "N/A"}</span>;
      },
    },
    {
      accessorKey: "user_agent",
      header: "Navigateur",
      cell: ({ row }) => {
        const ua = row.getValue("user_agent") as string | null;
        return <span className="text-sm">{formatUserAgent(ua)}</span>;
      },
    },
    {
      accessorKey: "last_used_at",
      header: "Dernière utilisation",
      cell: ({ row }) => {
        const date = row.getValue("last_used_at") as string;
        return <span className="text-sm">{formatDate(date)}</span>;
      },
    },
    {
      accessorKey: "expires_at",
      header: "Expire le",
      cell: ({ row }) => {
        const date = row.getValue("expires_at") as string;
        const expiresAt = new Date(date);
        const now = new Date();
        const isExpired = expiresAt < now;
        return (
          <span className={`text-sm ${isExpired ? "text-red-600" : ""}`}>
            {formatDate(date)}
          </span>
        );
      },
    },
    {
      id: "actions",
      header: "Actions",
      cell: ({ row }) => {
        const session = row.original;
        const isCurrent = session.is_current;
        const isRevoking = revoking === session.session_id;

        return (
          <Button
            variant="destructive"
            size="sm"
            disabled={isCurrent || isRevoking}
            onClick={() => revokeSession(session.session_id)}
          >
            {isRevoking ? "Révoquation..." : "Révoquer"}
          </Button>
        );
      },
    },
  ];

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-5xl max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>Sessions actives</DialogTitle>
          <DialogDescription>
            Gérez vos sessions actives. Vous pouvez révoquer les sessions sur d&apos;autres appareils.
          </DialogDescription>
        </DialogHeader>

        <div className="mt-4">
          {loading ? (
            <div className="text-center py-8 text-muted-foreground">
              Chargement des sessions...
            </div>
          ) : sessions.length === 0 ? (
            <div className="text-center py-8 text-muted-foreground">
              Aucune session active
            </div>
          ) : (
            <IADataTable columns={columns} data={sessions} />
          )}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Fermer
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

