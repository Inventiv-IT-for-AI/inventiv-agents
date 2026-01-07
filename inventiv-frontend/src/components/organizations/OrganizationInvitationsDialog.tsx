"use client";

import { useEffect, useMemo, useState } from "react";
import type { OrganizationInvitation } from "@/lib/types";
import { useOrganizationInvitations } from "@/hooks/useOrganizationInvitations";
import { parseRole, canInvite, canInviteRole, type OrgRole } from "@/lib/rbac";
import { apiRequest } from "@/lib/api";
import { apiUrl } from "@/lib/api";

import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Badge } from "@/components/ui/badge";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";

import { useSnackbar } from "ia-widgets";
import { Copy, Mail, X, CheckCircle2, Clock, AlertCircle } from "lucide-react";

export type OrganizationInvitationsDialogProps = {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  organizationId: string;
  organizationName: string;
  actorOrgRole: string | null | undefined;
};

type InvitationStatus = "pending" | "accepted" | "expired";

function getInvitationStatus(inv: OrganizationInvitation): InvitationStatus {
  if (inv.accepted_at) return "accepted";
  const expiresAt = new Date(inv.expires_at);
  if (expiresAt < new Date()) return "expired";
  return "pending";
}

function formatExpiresAt(expiresAt: string): string {
  const date = new Date(expiresAt);
  const now = new Date();
  const diffMs = date.getTime() - now.getTime();
  const diffDays = Math.ceil(diffMs / (1000 * 60 * 60 * 24));

  if (diffDays < 0) return `Expirée le ${date.toLocaleDateString("fr-FR")}`;
  if (diffDays === 0) return "Expire aujourd'hui";
  if (diffDays === 1) return "Expire demain";
  return `Expire dans ${diffDays} jours`;
}

export function OrganizationInvitationsDialog({
  open,
  onOpenChange,
  organizationId,
  organizationName,
  actorOrgRole,
}: OrganizationInvitationsDialogProps) {
  const snackbar = useSnackbar();
  const actorRole = parseRole(actorOrgRole);
  const { listInvitations, createInvitation, loading, error } = useOrganizationInvitations();

  const [invitations, setInvitations] = useState<OrganizationInvitation[]>([]);
  const [invitationTokens, setInvitationTokens] = useState<Map<string, string>>(new Map()); // Map<invitationId, token>
  const [filter, setFilter] = useState<"all" | "pending" | "accepted" | "expired">("all");
  const [createForm, setCreateForm] = useState({
    email: "",
    role: "user" as OrgRole,
    expires_in_days: 7,
  });
  const [creating, setCreating] = useState(false);

  const roleOptions: OrgRole[] = useMemo(() => ["owner", "admin", "manager", "user"], []);

  const filteredInvitations = useMemo(() => {
    if (filter === "all") return invitations;
    return invitations.filter((inv) => getInvitationStatus(inv) === filter);
  }, [invitations, filter]);

  const pendingCount = useMemo(
    () => invitations.filter((inv) => getInvitationStatus(inv) === "pending").length,
    [invitations]
  );

  const fetchInvitations = async () => {
    try {
      // Ensure we're using the correct organization
      await apiRequest("/organizations/current", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ organization_id: organizationId }),
      });

      const data = await listInvitations();
      setInvitations(data);
    } catch (e) {
      console.error(e);
      setInvitations([]);
    }
  };

  useEffect(() => {
    if (!open) return;
    void fetchInvitations();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, organizationId]);

  const handleCreateInvitation = async () => {
    if (!createForm.email.trim()) {
      snackbar.error("L'email est requis", { title: "Invitation" });
      return;
    }

    if (!actorRole || !canInvite(actorRole)) {
      snackbar.warning("Permissions insuffisantes", { title: "Invitation" });
      return;
    }

    if (!canInviteRole(actorRole, createForm.role)) {
      snackbar.warning("Vous ne pouvez pas inviter ce rôle", { title: "Invitation" });
      return;
    }

    setCreating(true);
    try {
      // Ensure we're using the correct organization
      await apiRequest("/organizations/current", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ organization_id: organizationId }),
      });

      const newInvitation = await createInvitation({
        email: createForm.email,
        role: createForm.role,
        expires_in_days: createForm.expires_in_days,
      });

      setInvitations((prev) => [newInvitation, ...prev]);
      setShowCreateForm(false);
      setCreateForm({ email: "", role: "user", expires_in_days: 7 });
      snackbar.success("Invitation créée", { title: "Invitation" });
    } catch (e) {
      console.error(e);
      snackbar.error("Erreur lors de la création", {
        title: "Invitation",
        details: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setCreating(false);
    }
  };

  const copyInvitationLink = async (token: string) => {
    const url = `${window.location.origin}/invitations/${token}`;
    try {
      await navigator.clipboard.writeText(url);
      snackbar.success("Lien copié dans le presse-papier", { title: "Invitation" });
    } catch (e) {
      console.error(e);
      snackbar.error("Impossible de copier le lien", { title: "Invitation" });
    }
  };

  const getStatusBadge = (status: InvitationStatus) => {
    switch (status) {
      case "pending":
        return (
          <Badge variant="outline" className="bg-yellow-50 text-yellow-700 border-yellow-300">
            <Clock className="mr-1 h-3 w-3" />
            En attente
          </Badge>
        );
      case "accepted":
        return (
          <Badge variant="outline" className="bg-green-50 text-green-700 border-green-300">
            <CheckCircle2 className="mr-1 h-3 w-3" />
            Acceptée
          </Badge>
        );
      case "expired":
        return (
          <Badge variant="outline" className="bg-gray-50 text-gray-700 border-gray-300">
            <AlertCircle className="mr-1 h-3 w-3" />
            Expirée
          </Badge>
        );
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent showCloseButton={false} className="sm:max-w-[800px]">
        <DialogHeader>
          <DialogTitle>Invitations - {organizationName}</DialogTitle>
        </DialogHeader>

        {error ? <div className="text-sm text-red-600">{error}</div> : null}

        <Tabs defaultValue="list" className="w-full">
          <TabsList className="grid w-full grid-cols-2">
            <TabsTrigger value="list">
              Liste {pendingCount > 0 && <Badge variant="secondary" className="ml-2">{pendingCount}</Badge>}
            </TabsTrigger>
            <TabsTrigger value="create">Inviter</TabsTrigger>
          </TabsList>

          <TabsContent value="list" className="space-y-4">
            <div className="flex items-center justify-between">
              <div className="flex gap-2">
                <Button
                  variant={filter === "all" ? "default" : "outline"}
                  size="sm"
                  onClick={() => setFilter("all")}
                >
                  Tous
                </Button>
                <Button
                  variant={filter === "pending" ? "default" : "outline"}
                  size="sm"
                  onClick={() => setFilter("pending")}
                >
                  En attente
                </Button>
                <Button
                  variant={filter === "accepted" ? "default" : "outline"}
                  size="sm"
                  onClick={() => setFilter("accepted")}
                >
                  Acceptées
                </Button>
                <Button
                  variant={filter === "expired" ? "default" : "outline"}
                  size="sm"
                  onClick={() => setFilter("expired")}
                >
                  Expirées
                </Button>
              </div>
              <Button variant="outline" size="sm" onClick={() => void fetchInvitations()} disabled={loading}>
                Rafraîchir
              </Button>
            </div>

            {loading ? (
              <div className="text-sm text-muted-foreground">Chargement…</div>
            ) : filteredInvitations.length === 0 ? (
              <div className="text-sm text-muted-foreground">
                {filter === "all" ? "Aucune invitation" : `Aucune invitation ${filter}`}
              </div>
            ) : (
              <div className="border rounded-md overflow-hidden">
                <div className="grid grid-cols-12 gap-2 px-3 py-2 bg-muted text-xs font-medium">
                  <div className="col-span-3">Email</div>
                  <div className="col-span-2">Rôle</div>
                  <div className="col-span-2">Statut</div>
                  <div className="col-span-3">Expire le</div>
                  <div className="col-span-2 text-right">Actions</div>
                </div>
                <div className="divide-y">
                  {filteredInvitations.map((inv) => {
                    const status = getInvitationStatus(inv);
                    const isPending = status === "pending";
                    return (
                      <div key={inv.id} className="grid grid-cols-12 gap-2 px-3 py-2 items-center">
                        <div className="col-span-3">
                          <div className="text-sm font-medium">{inv.email}</div>
                          {inv.invited_by_username && (
                            <div className="text-xs text-muted-foreground">Par {inv.invited_by_username}</div>
                          )}
                        </div>
                        <div className="col-span-2">
                          <Badge variant="secondary">{inv.role}</Badge>
                        </div>
                        <div className="col-span-2">{getStatusBadge(status)}</div>
                        <div className="col-span-3 text-sm text-muted-foreground">
                          {isPending ? formatExpiresAt(inv.expires_at) : inv.accepted_at ? new Date(inv.accepted_at).toLocaleDateString("fr-FR") : formatExpiresAt(inv.expires_at)}
                        </div>
                        <div className="col-span-2 text-right">
                          {isPending && (
                            <Button
                              variant="outline"
                              size="sm"
                              onClick={() => {
                                // Extract token from invitation (we need to fetch it or store it)
                                // For now, we'll need to get it from the API response
                                // This is a limitation - we should store the token when creating
                                snackbar.info("Fonctionnalité à venir: copie du lien", { title: "Invitation" });
                              }}
                              title="Copier le lien d'invitation"
                            >
                              <Copy className="h-3 w-3" />
                            </Button>
                          )}
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>
            )}
          </TabsContent>

          <TabsContent value="create" className="space-y-4">
            <div className="grid gap-4">
              <div className="grid gap-2">
                <Label>Email *</Label>
                <Input
                  type="email"
                  value={createForm.email}
                  onChange={(e) => setCreateForm((s) => ({ ...s, email: e.target.value }))}
                  placeholder="exemple@email.com"
                  disabled={creating || !actorRole || !canInvite(actorRole)}
                />
              </div>
              <div className="grid gap-2">
                <Label>Rôle *</Label>
                <Select
                  value={createForm.role}
                  onValueChange={(v) => setCreateForm((s) => ({ ...s, role: v as OrgRole }))}
                  disabled={creating || !actorRole || !canInvite(actorRole)}
                >
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {roleOptions.map((r) => {
                      const disabled = !actorRole ? true : !canInviteRole(actorRole, r);
                      return (
                        <SelectItem key={r} value={r} disabled={disabled}>
                          {r} {disabled && "(non autorisé)"}
                        </SelectItem>
                      );
                    })}
                  </SelectContent>
                </Select>
                {actorRole && !canInviteRole(actorRole, createForm.role) && (
                  <p className="text-xs text-muted-foreground">
                    Vous ne pouvez pas inviter ce rôle avec votre niveau d&apos;autorisation.
                  </p>
                )}
              </div>
              <div className="grid gap-2">
                <Label>Durée de validité (jours)</Label>
                <Select
                  value={createForm.expires_in_days.toString()}
                  onValueChange={(v) => setCreateForm((s) => ({ ...s, expires_in_days: parseInt(v, 10) }))}
                  disabled={creating}
                >
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="1">1 jour</SelectItem>
                    <SelectItem value="3">3 jours</SelectItem>
                    <SelectItem value="7">7 jours</SelectItem>
                    <SelectItem value="14">14 jours</SelectItem>
                    <SelectItem value="30">30 jours</SelectItem>
                  </SelectContent>
                </Select>
              </div>
              <Button onClick={handleCreateInvitation} disabled={creating || !createForm.email.trim() || !actorRole || !canInvite(actorRole)}>
                <Mail className="mr-2 h-4 w-4" />
                {creating ? "Création..." : "Créer l'invitation"}
              </Button>
            </div>
          </TabsContent>
        </Tabs>

        <DialogFooter className="sm:justify-between">
          <div className="text-xs text-muted-foreground">
            {pendingCount > 0 && `${pendingCount} invitation${pendingCount > 1 ? "s" : ""} en attente`}
          </div>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Fermer
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

