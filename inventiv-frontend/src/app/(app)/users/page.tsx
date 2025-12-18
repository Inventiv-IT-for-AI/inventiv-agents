"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import { apiUrl } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useI18n } from "@/i18n/I18nProvider";
import { LOCALE_LABELS, normalizeLocale } from "@/i18n/i18n";
import type { LoadRangeResult } from "@/components/shared/VirtualizedRemoteList";
import { InventivDataTable, type DataTableSortState, type InventivDataTableColumn } from "@/components/shared/InventivDataTable";

type User = {
  id: string;
  username: string;
  email: string;
  role: string;
  first_name?: string | null;
  last_name?: string | null;
  locale_code: string;
  created_at: string;
  updated_at: string;
};

export default function UsersPage() {
  const { t } = useI18n();
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [refreshTick, setRefreshTick] = useState(0);
  const [sort, setSort] = useState<DataTableSortState>(null);
  const [locales, setLocales] = useState<Array<{ code: string; name: string; is_rtl?: boolean; is_active?: boolean }>>([]);

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
    locale_code: "en-US",
  });

  const fetchLocales = async () => {
    try {
      const res = await fetch(apiUrl("/locales"));
      if (!res.ok) return;
      const data = await res.json();
      if (Array.isArray(data)) setLocales(data);
    } catch {
      // ignore
    }
  };

  useEffect(() => {
    void fetchLocales();
  }, []);

  const openCreate = () => {
    setForm({ username: "admin", email: "", password: "", role: "admin", first_name: "", last_name: "", locale_code: "en-US" });
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
      locale_code: normalizeLocale(u.locale_code),
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
        locale_code: normalizeLocale(form.locale_code),
      }),
    });
    if (!res.ok) {
      const msg = await res.text().catch(() => "");
      alert(`Erreur création user (${res.status}) ${msg}`);
      return;
    }
    setCreateOpen(false);
    setRefreshTick((v) => v + 1);
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
        locale_code: normalizeLocale(form.locale_code),
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
    setRefreshTick((v) => v + 1);
  };

  const deleteUser = async (u: User) => {
    if (!confirm(`Supprimer l'utilisateur ${u.email} ?`)) return;
    const res = await fetch(apiUrl(`/users/${u.id}`), { method: "DELETE" });
    if (!res.ok && res.status !== 204) {
      const msg = await res.text().catch(() => "");
      alert(`Erreur suppression user (${res.status}) ${msg}`);
      return;
    }
    setRefreshTick((v) => v + 1);
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

  const columns = useMemo<InventivDataTableColumn<User>[]>(() => {
    return [
      { id: "username", label: t("usersPage.username"), width: 200, sortable: true, cell: ({ row }) => <span className="font-mono text-xs">{row.username}</span> },
      { id: "email", label: t("usersPage.email"), width: 280, sortable: true, cell: ({ row }) => <span className="font-medium">{row.email}</span> },
      {
        id: "name",
        label: t("usersPage.name"),
        width: 220,
        sortable: false,
        cell: ({ row }) => <span>{`${row.first_name ?? ""} ${row.last_name ?? ""}`.trim() || "-"}</span>,
      },
      { id: "role", label: t("usersPage.role"), width: 140, sortable: true, cell: ({ row }) => row.role },
      {
        id: "locale_code",
        label: t("usersPage.locale"),
        width: 160,
        sortable: false,
        cell: ({ row }) => <span className="font-mono text-xs">{LOCALE_LABELS[normalizeLocale(row.locale_code)] ?? row.locale_code}</span>,
      },
      {
        id: "created_at",
        label: t("usersPage.createdAt"),
        width: 200,
        sortable: true,
        cell: ({ row }) => <span className="font-mono text-xs">{new Date(row.created_at).toISOString().slice(0, 19).replace("T", " ")}</span>,
      },
      {
        id: "actions",
        label: t("usersPage.actions"),
        width: 220,
        align: "right",
        disableReorder: true,
        sortable: false,
        cell: ({ row }) => (
          <div className="flex justify-end gap-2" onClick={(e) => e.stopPropagation()}>
            <Button variant="outline" size="sm" onClick={() => openEdit(row)}>
              {t("usersPage.edit")}
            </Button>
            <Button variant="destructive" size="sm" onClick={() => deleteUser(row)}>
              {t("usersPage.delete")}
            </Button>
          </div>
        ),
      },
    ];
  }, [deleteUser, openEdit, t]);

  return (
    <div className="p-8 space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">{t("usersPage.title")}</h1>
          <p className="text-muted-foreground">{t("usersPage.subtitle")}</p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" onClick={() => setRefreshTick((v) => v + 1)} disabled={loading}>
            {t("usersPage.refresh")}
          </Button>
          <Button onClick={openCreate}>{t("usersPage.create")}</Button>
        </div>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>{t("usersPage.listTitle")}</CardTitle>
        </CardHeader>
        <CardContent>
          {error ? <div className="text-sm text-red-600 mb-3">{error}</div> : null}
          <InventivDataTable<User>
            listId="users:list"
            title={t("usersPage.listTitle")}
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
            <DialogTitle>{t("usersPage.create")}</DialogTitle>
          </DialogHeader>
          <div className="grid gap-3 py-2">
            <div className="grid gap-2">
              <Label>{t("usersPage.username")}</Label>
              <Input value={form.username} onChange={(e) => setForm((s) => ({ ...s, username: e.target.value }))} />
            </div>
            <div className="grid gap-2">
              <Label>{t("usersPage.email")}</Label>
              <Input value={form.email} onChange={(e) => setForm((s) => ({ ...s, email: e.target.value }))} />
            </div>
            <div className="grid gap-2">
              <Label>{t("usersPage.password")}</Label>
              <Input type="password" value={form.password} onChange={(e) => setForm((s) => ({ ...s, password: e.target.value }))} />
            </div>
            <div className="grid gap-2">
              <Label>{t("usersPage.role")}</Label>
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
            <div className="grid gap-2">
              <Label>{t("usersPage.locale")}</Label>
              <Select value={form.locale_code} onValueChange={(v) => setForm((s) => ({ ...s, locale_code: normalizeLocale(v) }))}>
                <SelectTrigger className="w-full">
                  <SelectValue placeholder={LOCALE_LABELS[normalizeLocale(form.locale_code)] ?? form.locale_code} />
                </SelectTrigger>
                <SelectContent>
                  {(locales.length ? locales.map((l) => l.code) : Object.keys(LOCALE_LABELS)).map((code) => (
                    <SelectItem key={code} value={code}>
                      {LOCALE_LABELS[normalizeLocale(code)] ?? code}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>
          <DialogFooter className="sm:justify-between">
            <Button variant="outline" onClick={() => setCreateOpen(false)}>
              {t("usersPage.cancel")}
            </Button>
            <Button onClick={createUser}>{t("usersPage.create")}</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Edit dialog */}
      <Dialog open={editOpen} onOpenChange={(o) => { setEditOpen(o); if (!o) setSelected(null); }}>
        <DialogContent showCloseButton={false}>
          <DialogHeader>
            <DialogTitle>{t("usersPage.edit")}</DialogTitle>
          </DialogHeader>
          <div className="grid gap-3 py-2">
            <div className="grid gap-2">
              <Label>{t("usersPage.username")}</Label>
              <Input value={form.username} onChange={(e) => setForm((s) => ({ ...s, username: e.target.value }))} />
            </div>
            <div className="grid gap-2">
              <Label>{t("usersPage.email")}</Label>
              <Input value={form.email} onChange={(e) => setForm((s) => ({ ...s, email: e.target.value }))} />
            </div>
            <div className="grid gap-2">
              <Label>{t("usersPage.passwordOptional")}</Label>
              <Input type="password" value={form.password} onChange={(e) => setForm((s) => ({ ...s, password: e.target.value }))} />
            </div>
            <div className="grid gap-2">
              <Label>{t("usersPage.role")}</Label>
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
            <div className="grid gap-2">
              <Label>{t("usersPage.locale")}</Label>
              <Select value={form.locale_code} onValueChange={(v) => setForm((s) => ({ ...s, locale_code: normalizeLocale(v) }))}>
                <SelectTrigger className="w-full">
                  <SelectValue placeholder={LOCALE_LABELS[normalizeLocale(form.locale_code)] ?? form.locale_code} />
                </SelectTrigger>
                <SelectContent>
                  {(locales.length ? locales.map((l) => l.code) : Object.keys(LOCALE_LABELS)).map((code) => (
                    <SelectItem key={code} value={code}>
                      {LOCALE_LABELS[normalizeLocale(code)] ?? code}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>
          <DialogFooter className="sm:justify-between">
            <Button variant="outline" onClick={() => setEditOpen(false)}>
              {t("usersPage.cancel")}
            </Button>
            <Button onClick={saveUser}>{t("usersPage.save")}</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}


