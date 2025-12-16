"use client";

import { useEffect, useState } from "react";
import { apiUrl } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";

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
  const [users, setUsers] = useState<User[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

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
        setUsers([]);
        return;
      }
      const data = (await res.json()) as User[];
      setUsers(data);
    } catch (e) {
      console.error(e);
      setError("Erreur réseau");
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void fetchUsers();
  }, []);

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
      alert(`Erreur création user (${res.status}) ${msg}`);
      return;
    }
    setCreateOpen(false);
    await fetchUsers();
  };

  const saveUser = async () => {
    if (!selected) return;
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
      alert(`Erreur update user (${res.status}) ${msg}`);
      return;
    }
    setEditOpen(false);
    setSelected(null);
    await fetchUsers();
  };

  const deleteUser = async (u: User) => {
    if (!confirm(`Supprimer l'utilisateur ${u.email} ?`)) return;
    const res = await fetch(apiUrl(`/users/${u.id}`), { method: "DELETE" });
    if (!res.ok && res.status !== 204) {
      const msg = await res.text().catch(() => "");
      alert(`Erreur suppression user (${res.status}) ${msg}`);
      return;
    }
    await fetchUsers();
  };

  return (
    <div className="p-8 space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Users</h1>
          <p className="text-muted-foreground">Créer / modifier / supprimer des users (admin).</p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" onClick={fetchUsers} disabled={loading}>
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
          {error ? <div className="text-sm text-red-600 mb-3">{error}</div> : null}
          {loading ? (
            <div className="text-sm text-muted-foreground">Chargement…</div>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Username</TableHead>
                  <TableHead>Email</TableHead>
                  <TableHead>Nom</TableHead>
                  <TableHead>Rôle</TableHead>
                  <TableHead className="text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {users.map((u) => (
                  <TableRow key={u.id}>
                    <TableCell className="font-mono text-xs">{u.username}</TableCell>
                    <TableCell className="font-medium">{u.email}</TableCell>
                    <TableCell>{`${u.first_name ?? ""} ${u.last_name ?? ""}`.trim() || "-"}</TableCell>
                    <TableCell>{u.role}</TableCell>
                    <TableCell className="text-right space-x-2">
                      <Button variant="outline" size="sm" onClick={() => openEdit(u)}>
                        Edit
                      </Button>
                      <Button variant="destructive" size="sm" onClick={() => deleteUser(u)}>
                        Delete
                      </Button>
                    </TableCell>
                  </TableRow>
                ))}
                {users.length === 0 ? (
                  <TableRow>
                    <TableCell colSpan={5} className="text-sm text-muted-foreground">
                      Aucun user
                    </TableCell>
                  </TableRow>
                ) : null}
              </TableBody>
            </Table>
          )}
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
              <Input value={form.username} onChange={(e) => setForm((s) => ({ ...s, username: e.target.value }))} />
            </div>
            <div className="grid gap-2">
              <Label>Email</Label>
              <Input value={form.email} onChange={(e) => setForm((s) => ({ ...s, email: e.target.value }))} />
            </div>
            <div className="grid gap-2">
              <Label>Mot de passe</Label>
              <Input type="password" value={form.password} onChange={(e) => setForm((s) => ({ ...s, password: e.target.value }))} />
            </div>
            <div className="grid gap-2">
              <Label>Rôle</Label>
              <Input value={form.role} onChange={(e) => setForm((s) => ({ ...s, role: e.target.value }))} />
            </div>
            <div className="grid gap-2">
              <Label>Prénom</Label>
              <Input value={form.first_name} onChange={(e) => setForm((s) => ({ ...s, first_name: e.target.value }))} />
            </div>
            <div className="grid gap-2">
              <Label>Nom</Label>
              <Input value={form.last_name} onChange={(e) => setForm((s) => ({ ...s, last_name: e.target.value }))} />
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
      <Dialog open={editOpen} onOpenChange={(o) => { setEditOpen(o); if (!o) setSelected(null); }}>
        <DialogContent showCloseButton={false}>
          <DialogHeader>
            <DialogTitle>Modifier user</DialogTitle>
          </DialogHeader>
          <div className="grid gap-3 py-2">
            <div className="grid gap-2">
              <Label>Username</Label>
              <Input value={form.username} onChange={(e) => setForm((s) => ({ ...s, username: e.target.value }))} />
            </div>
            <div className="grid gap-2">
              <Label>Email</Label>
              <Input value={form.email} onChange={(e) => setForm((s) => ({ ...s, email: e.target.value }))} />
            </div>
            <div className="grid gap-2">
              <Label>Nouveau mot de passe (optionnel)</Label>
              <Input type="password" value={form.password} onChange={(e) => setForm((s) => ({ ...s, password: e.target.value }))} />
            </div>
            <div className="grid gap-2">
              <Label>Rôle</Label>
              <Input value={form.role} onChange={(e) => setForm((s) => ({ ...s, role: e.target.value }))} />
            </div>
            <div className="grid gap-2">
              <Label>Prénom</Label>
              <Input value={form.first_name} onChange={(e) => setForm((s) => ({ ...s, first_name: e.target.value }))} />
            </div>
            <div className="grid gap-2">
              <Label>Nom</Label>
              <Input value={form.last_name} onChange={(e) => setForm((s) => ({ ...s, last_name: e.target.value }))} />
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


