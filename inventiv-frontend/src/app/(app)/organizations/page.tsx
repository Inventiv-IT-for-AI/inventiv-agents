"use client";

import { useCallback, useEffect, useMemo, useState, type ChangeEvent } from "react";
import { apiRequest } from "@/lib/api";
import type { Organization } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { IADataTable, type DataTableSortState, type IADataTableColumn, type LoadRangeResult } from "ia-widgets";
import { IAAlert, IAAlertDescription, IAAlertTitle } from "ia-designsys";
import { useSnackbar } from "ia-widgets";
import { Building2, Users, CheckCircle2 } from "lucide-react";
import { OrganizationMembersDialog } from "@/components/organizations/OrganizationMembersDialog";

type OrganizationWithActions = Organization & {
  canManage?: boolean; // owner, admin, or manager
  canEdit?: boolean; // owner or admin
};

export default function OrganizationsPage() {
  const snackbar = useSnackbar();
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [refreshTick, setRefreshTick] = useState(0);
  const [sort, setSort] = useState<DataTableSortState>(null);

  const [createOpen, setCreateOpen] = useState(false);
  const [membersOpen, setMembersOpen] = useState(false);
  const [selectedOrg, setSelectedOrg] = useState<Organization | null>(null);
  const [currentOrgId, setCurrentOrgId] = useState<string | null>(null);

  const [form, setForm] = useState({
    name: "",
    slug: "",
  });

  // Fetch current user info to get current organization
  useEffect(() => {
    const fetchMe = async () => {
      try {
        const res = await apiRequest("/auth/me");
        if (res.ok) {
          const data = await res.json();
          setCurrentOrgId(data.current_organization_id || null);
        }
      } catch (e) {
        console.error("Failed to fetch current user:", e);
      }
    };
    void fetchMe();
  }, [refreshTick]);

  const loadRange = useCallback(
    async (offset: number, limit: number): Promise<LoadRangeResult<OrganizationWithActions>> => {
      try {
        const res = await apiRequest("/organizations");
        if (!res.ok) {
          throw new Error(`organizations failed (${res.status})`);
        }
        const data = (await res.json()) as Organization[];
        
        // Enrich with permissions
        const enriched: OrganizationWithActions[] = data.map((org) => {
          const role = org.role?.toLowerCase();
          const canManage = role === "owner" || role === "admin" || role === "manager";
          const canEdit = role === "owner" || role === "admin";
          return { ...org, canManage, canEdit };
        });

        // Simple pagination (client-side for now)
        const sorted = enriched.sort((a, b) => {
          if (!sort) return 0;
          
          // Special handling for member_count (number)
          if (sort.columnId === "member_count") {
            const aVal = a.member_count ?? 0;
            const bVal = b.member_count ?? 0;
            if (sort.direction === "asc") {
              return aVal - bVal;
            }
            return bVal - aVal;
          }
          
          // Default string comparison for other columns
          const aVal = a[sort.columnId as keyof Organization] as string | number;
          const bVal = b[sort.columnId as keyof Organization] as string | number;
          if (sort.direction === "asc") {
            return aVal > bVal ? 1 : aVal < bVal ? -1 : 0;
          }
          return aVal < bVal ? 1 : aVal > bVal ? -1 : 0;
        });

        return {
          offset,
          items: sorted.slice(offset, offset + limit),
          totalCount: sorted.length,
          filteredCount: sorted.length,
        };
      } catch (e) {
        console.error(e);
        throw e;
      }
    },
    [sort]
  );

  const fetchOrganizations = async () => {
    try {
      setLoading(true);
      setError(null);
      // Trigger refresh by incrementing refreshTick
      setRefreshTick((v) => v + 1);
    } catch (e) {
      console.error(e);
      setError("Erreur réseau");
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void fetchOrganizations();
  }, []);

  const openCreate = () => {
    setForm({ name: "", slug: "" });
    setCreateOpen(true);
  };

  const openMembers = (org: Organization) => {
    setSelectedOrg(org);
    setMembersOpen(true);
  };

  const createOrganization = async () => {
    if (!form.name.trim()) {
      setError("Le nom est requis");
      return;
    }
    try {
      const res = await apiRequest("/organizations", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          name: form.name.trim(),
          slug: form.slug.trim() || undefined,
          set_as_current: true,
        }),
      });
      if (!res.ok) {
        const body = await res.json().catch(() => null);
        const code = body?.error || body?.message;
        const msg =
          code === "conflict" || code === "organization_slug_already_exists"
            ? "Ce slug d'organisation existe déjà"
            : `Erreur création organisation (${res.status})`;
        setError(msg);
        snackbar.error(msg, { title: "Organisations", details: JSON.stringify(body, null, 2) });
        return;
      }
      setCreateOpen(false);
      setRefreshTick((v) => v + 1);
      // Refresh current org
      const meRes = await apiRequest("/auth/me");
      if (meRes.ok) {
        const meData = await meRes.json();
        setCurrentOrgId(meData.current_organization_id || null);
      }
      snackbar.success("Organisation créée", { title: "Organisations" });
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(`Erreur réseau ${msg}`);
      snackbar.error("Erreur réseau", { title: "Organisations", details: msg });
    }
  };

  const setCurrentOrganization = useCallback(async (orgId: string | null) => {
    try {
      const res = await apiRequest("/organizations/current", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ organization_id: orgId }),
      });
      if (!res.ok) {
        const body = await res.json().catch(() => null);
        const code = body?.error || body?.message;
        const msg =
          code === "not_a_member"
            ? "Vous n'êtes pas membre de cette organisation"
            : "Impossible de changer d'organisation";
        snackbar.error(msg, { title: "Organisations", details: JSON.stringify(body, null, 2) });
        return;
      }
      setCurrentOrgId(orgId);
      setRefreshTick((v) => v + 1);
      snackbar.success(orgId ? "Organisation sélectionnée" : "Mode Personal activé", { title: "Organisations" });
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      snackbar.error("Erreur réseau", { title: "Organisations", details: msg });
    }
  }, [snackbar, setCurrentOrgId, setRefreshTick, setError]);

  const columns = useMemo<IADataTableColumn<OrganizationWithActions>[]>(() => {
    return [
      {
        id: "name",
        label: "Nom",
        width: 250,
        sortable: true,
        cell: ({ row }) => (
          <div className="flex items-center gap-2">
            <Building2 className="h-4 w-4 text-muted-foreground" />
            <span className="font-medium">{row.name}</span>
            {currentOrgId === row.id && (
              <CheckCircle2 className="h-4 w-4 text-green-600" aria-label="Organisation courante" />
            )}
          </div>
        ),
      },
      {
        id: "slug",
        label: "Slug",
        width: 200,
        sortable: true,
        cell: ({ row }) => <span className="font-mono text-xs text-muted-foreground">{row.slug}</span>,
      },
      {
        id: "member_count",
        label: "Nombre d'utilisateurs",
        width: 180,
        sortable: true,
        cell: ({ row }) => (
          <div className="flex items-center gap-2">
            <Users className="h-4 w-4 text-muted-foreground" />
            <span className="font-medium">{row.member_count ?? 0}</span>
          </div>
        ),
      },
      {
        id: "created_at",
        label: "Créée",
        width: 200,
        sortable: true,
        cell: ({ row }) => (
          <span className="font-mono text-xs">{new Date(row.created_at).toISOString().slice(0, 19).replace("T", " ")}</span>
        ),
      },
      {
        id: "actions",
        label: "Actions",
        width: 300,
        align: "right",
        disableReorder: true,
        sortable: false,
        cell: ({ row }) => (
          <div className="flex justify-end gap-2" onClick={(e) => e.stopPropagation()}>
            {currentOrgId !== row.id && (
              <Button variant="outline" size="sm" onClick={() => void setCurrentOrganization(row.id)}>
                Sélectionner
              </Button>
            )}
            {row.canManage && (
              <Button variant="outline" size="sm" onClick={() => openMembers(row)}>
                <Users className="mr-1 h-3 w-3" />
                Membres
              </Button>
            )}
          </div>
        ),
      },
    ];
  }, [currentOrgId, setCurrentOrganization, openMembers]);

  return (
    <div className="p-8 space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Organisations</h1>
          <p className="text-muted-foreground">Gérer vos organisations et leurs membres (owner/admin/manager).</p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" onClick={() => setRefreshTick((v) => v + 1)} disabled={loading}>
            Refresh
          </Button>
          <Button onClick={openCreate}>
            <Building2 className="mr-2 h-4 w-4" />
            Créer une organisation
          </Button>
        </div>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Liste</CardTitle>
        </CardHeader>
        <CardContent>
          {error ? (
            <IAAlert variant="destructive" className="mb-3">
              <IAAlertTitle>Erreur</IAAlertTitle>
              <IAAlertDescription>{error}</IAAlertDescription>
            </IAAlert>
          ) : null}
          <IADataTable<OrganizationWithActions>
            listId="organizations:list"
            title="Organisations"
            dataKey={JSON.stringify({ refreshTick, sort })}
            leftMeta={loading ? <span className="text-sm text-muted-foreground">Loading…</span> : undefined}
            autoHeight={true}
            height={520}
            rowHeight={52}
            columns={columns}
            loadRange={loadRange}
            sortState={sort}
            onSortChange={setSort}
            sortingMode="client"
          />
        </CardContent>
      </Card>

      {/* Create dialog */}
      <Dialog open={createOpen} onOpenChange={setCreateOpen}>
        <DialogContent showCloseButton={false}>
          <DialogHeader>
            <DialogTitle>Créer une organisation</DialogTitle>
          </DialogHeader>
          <div className="grid gap-3 py-2">
            <div className="grid gap-2">
              <Label>Nom *</Label>
              <Input
                value={form.name}
                onChange={(e: ChangeEvent<HTMLInputElement>) => setForm((s) => ({ ...s, name: e.target.value }))}
                placeholder="Ex: Mon Entreprise"
              />
            </div>
            <div className="grid gap-2">
              <Label>Slug (optionnel)</Label>
              <Input
                value={form.slug}
                onChange={(e: ChangeEvent<HTMLInputElement>) => setForm((s) => ({ ...s, slug: e.target.value }))}
                placeholder="Ex: mon-entreprise (auto-généré si vide)"
              />
              <p className="text-xs text-muted-foreground">
                Identifiant unique de l&apos;organisation. Sera généré automatiquement à partir du nom si non spécifié.
              </p>
            </div>
          </div>
          <DialogFooter className="sm:justify-between">
            <Button variant="outline" onClick={() => setCreateOpen(false)}>
              Annuler
            </Button>
            <Button onClick={createOrganization} disabled={!form.name.trim()}>
              Créer
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Members dialog */}
      {selectedOrg && (
        <OrganizationMembersDialog
          open={membersOpen}
          onOpenChange={(open) => {
            setMembersOpen(open);
            if (!open) {
              setSelectedOrg(null);
              setRefreshTick((v) => v + 1);
            }
          }}
          organizationId={selectedOrg.id}
          organizationName={selectedOrg.name}
          actorOrgRole={selectedOrg.role}
        />
      )}
    </div>
  );
}

