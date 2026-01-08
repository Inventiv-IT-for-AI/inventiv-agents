"use client";

import { useEffect, useMemo, useState } from "react";
import type { OrganizationMember } from "@/lib/types";
import { apiUrl } from "@/lib/api";

import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";

import { useSnackbar } from "ia-widgets";

type OrgRole = "owner" | "admin" | "manager" | "user";

function parseRole(s: string | null | undefined): OrgRole | null {
  const v = (s ?? "").toLowerCase().trim();
  if (v === "owner" || v === "admin" || v === "manager" || v === "user") return v;
  return null;
}

function canAssignRole(actor: OrgRole, from: OrgRole, to: OrgRole): boolean {
  if (actor === "owner") return true;
  if (actor === "manager") return (from === "user" && to === "manager") || (from === "manager" && to === "user");
  if (actor === "admin") return (from === "user" && to === "admin") || (from === "admin" && to === "user");
  return false;
}

function canRemoveMember(actor: OrgRole, target: OrgRole, isSelf: boolean): boolean {
  if (isSelf) return true;
  if (actor === "owner") return true;
  if (actor === "admin") return target === "admin" || target === "user";
  if (actor === "manager") return target === "manager" || target === "user";
  return false;
}

export type OrganizationMembersDialogProps = {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  actorUserId: string | null;
  actorOrgRole: string | null | undefined;
};

export function OrganizationMembersDialog({ open, onOpenChange, actorUserId, actorOrgRole }: OrganizationMembersDialogProps) {
  const snackbar = useSnackbar();
  const actorRole = parseRole(actorOrgRole);

  const [members, setMembers] = useState<OrganizationMember[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const roleOptions: OrgRole[] = useMemo(() => ["owner", "admin", "manager", "user"], []);

  const fetchMembers = async () => {
    setError(null);
    setLoading(true);
    try {
      const res = await fetch(apiUrl("/organizations/current/members"));
      if (!res.ok) {
        const body = await res.json().catch(() => null);
        const code = body?.error || body?.message;
        const msg =
          code === "no_current_organization"
            ? "Aucune organisation sélectionnée"
            : code === "not_a_member"
              ? "Vous n’êtes pas membre de cette organisation"
              : "Impossible de charger les membres";
        setError(msg);
        setMembers([]);
        return;
      }
      const data = (await res.json()) as OrganizationMember[];
      setMembers(Array.isArray(data) ? data : []);
    } catch (e) {
      console.error(e);
      setError("Erreur réseau");
      setMembers([]);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (!open) return;
    void fetchMembers();
  }, [open]);

  const setRole = async (memberUserId: string, toRole: OrgRole) => {
    const m = members.find((x) => x.user_id === memberUserId);
    const fromRole = parseRole(m?.role);
    if (!actorRole || !fromRole) {
      snackbar.error("Rôle invalide", { title: "Organisation" });
      return;
    }
    if (!canAssignRole(actorRole, fromRole, toRole)) {
      snackbar.warning("Action non autorisée", { title: "Organisation" });
      return;
    }
    try {
      const res = await fetch(apiUrl(`/organizations/current/members/${memberUserId}`), {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ role: toRole }),
      });
      const body = await res.json().catch(() => null);
      if (!res.ok) {
        const code = body?.error || body?.message;
        const msg =
          code === "last_owner_cannot_be_changed"
            ? "Impossible: dernier Owner"
            : code === "role_change_not_allowed"
              ? "Changement de rôle non autorisé"
              : "Erreur lors du changement de rôle";
        snackbar.error(msg, { title: "Organisation", details: JSON.stringify({ status: res.status, body }, null, 2) });
        return;
      }
      setMembers((prev) => prev.map((x) => (x.user_id === memberUserId ? { ...x, role: toRole } : x)));
      snackbar.success("Rôle mis à jour", { title: "Organisation" });
    } catch (e) {
      console.error(e);
      snackbar.error("Erreur réseau", { title: "Organisation", details: e instanceof Error ? e.message : String(e) });
    }
  };

  const removeMember = async (memberUserId: string) => {
    const m = members.find((x) => x.user_id === memberUserId);
    const targetRole = parseRole(m?.role);
    const isSelf = !!actorUserId && actorUserId === memberUserId;
    if (!actorRole || !targetRole) {
      snackbar.error("Rôle invalide", { title: "Organisation" });
      return;
    }
    if (!canRemoveMember(actorRole, targetRole, isSelf)) {
      snackbar.warning("Action non autorisée", { title: "Organisation" });
      return;
    }
    try {
      const res = await fetch(apiUrl(`/organizations/current/members/${memberUserId}`), { method: "DELETE" });
      const body = await res.json().catch(() => null);
      if (!res.ok) {
        const code = body?.error || body?.message;
        const msg =
          code === "last_owner_cannot_be_removed"
            ? "Impossible: dernier Owner"
            : code === "member_remove_not_allowed"
              ? "Suppression non autorisée"
              : "Erreur lors de la suppression";
        snackbar.error(msg, { title: "Organisation", details: JSON.stringify({ status: res.status, body }, null, 2) });
        return;
      }
      if (isSelf) {
        snackbar.success("Vous avez quitté l’organisation", { title: "Organisation" });
        onOpenChange(false);
        return;
      }
      setMembers((prev) => prev.filter((x) => x.user_id !== memberUserId));
      snackbar.success("Membre retiré", { title: "Organisation" });
    } catch (e) {
      console.error(e);
      snackbar.error("Erreur réseau", { title: "Organisation", details: e instanceof Error ? e.message : String(e) });
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent showCloseButton={false} className="sm:max-w-[720px]">
        <DialogHeader>
          <DialogTitle>Membres & rôles</DialogTitle>
        </DialogHeader>

        {error ? <div className="text-sm text-red-600">{error}</div> : null}

        <div className="grid gap-2">
          {loading ? (
            <div className="text-sm text-muted-foreground">Chargement…</div>
          ) : members.length === 0 ? (
            <div className="text-sm text-muted-foreground">Aucun membre</div>
          ) : (
            <div className="border rounded-md overflow-hidden">
              <div className="grid grid-cols-12 gap-2 px-3 py-2 bg-muted text-xs font-medium">
                <div className="col-span-5">Utilisateur</div>
                <div className="col-span-3">Email</div>
                <div className="col-span-2">Rôle</div>
                <div className="col-span-2 text-right">Actions</div>
              </div>
              <div className="divide-y">
                {members.map((m) => {
                  const fromRole = parseRole(m.role) ?? "user";
                  const isSelf = !!actorUserId && actorUserId === m.user_id;
                  return (
                    <div key={m.user_id} className="grid grid-cols-12 gap-2 px-3 py-2 items-center">
                      <div className="col-span-5">
                        <div className="text-sm font-medium">
                          {m.first_name || m.last_name ? `${m.first_name ?? ""} ${m.last_name ?? ""}`.trim() : m.username}
                          {isSelf ? " (vous)" : ""}
                        </div>
                        <div className="text-xs text-muted-foreground">{m.username}</div>
                      </div>
                      <div className="col-span-3 text-sm truncate">{m.email}</div>
                      <div className="col-span-2">
                        <Select
                          value={fromRole}
                          onValueChange={(v) => void setRole(m.user_id, v as OrgRole)}
                          disabled={!actorRole}
                        >
                          <SelectTrigger>
                            <SelectValue />
                          </SelectTrigger>
                          <SelectContent>
                            {roleOptions.map((r) => {
                              const disabled = !actorRole ? true : !canAssignRole(actorRole, fromRole, r);
                              return (
                                <SelectItem key={r} value={r} disabled={disabled}>
                                  {r}
                                </SelectItem>
                              );
                            })}
                          </SelectContent>
                        </Select>
                      </div>
                      <div className="col-span-2 text-right">
                        <Button
                          variant={isSelf ? "outline" : "destructive"}
                          size="sm"
                          onClick={() => void removeMember(m.user_id)}
                          disabled={!actorRole || !canRemoveMember(actorRole, fromRole, isSelf)}
                        >
                          {isSelf ? "Quitter" : "Retirer"}
                        </Button>
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          )}
        </div>

        <DialogFooter className="sm:justify-between">
          <Button variant="outline" onClick={() => void fetchMembers()} disabled={loading}>
            Rafraîchir
          </Button>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Fermer
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}


