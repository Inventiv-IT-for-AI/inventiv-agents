"use client";

import { useCallback, useEffect, useMemo, useState, type ChangeEvent } from "react";
import { apiUrl } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { IADataTable, type DataTableSortState, type IADataTableColumn, type LoadRangeResult } from "ia-widgets";
import { IAAlert, IAAlertDescription, IAAlertTitle } from "ia-designsys";
import { useSnackbar } from "ia-widgets";

type User = {
  id: string;
  username: string;
  email: string;
  role: string;
  first_name?: string | null;
  last_name?: string | null;
  created_at: string;
  updated_at: string;
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

  useEffect(() => {
    void fetchUsers();
  }, [refreshTick]);

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
      const res = await fetch(apiUrl("/users"), {
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
      const res = await fetch(apiUrl(`/users/${selected.id}`), {
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
      const res = await fetch(apiUrl(`/users/${u.id}`), { method: "DELETE" });
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
      if (sort) {
        const by = ({ username: "username", email: "email", role: "role", created_at: "created_at", updated_at: "updated_at" } as Record<string, string>)[
          sort.columnId
        ];
        if (by) {
          params.set("sort_by", by);
          params.set("sort_dir", sort.direction);
        }
      }
      const res = await fetch(apiUrl(`/users/search?${params.toString()}`));
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
    [sort]
  );

  const columns = useMemo<IADataTableColumn<User>[]>(() => {
    return [
      { id: "username", label: "Username", width: 200, sortable: true, cell: ({ row }) => <span className="font-mono text-xs">{row.username}</span> },
      { id: "email", label: "Email", width: 280, sortable: true, cell: ({ row }) => <span className="font-medium">{row.email}</span> },
      {
        id: "name",
        label: "Nom",
        width: 220,
        sortable: false,
        cell: ({ row }) => <span>{`${row.first_name ?? ""} ${row.last_name ?? ""}`.trim() || "-"}</span>,
      },
      { id: "role", label: "Rôle", width: 140, sortable: true, cell: ({ row }) => row.role },
      {
        id: "created_at",
        label: "Créé",
        width: 200,
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
  }, []);

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


