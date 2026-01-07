"use client";

import { useCallback, useEffect, useMemo, useState, type ChangeEvent } from "react";
import { apiRequest } from "@/lib/api";
import type { Organization } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { IADataTable, type DataTableSortState, type IADataTableColumn, type LoadRangeResult } from "ia-widgets";
import { IAAlert, IAAlertDescription, IAAlertTitle } from "ia-designsys";
import { useSnackbar } from "ia-widgets";
import { Building2 } from "lucide-react";

type User = {
  id: string;
  username: string;
  email: string;
  role: string; // Global role (admin, user, etc.)
  first_name?: string | null;
  last_name?: string | null;
  created_at: string;
  updated_at: string;
  // Organization context (nullable - user may not be in any org)
  organization_id?: string | null;
  organization_name?: string | null;
  organization_slug?: string | null;
  organization_role?: string | null; // Role in the organization (owner, admin, manager, user)
};

export default function UsersPage() {
  const snackbar = useSnackbar();
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [refreshTick, setRefreshTick] = useState(0);
  const [sort, setSort] = useState<DataTableSortState>(null);

  const [createOpen, setCreateOpen] = useState(false);
  const [editOpen, setEditOpen] = useState(false);
  const [selected, setSelected] = useState<User | null>(null);

  // Filters
  const [organizations, setOrganizations] = useState<Organization[]>([]);
  const [filterOrganizationId, setFilterOrganizationId] = useState<string | null>(null);
  const [filterOrganizationRole, setFilterOrganizationRole] = useState<string | null>(null);

  const [form, setForm] = useState({
    username: "admin",
    email: "",
    password: "",
    role: "admin",
    first_name: "",
    last_name: "",
  });

  const fetchUsers = async () => {
    try {
      setLoading(true);
      setError(null);
      const res = await fetch(apiUrl("/users"));
      if (!res.ok) {
        setError("Accès refusé (admin requis) ou erreur API");
        return;
      }
    } catch (e) {
      console.error(e);
      setError("Erreur réseau");
    } finally {
      setLoading(false);
    }
  };

  // Fetch organizations for filter
  useEffect(() => {
    const fetchOrgs = async () => {
      try {
        const res = await apiRequest("/organizations");
        if (res.ok) {
          const data = (await res.json()) as Organization[];
          setOrganizations(Array.isArray(data) ? data : []);
        }
      } catch (e) {
        console.error("Failed to fetch organizations:", e);
      }
    };
    void fetchOrgs();
  }, []);

  useEffect(() => {
    void fetchUsers();
  }, [refreshTick, filterOrganizationId, filterOrganizationRole]);

  const openCreate = () => {
    setForm({ username: "admin", email: "", password: "", role: "admin", first_name: "", last_name: "" });
    setCreateOpen(true);
  };

  const openEdit = (u: User) => {
    setSelected(u);
    setForm({
      username: u.username,
      email: u.email,
      password: "",
      role: u.role,
      first_name: u.first_name ?? "",
      last_name: u.last_name ?? "",
    });
    setEditOpen(true);
  };

  const createUser = async () => {
    try {
      const res = await apiRequest("/users", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          username: form.username,
          email: form.email,
          password: form.password,
          role: form.role,
          first_name: form.first_name || null,
          last_name: form.last_name || null,
        }),
      });
      if (!res.ok) {
        const msg = await res.text().catch(() => "");
        const eMsg = `Erreur création user (${res.status})`;
        setError(`${eMsg} ${msg}`);
        snackbar.error(eMsg, { title: "Users", details: msg });
        return;
      }
      setCreateOpen(false);
      setRefreshTick((v) => v + 1);
      snackbar.success("Utilisateur créé", { title: "Users" });
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(`Erreur réseau ${msg}`);
      snackbar.error("Erreur réseau", { title: "Users", details: msg });
    }
  };

  const saveUser = async () => {
    if (!selected) return;
    try {
      const res = await apiRequest(`/users/${selected.id}`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          username: form.username,
          email: form.email,
          role: form.role,
          first_name: form.first_name || null,
          last_name: form.last_name || null,
          ...(form.password.trim() ? { password: form.password } : {}),
        }),
      });
      if (!res.ok) {
        const msg = await res.text().catch(() => "");
        const eMsg = `Erreur update user (${res.status})`;
        setError(`${eMsg} ${msg}`);
        snackbar.error(eMsg, { title: "Users", details: msg });
        return;
      }
      setEditOpen(false);
      setSelected(null);
      setRefreshTick((v) => v + 1);
      snackbar.success("Utilisateur mis à jour", { title: "Users" });
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(`Erreur réseau ${msg}`);
      snackbar.error("Erreur réseau", { title: "Users", details: msg });
    }
  };

  const deleteUser = async (u: User) => {
    if (!confirm(`Supprimer l'utilisateur ${u.email} ?`)) return;
    try {
      const res = await apiRequest(`/users/${u.id}`, { method: "DELETE" });
      if (!res.ok && res.status !== 204) {
        const msg = await res.text().catch(() => "");
        const eMsg = `Erreur suppression user (${res.status})`;
        setError(`${eMsg} ${msg}`);
        snackbar.error(eMsg, { title: "Users", details: msg });
        return;
      }
      setRefreshTick((v) => v + 1);
      snackbar.success("Utilisateur supprimé", { title: "Users" });
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(`Erreur réseau ${msg}`);
      snackbar.error("Erreur réseau", { title: "Users", details: msg });
    }
  };

  type UsersSearchResponse = {
    offset: number;
    limit: number;
    total_count: number;
    filtered_count: number;
    rows: User[];
  };

  const loadRange = useCallback(
    async (offset: number, limit: number): Promise<LoadRangeResult<User>> => {
      const params = new URLSearchParams();
      params.set("offset", String(offset));
      params.set("limit", String(limit));
      if (filterOrganizationId) {
        params.set("organization_id", filterOrganizationId);
      }
      if (filterOrganizationRole) {
        params.set("organization_role", filterOrganizationRole);
      }
      if (sort) {
        const by = ({
          username: "username",
          email: "email",
          role: "role",
          created_at: "created_at",
          updated_at: "updated_at",
          organization_name: "organization_name",
          organization_role: "organization_role",
        } as Record<string, string>)[sort.columnId];
        if (by) {
          params.set("sort_by", by);
          params.set("sort_dir", sort.direction);
        }
      }
      const res = await apiRequest(`/users/search?${params.toString()}`);
      if (!res.ok) {
        throw new Error(`users/search failed (${res.status})`);
      }
      const data = (await res.json()) as UsersSearchResponse;
      return {
        offset: data.offset,
        items: data.rows,
        totalCount: data.total_count,
        filteredCount: data.filtered_count,
      };
    },
    [sort, filterOrganizationId, filterOrganizationRole]
  );

  const columns = useMemo<IADataTableColumn<User>[]>(() => {
    return [
      { id: "username", label: "Username", width: 180, sortable: true, cell: ({ row }) => <span className="font-mono text-xs">{row.username}</span> },
      { id: "email", label: "Email", width: 240, sortable: true, cell: ({ row }) => <span className="font-medium">{row.email}</span> },
      {
        id: "name",
        label: "Nom",
        width: 180,
        sortable: false,
        cell: ({ row }) => <span>{`${row.first_name ?? ""} ${row.last_name ?? ""}`.trim() || "-"}</span>,
      },
      { id: "role", label: "Rôle global", width: 120, sortable: true, cell: ({ row }) => <span className="text-xs">{row.role}</span> },
      {
        id: "organization_name",
        label: "Organisation",
        width: 200,
        sortable: true,
        cell: ({ row }) => (
          <div className="flex items-center gap-2">
            {row.organization_name ? (
              <>
                <Building2 className="h-3 w-3 text-muted-foreground" />
                <span className="text-sm">{row.organization_name}</span>
              </>
            ) : (
              <span className="text-xs text-muted-foreground italic">Aucune</span>
            )}
          </div>
        ),
      },
      {
        id: "organization_role",
        label: "Rôle org",
        width: 120,
        sortable: true,
        cell: ({ row }) => {
          if (!row.organization_role) return <span className="text-xs text-muted-foreground">-</span>;
          const roleLabels: Record<string, string> = {
            owner: "Owner",
            admin: "Admin",
            manager: "Manager",
            user: "User",
          };
          const role = row.organization_role.toLowerCase();
          return (
            <span className="inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium bg-primary/10 text-primary">
              {roleLabels[role] || role}
            </span>
          );
        },
      },
      {
        id: "created_at",
        label: "Créé",
        width: 180,
        sortable: true,
        cell: ({ row }) => <span className="font-mono text-xs">{new Date(row.created_at).toISOString().slice(0, 19).replace("T", " ")}</span>,
      },
      {
        id: "actions",
        label: "Actions",
        width: 220,
        align: "right",
        disableReorder: true,
        sortable: false,
        cell: ({ row }) => (
          <div className="flex justify-end gap-2" onClick={(e) => e.stopPropagation()}>
            <Button variant="outline" size="sm" onClick={() => openEdit(row)}>
              Edit
            </Button>
            <Button variant="destructive" size="sm" onClick={() => deleteUser(row)}>
              Delete
            </Button>
          </div>
        ),
      },
    ];
  }, [deleteUser, openEdit]);

  return (
    <div className="p-8 space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Users</h1>
          <p className="text-muted-foreground">Créer / modifier / supprimer des users (admin).</p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" onClick={() => setRefreshTick((v) => v + 1)} disabled={loading}>
            Refresh
          </Button>
          <Button onClick={openCreate}>Créer un user</Button>
        </div>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Liste</CardTitle>
        </CardHeader>
        <CardContent>
          {/* Filters */}
          <div className="mb-4 flex gap-4 items-end">
            <div className="flex-1 grid gap-2">
              <Label htmlFor="filter-org">Filtrer par Organisation</Label>
              <Select
                value={filterOrganizationId || "all"}
                onValueChange={(value) => setFilterOrganizationId(value === "all" ? null : value)}
              >
                <SelectTrigger id="filter-org" className="w-full">
                  <SelectValue placeholder="Toutes les organisations" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">Toutes les organisations</SelectItem>
                  {organizations.map((org) => (
                    <SelectItem key={org.id} value={org.id}>
                      {org.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="flex-1 grid gap-2">
              <Label htmlFor="filter-role">Filtrer par Rôle org</Label>
              <Select
                value={filterOrganizationRole || "all"}
                onValueChange={(value) => setFilterOrganizationRole(value === "all" ? null : value)}
              >
                <SelectTrigger id="filter-role" className="w-full">
                  <SelectValue placeholder="Tous les rôles" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">Tous les rôles</SelectItem>
                  <SelectItem value="owner">Owner</SelectItem>
                  <SelectItem value="admin">Admin</SelectItem>
                  <SelectItem value="manager">Manager</SelectItem>
                  <SelectItem value="user">User</SelectItem>
                </SelectContent>
              </Select>
            </div>
            {(filterOrganizationId || filterOrganizationRole) && (
              <Button
                variant="outline"
                onClick={() => {
                  setFilterOrganizationId(null);
                  setFilterOrganizationRole(null);
                }}
              >
                Réinitialiser
              </Button>
            )}
          </div>

          {error ? (
            <IAAlert variant="destructive" className="mb-3">
              <IAAlertTitle>Erreur</IAAlertTitle>
              <IAAlertDescription>{error}</IAAlertDescription>
            </IAAlert>
          ) : null}
          <IADataTable<User>
            listId="users:list"
            title="Users"
            dataKey={JSON.stringify({ refreshTick, sort })}
            leftMeta={loading ? <span className="text-sm text-muted-foreground">Loading…</span> : undefined}
            autoHeight={true}
            height={520}
            rowHeight={52}
            columns={columns}
            loadRange={loadRange}
            sortState={sort}
            onSortChange={setSort}
            sortingMode="server"
          />
        </CardContent>
      </Card>

      {/* Create dialog */}
      <Dialog open={createOpen} onOpenChange={setCreateOpen}>
        <DialogContent showCloseButton={false}>
          <DialogHeader>
            <DialogTitle>Créer un user</DialogTitle>
          </DialogHeader>
          <div className="grid gap-3 py-2">
            <div className="grid gap-2">
              <Label>Username</Label>
              <Input value={form.username} onChange={(e: ChangeEvent<HTMLInputElement>) => setForm((s) => ({ ...s, username: e.target.value }))} />
            </div>
            <div className="grid gap-2">
              <Label>Email</Label>
              <Input value={form.email} onChange={(e: ChangeEvent<HTMLInputElement>) => setForm((s) => ({ ...s, email: e.target.value }))} />
            </div>
            <div className="grid gap-2">
              <Label>Mot de passe</Label>
              <Input type="password" value={form.password} onChange={(e: ChangeEvent<HTMLInputElement>) => setForm((s) => ({ ...s, password: e.target.value }))} />
            </div>
            <div className="grid gap-2">
              <Label>Rôle</Label>
              <Input value={form.role} onChange={(e: ChangeEvent<HTMLInputElement>) => setForm((s) => ({ ...s, role: e.target.value }))} />
            </div>
            <div className="grid gap-2">
              <Label>Prénom</Label>
              <Input value={form.first_name} onChange={(e: ChangeEvent<HTMLInputElement>) => setForm((s) => ({ ...s, first_name: e.target.value }))} />
            </div>
            <div className="grid gap-2">
              <Label>Nom</Label>
              <Input value={form.last_name} onChange={(e: ChangeEvent<HTMLInputElement>) => setForm((s) => ({ ...s, last_name: e.target.value }))} />
            </div>
          </div>
          <DialogFooter className="sm:justify-between">
            <Button variant="outline" onClick={() => setCreateOpen(false)}>
              Annuler
            </Button>
            <Button onClick={createUser}>Créer</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Edit dialog */}
      <Dialog open={editOpen} onOpenChange={(o: boolean) => { setEditOpen(o); if (!o) setSelected(null); }}>
        <DialogContent showCloseButton={false}>
          <DialogHeader>
            <DialogTitle>Modifier user</DialogTitle>
          </DialogHeader>
          <div className="grid gap-3 py-2">
            <div className="grid gap-2">
              <Label>Username</Label>
              <Input value={form.username} onChange={(e: ChangeEvent<HTMLInputElement>) => setForm((s) => ({ ...s, username: e.target.value }))} />
            </div>
            <div className="grid gap-2">
              <Label>Email</Label>
              <Input value={form.email} onChange={(e: ChangeEvent<HTMLInputElement>) => setForm((s) => ({ ...s, email: e.target.value }))} />
            </div>
            <div className="grid gap-2">
              <Label>Nouveau mot de passe (optionnel)</Label>
              <Input type="password" value={form.password} onChange={(e: ChangeEvent<HTMLInputElement>) => setForm((s) => ({ ...s, password: e.target.value }))} />
            </div>
            <div className="grid gap-2">
              <Label>Rôle</Label>
              <Input value={form.role} onChange={(e: ChangeEvent<HTMLInputElement>) => setForm((s) => ({ ...s, role: e.target.value }))} />
            </div>
            <div className="grid gap-2">
              <Label>Prénom</Label>
              <Input value={form.first_name} onChange={(e: ChangeEvent<HTMLInputElement>) => setForm((s) => ({ ...s, first_name: e.target.value }))} />
            </div>
            <div className="grid gap-2">
              <Label>Nom</Label>
              <Input value={form.last_name} onChange={(e: ChangeEvent<HTMLInputElement>) => setForm((s) => ({ ...s, last_name: e.target.value }))} />
            </div>
          </div>
          <DialogFooter className="sm:justify-between">
            <Button variant="outline" onClick={() => setEditOpen(false)}>
              Annuler
            </Button>
            <Button onClick={saveUser}>Enregistrer</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}


