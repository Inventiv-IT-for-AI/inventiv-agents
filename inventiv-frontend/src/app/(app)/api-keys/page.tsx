"use client";

import { useCallback, useMemo, useState, type ChangeEvent } from "react";
import { apiUrl } from "@/lib/api";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Plus, Pencil, Trash2 } from "lucide-react";
import { IAConfirmModal, IADataTable, IACopyButton, type DataTableSortState, type IADataTableColumn, type LoadRangeResult } from "ia-widgets";
import type { ApiKey } from "@/lib/types";

export default function ApiKeysPage() {
    const [refreshTick, setRefreshTick] = useState(0);
    const [sort, setSort] = useState<DataTableSortState>(null);

    const [apiKeysLoading, setApiKeysLoading] = useState(false);

    const [apiKeyCreateOpen, setApiKeyCreateOpen] = useState(false);
    const [apiKeyName, setApiKeyName] = useState("");
    const [createdApiKey, setCreatedApiKey] = useState<string | null>(null);

    const [apiKeyEditOpen, setApiKeyEditOpen] = useState(false);
    const [editingApiKey, setEditingApiKey] = useState<ApiKey | null>(null);
    const [apiKeyEditName, setApiKeyEditName] = useState("");

    const [revokeOpen, setRevokeOpen] = useState(false);
    const [apiKeyToRevoke, setApiKeyToRevoke] = useState<ApiKey | null>(null);

    // NOTE: don't set state synchronously in an effect (eslint react-hooks/set-state-in-effect).
    // We drive loading from the actual fetch lifecycle in `loadRange`.

    const createApiKey = async () => {
        const name = apiKeyName.trim();
        if (!name) return;
        const res = await fetch(apiUrl("api_keys"), {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ name }),
        });
        if (!res.ok) return;
        const body = (await res.json()) as { api_key?: string };
        setCreatedApiKey(body.api_key ?? null);
        setApiKeyName("");
        setRefreshTick((s) => s + 1);
    };

    const askRevoke = (k: ApiKey) => {
        setApiKeyToRevoke(k);
        setRevokeOpen(true);
    };

    const openRename = (k: ApiKey) => {
        setEditingApiKey(k);
        setApiKeyEditName(k.name);
        setApiKeyEditOpen(true);
    };

    const saveRename = async () => {
        if (!editingApiKey) return;
        const name = apiKeyEditName.trim();
        if (!name) return;
        const res = await fetch(apiUrl(`api_keys/${editingApiKey.id}`), {
            method: "PUT",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ name }),
        });
        if (!res.ok) return;
        setApiKeyEditOpen(false);
        setEditingApiKey(null);
        setRefreshTick((s) => s + 1);
    };

    type ApiKeysSearchResponse = {
        offset: number;
        limit: number;
        total_count: number;
        filtered_count: number;
        rows: ApiKey[];
    };

    const loadRange = useCallback(
        async (offset: number, limit: number): Promise<LoadRangeResult<ApiKey>> => {
            setApiKeysLoading(true);
            const params = new URLSearchParams();
            params.set("offset", String(offset));
            params.set("limit", String(limit));
            if (sort) {
                const by = ({ name: "name", prefix: "key_prefix", created_at: "created_at", status: "revoked_at" } as Record<string, string>)[sort.columnId];
                if (by) {
                    params.set("sort_by", by);
                    params.set("sort_dir", sort.direction);
                }
            }
            try {
                const res = await fetch(apiUrl(`api_keys/search?${params.toString()}`));
                if (!res.ok) throw new Error(`api_keys/search failed (${res.status})`);
                const data = (await res.json()) as ApiKeysSearchResponse;
                return {
                    offset: data.offset,
                    items: data.rows,
                    totalCount: data.total_count,
                    filteredCount: data.filtered_count,
                };
            } finally {
                setApiKeysLoading(false);
            }
        },
        [sort]
    );

    const apiKeyColumns = useMemo<IADataTableColumn<ApiKey>[]>(() => {
        return [
            { id: "name", label: "Name", width: 260, sortable: true, cell: ({ row }) => <span className="font-medium">{row.name}</span> },
            { id: "prefix", label: "Prefix", width: 160, sortable: true, cell: ({ row }) => <span className="font-mono text-xs">{row.key_prefix}</span> },
        {
            id: "created_at",
            label: "Created",
            width: 180,
            sortable: true,
            cell: ({ row }) => <span className="font-mono text-xs">{new Date(row.created_at).toISOString().slice(0, 19).replace("T", " ")}</span>,
        },
        {
            id: "status",
            label: "Status",
            width: 120,
            sortable: true,
            cell: ({ row }) =>
                row.revoked_at ? (
                    <span className="text-xs px-2 py-1 rounded bg-gray-200 text-gray-700">revoked</span>
                ) : (
                    <span className="text-xs px-2 py-1 rounded bg-green-200 text-green-800">active</span>
                ),
        },
        {
            id: "actions",
            label: "Actions",
            width: 200,
            align: "right",
            disableReorder: true,
            sortable: false,
            cell: ({ row }) => (
                <div className="flex justify-end gap-2" onClick={(e) => e.stopPropagation()}>
                    <Button variant="outline" size="sm" onClick={() => openRename(row)}>
                        <Pencil className="h-4 w-4 mr-2" />
                        Modifier
                    </Button>
                    <Button variant="destructive" size="sm" onClick={() => askRevoke(row)} disabled={!!row.revoked_at}>
                        <Trash2 className="h-4 w-4 mr-2" />
                        Revoke
                    </Button>
                </div>
            ),
        },
        ];
    }, []);

    return (
        <div className="p-6 space-y-4">
            <div className="flex items-center justify-between">
                <div>
                    <div className="text-2xl font-semibold">API Keys</div>
                    <div className="text-sm text-muted-foreground">Manage your OpenAI-compatible client API keys.</div>
                </div>
                <Button size="sm" onClick={() => { setApiKeyCreateOpen(true); setCreatedApiKey(null); }}>
                    <Plus className="h-4 w-4 mr-2" />
                    Ajouter
                </Button>
            </div>

            <Card>
                <CardContent>
                    <IADataTable<ApiKey>
                        listId="api_keys:list"
                        title="API Keys"
                        dataKey={JSON.stringify({ refreshTick, sort })}
                        leftMeta={apiKeysLoading ? <span className="text-sm text-muted-foreground">Loading…</span> : undefined}
                        autoHeight={true}
                        height={420}
                        rowHeight={52}
                        columns={apiKeyColumns}
                        loadRange={loadRange}
                        sortState={sort}
                        onSortChange={setSort}
                        sortingMode="server"
                    />
                </CardContent>
            </Card>

            {/* Create API Key dialog (shows secret once) */}
            <Dialog open={apiKeyCreateOpen} onOpenChange={(open: boolean) => { setApiKeyCreateOpen(open); if (!open) { setCreatedApiKey(null); setApiKeyName(""); } }}>
                <DialogContent showCloseButton={false} className="sm:max-w-[560px]">
                    <DialogHeader>
                        <DialogTitle>Ajouter une API Key</DialogTitle>
                    </DialogHeader>
                    <div className="grid gap-4 py-4">
                        {!createdApiKey ? (
                            <div className="grid grid-cols-4 items-center gap-4">
                                <Label htmlFor="api_key_name" className="text-right">Nom</Label>
                                <Input
                                    id="api_key_name"
                                    value={apiKeyName}
                                    onChange={(e: ChangeEvent<HTMLInputElement>) => setApiKeyName(e.target.value)}
                                    className="col-span-3"
                                    placeholder="ex: prod-backend, n8n, langchain..."
                                />
                            </div>
                        ) : (
                            <div className="space-y-2">
                                <p className="text-sm text-muted-foreground">
                                    Copie ta clé maintenant. Elle ne sera affichée qu’une seule fois.
                                </p>
                                <div className="flex items-center gap-2">
                                    <Input value={createdApiKey} readOnly className="font-mono text-xs" />
                                    <IACopyButton text={createdApiKey} />
                                </div>
                            </div>
                        )}
                    </div>
                    <DialogFooter>
                        <Button variant="outline" onClick={() => setApiKeyCreateOpen(false)}>
                            Fermer
                        </Button>
                        {!createdApiKey && (
                            <Button onClick={createApiKey} disabled={!apiKeyName.trim()}>
                                Créer
                            </Button>
                        )}
                    </DialogFooter>
                </DialogContent>
            </Dialog>

            {/* Rename API Key dialog */}
            <Dialog open={apiKeyEditOpen} onOpenChange={(open: boolean) => { setApiKeyEditOpen(open); if (!open) { setEditingApiKey(null); setApiKeyEditName(""); } }}>
                <DialogContent showCloseButton={false} className="sm:max-w-[560px]">
                    <DialogHeader>
                        <DialogTitle>Modifier l’API Key</DialogTitle>
                    </DialogHeader>
                    <div className="grid gap-4 py-4">
                        <div className="grid grid-cols-4 items-center gap-4">
                            <Label htmlFor="api_key_edit_name" className="text-right">Nom</Label>
                            <Input
                                id="api_key_edit_name"
                                value={apiKeyEditName}
                                onChange={(e: ChangeEvent<HTMLInputElement>) => setApiKeyEditName(e.target.value)}
                                className="col-span-3"
                            />
                        </div>
                    </div>
                    <DialogFooter>
                        <Button variant="outline" onClick={() => setApiKeyEditOpen(false)}>Annuler</Button>
                        <Button onClick={saveRename} disabled={!apiKeyEditName.trim()}>Enregistrer</Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>

            <IAConfirmModal
                open={revokeOpen}
                onOpenChange={(open) => {
                    if (!open) {
                        setRevokeOpen(false);
                        setApiKeyToRevoke(null);
                    }
                }}
                title="Révoquer l’API Key"
                description="Cette action invalide la clé et est irréversible."
                details={
                    <div className="space-y-1">
                        <div>
                            Nom: <span className="font-medium text-foreground">{apiKeyToRevoke?.name ?? "-"}</span>
                        </div>
                        <div>
                            Prefix: <span className="font-mono text-foreground text-xs">{apiKeyToRevoke?.key_prefix ?? "-"}</span>
                        </div>
                        <div>
                            ID: <span className="font-mono text-foreground text-xs">{apiKeyToRevoke?.id ?? "-"}</span>
                        </div>
                    </div>
                }
                confirmLabel="Révoquer"
                confirmingLabel="Révocation..."
                confirmVariant="destructive"
                successTitle="Demande de révocation prise en compte"
                successTone="danger"
                onConfirm={async () => {
                    if (!apiKeyToRevoke?.id) return;
                    const res = await fetch(apiUrl(`api_keys/${apiKeyToRevoke.id}`), { method: "DELETE" });
                    if (!res.ok) throw new Error("revoke failed");
                    setRefreshTick((s) => s + 1);
                }}
                autoCloseMs={1500}
            />
        </div>
    );
}


