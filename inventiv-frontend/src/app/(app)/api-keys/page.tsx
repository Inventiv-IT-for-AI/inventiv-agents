"use client";

import { useEffect, useState } from "react";
import { apiUrl } from "@/lib/api";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Plus, Pencil, Trash2 } from "lucide-react";
import { VirtualizedDataTable, type DataTableColumn } from "@/components/shared/VirtualizedDataTable";
import type { ApiKey } from "@/lib/types";
import { CopyButton } from "@/components/shared/CopyButton";

export default function ApiKeysPage() {
    const [refreshTick, setRefreshTick] = useState(0);

    const [apiKeys, setApiKeys] = useState<ApiKey[]>([]);
    const [apiKeysLoading, setApiKeysLoading] = useState(false);

    const [apiKeyCreateOpen, setApiKeyCreateOpen] = useState(false);
    const [apiKeyName, setApiKeyName] = useState("");
    const [createdApiKey, setCreatedApiKey] = useState<string | null>(null);

    const [apiKeyEditOpen, setApiKeyEditOpen] = useState(false);
    const [editingApiKey, setEditingApiKey] = useState<ApiKey | null>(null);
    const [apiKeyEditName, setApiKeyEditName] = useState("");

    useEffect(() => {
        setApiKeysLoading(true);
        fetch(apiUrl("api_keys"))
            .then((r) => (r.ok ? r.json() : Promise.reject()))
            .then((rows: ApiKey[]) => setApiKeys(rows))
            .catch(() => null)
            .finally(() => setApiKeysLoading(false));
    }, [refreshTick]);

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

    const revokeApiKey = async (id: string) => {
        await fetch(apiUrl(`api_keys/${id}`), { method: "DELETE" });
        setRefreshTick((s) => s + 1);
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

    const apiKeyColumns: DataTableColumn<ApiKey>[] = [
        { id: "name", label: "Name", width: 260, cell: ({ row }) => <span className="font-medium">{row.name}</span> },
        { id: "prefix", label: "Prefix", width: 160, cell: ({ row }) => <span className="font-mono text-xs">{row.key_prefix}</span> },
        {
            id: "created_at",
            label: "Created",
            width: 180,
            cell: ({ row }) => <span className="font-mono text-xs">{new Date(row.created_at).toISOString().slice(0, 19).replace("T", " ")}</span>,
        },
        {
            id: "status",
            label: "Status",
            width: 120,
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
            cell: ({ row }) => (
                <div className="flex justify-end gap-2" onClick={(e) => e.stopPropagation()}>
                    <Button variant="outline" size="sm" onClick={() => openRename(row)}>
                        <Pencil className="h-4 w-4 mr-2" />
                        Modifier
                    </Button>
                    <Button variant="destructive" size="sm" onClick={() => revokeApiKey(row.id)} disabled={!!row.revoked_at}>
                        <Trash2 className="h-4 w-4 mr-2" />
                        Revoke
                    </Button>
                </div>
            ),
        },
    ];

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
                    <VirtualizedDataTable<ApiKey>
                        listId="api_keys:list"
                        title="API Keys"
                        dataKey={String(refreshTick)}
                        leftMeta={apiKeysLoading ? <span className="text-sm text-muted-foreground">Loading…</span> : undefined}
                        autoHeight={true}
                        height={420}
                        rowHeight={52}
                        columns={apiKeyColumns}
                        rows={apiKeys}
                    />
                </CardContent>
            </Card>

            {/* Create API Key dialog (shows secret once) */}
            <Dialog open={apiKeyCreateOpen} onOpenChange={(open) => { setApiKeyCreateOpen(open); if (!open) { setCreatedApiKey(null); setApiKeyName(""); } }}>
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
                                    onChange={(e) => setApiKeyName(e.target.value)}
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
                                    <CopyButton text={createdApiKey} />
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
            <Dialog open={apiKeyEditOpen} onOpenChange={(open) => { setApiKeyEditOpen(open); if (!open) { setEditingApiKey(null); setApiKeyEditName(""); } }}>
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
                                onChange={(e) => setApiKeyEditName(e.target.value)}
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
        </div>
    );
}


