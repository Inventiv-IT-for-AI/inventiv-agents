"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import { Card, CardContent } from "@/components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { CopyButton } from "@/components/shared/CopyButton";
import { Button } from "@/components/ui/button";
import Link from "next/link";
import { apiUrl } from "@/lib/api";
import type { ApiKey } from "@/lib/types";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { ScrollArea } from "@/components/ui/scroll-area";

export default function WorkbenchPage() {
  const baseUrl = useMemo(() => {
    if (typeof window === "undefined") return "";
    // In local dev/staging/prod we recommend going through the UI proxy.
    return `${window.location.origin}/api/backend/v1`;
  }, []);

  const [apiKeys, setApiKeys] = useState<ApiKey[]>([]);
  const [selectedKeyId, setSelectedKeyId] = useState<string>("");
  const [apiKeyValue, setApiKeyValue] = useState<string>("");
  const [apiKeyError, setApiKeyError] = useState<string | null>(null);

  const [models, setModels] = useState<string[]>([]);
  const [selectedModel, setSelectedModel] = useState<string>("");
  const [modelsError, setModelsError] = useState<string | null>(null);

  type ChatMsg = { role: "system" | "user" | "assistant"; content: string };
  const [messages, setMessages] = useState<ChatMsg[]>([
    { role: "system", content: "You are a helpful assistant." },
  ]);
  const [prompt, setPrompt] = useState("");
  const [sending, setSending] = useState(false);
  const [chatError, setChatError] = useState<string | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  const storageKey = useMemo(() => {
    if (!selectedKeyId) return null;
    return `workbench:api_key:${selectedKeyId}`;
  }, [selectedKeyId]);

  const selectedKeyLabel = useMemo(() => {
    const k = apiKeys.find((x) => x.id === selectedKeyId);
    if (!k) return null;
    return `${k.name} (${k.key_prefix}…)`;
  }, [apiKeys, selectedKeyId]);

  const authHeader = useMemo(() => {
    const v = apiKeyValue.trim();
    if (!v) return null;
    return `Bearer ${v}`;
  }, [apiKeyValue]);

  const loadApiKeys = async () => {
    setApiKeyError(null);
    try {
      const res = await fetch(apiUrl("api_keys"), { cache: "no-store" });
      if (!res.ok) {
        const body = await res.json().catch(() => null);
        throw new Error(body?.message || body?.error || `http_${res.status}`);
      }
      const data = (await res.json()) as ApiKey[];
      setApiKeys(data);
      if (!selectedKeyId && data.length > 0) setSelectedKeyId(data[0].id);
    } catch (e) {
      setApiKeys([]);
      setApiKeyError(e instanceof Error ? e.message : String(e));
    }
  };

  const rememberKeyLocally = () => {
    if (!storageKey) return;
    try {
      window.localStorage.setItem(storageKey, apiKeyValue);
    } catch {
      // ignore
    }
  };

  const forgetKeyLocally = () => {
    if (!storageKey) return;
    try {
      window.localStorage.removeItem(storageKey);
    } catch {
      // ignore
    }
  };

  const fetchModels = async () => {
    setModelsError(null);
    setModels([]);
    setSelectedModel("");
    if (!authHeader) {
      setModelsError("API_KEY manquante");
      return;
    }
    try {
      const res = await fetch(apiUrl("v1/models"), {
        method: "GET",
        headers: { Authorization: authHeader },
        cache: "no-store",
      });
      const body = await res.json().catch(() => null);
      if (!res.ok) {
        throw new Error(body?.message || body?.error || `http_${res.status}`);
      }
      const ids: string[] = (body?.data || [])
        .map((x: any) => x?.id)
        .filter((x: any) => typeof x === "string" && x.trim().length > 0);
      setModels(ids);
      if (ids.length > 0) setSelectedModel(ids[0]);
      if (ids.length === 0) setModelsError("Aucun modèle disponible (aucun worker READY).");
    } catch (e) {
      setModelsError(e instanceof Error ? e.message : String(e));
    }
  };

  const sendChat = async () => {
    setChatError(null);
    const p = prompt.trim();
    if (!p) return;
    if (!authHeader) {
      setChatError("API_KEY manquante");
      return;
    }
    if (!selectedModel) {
      setChatError("Model manquant");
      return;
    }
    setSending(true);
    try {
      const nextMsgs = [...messages, { role: "user" as const, content: p }];
      setMessages(nextMsgs);
      setPrompt("");

      const res = await fetch(apiUrl("v1/chat/completions"), {
        method: "POST",
        headers: {
          Authorization: authHeader,
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          model: selectedModel,
          messages: nextMsgs.map((m) => ({ role: m.role, content: m.content })),
          stream: false,
        }),
      });
      const body = await res.json().catch(() => null);
      if (!res.ok) {
        throw new Error(body?.message || body?.error || `http_${res.status}`);
      }
      const content =
        body?.choices?.[0]?.message?.content ??
        body?.choices?.[0]?.text ??
        "";
      setMessages((prev) => [...prev, { role: "assistant", content: String(content) }]);
    } catch (e) {
      setChatError(e instanceof Error ? e.message : String(e));
    } finally {
      setSending(false);
    }
  };

  useEffect(() => {
    void loadApiKeys();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (!storageKey) return;
    try {
      const v = window.localStorage.getItem(storageKey) ?? "";
      setApiKeyValue(v);
    } catch {
      setApiKeyValue("");
    }
  }, [storageKey]);

  useEffect(() => {
    // auto-scroll chat
    const el = scrollRef.current;
    if (!el) return;
    el.scrollTop = el.scrollHeight;
  }, [messages, sending]);

  const curl = useMemo(() => {
    const u = baseUrl || "http://<ui-host>/api/backend/v1";
    return [
      `# List models`,
      `curl -sS "${u}/models" \\`,
      `  -H "Authorization: Bearer <API_KEY>"`,
      ``,
      `# Chat completion`,
      `curl -sS "${u}/chat/completions" \\`,
      `  -H "Authorization: Bearer <API_KEY>" \\`,
      `  -H "Content-Type: application/json" \\`,
      `  -d '{`,
      `    "model": "<MODEL_ID>",`,
      `    "messages": [{"role":"user","content":"Hello!"}],`,
      `    "max_tokens": 64`,
      `  }'`,
    ].join("\n");
  }, [baseUrl]);

  const python = useMemo(() => {
    const u = baseUrl || "http://<ui-host>/api/backend/v1";
    return [
      `from openai import OpenAI`,
      ``,
      `client = OpenAI(`,
      `    base_url="${u}",`,
      `    api_key="<API_KEY>",`,
      `)`,
      ``,
      `print(client.models.list())`,
      ``,
      `resp = client.chat.completions.create(`,
      `    model="<MODEL_ID>",`,
      `    messages=[{"role": "user", "content": "Hello!"}],`,
      `    max_tokens=64,`,
      `)`,
      `print(resp.choices[0].message.content)`,
    ].join("\n");
  }, [baseUrl]);

  const js = useMemo(() => {
    const u = baseUrl || "http://<ui-host>/api/backend/v1";
    return [
      `import OpenAI from "openai";`,
      ``,
      `const client = new OpenAI({`,
      `  baseURL: "${u}",`,
      `  apiKey: "<API_KEY>",`,
      `});`,
      ``,
      `console.log(await client.models.list());`,
      ``,
      `const resp = await client.chat.completions.create({`,
      `  model: "<MODEL_ID>",`,
      `  messages: [{ role: "user", content: "Hello!" }],`,
      `  max_tokens: 64,`,
      `});`,
      `console.log(resp.choices[0]?.message?.content);`,
    ].join("\n");
  }, [baseUrl]);

  return (
    <div className="p-8 space-y-6">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Workbench</h1>
          <p className="text-muted-foreground">
            Endpoint OpenAI compatible + exemples de code pour intégrer tes modèles.
          </p>
        </div>
        <Button asChild variant="secondary">
          <Link href="/settings?tab=api_keys">Gérer les API Keys</Link>
        </Button>
      </div>

      <Card>
        <CardContent className="p-6 space-y-2">
          <div className="text-sm font-medium">Base URL (OpenAI compatible)</div>
          <div className="flex items-center gap-2">
            <code className="text-xs bg-muted px-2 py-1 rounded border flex-1 overflow-x-auto">
              {baseUrl || "—"}
            </code>
            {baseUrl ? <CopyButton text={baseUrl} /> : null}
          </div>
          <div className="text-xs text-muted-foreground">
            Auth: <code className="font-mono">Authorization: Bearer &lt;API_KEY&gt;</code> (ou{" "}
            <code className="font-mono">X-API-Key</code>).
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardContent className="p-6 space-y-4">
          <div className="flex items-start justify-between gap-3">
            <div>
              <div className="text-sm font-medium">API Key (pour tester)</div>
              <div className="text-xs text-muted-foreground">
                La clé en clair n’est jamais renvoyée par le serveur après création. Tu peux la coller ici (option: mémoriser en local).
              </div>
            </div>
            <Button variant="outline" size="sm" onClick={loadApiKeys}>
              Recharger
            </Button>
          </div>

          {apiKeyError ? (
            <div className="text-xs text-red-600">{apiKeyError}</div>
          ) : null}

          <div className="grid gap-3 md:grid-cols-[1fr_1fr]">
            <div className="grid gap-2">
              <Label>Choisir une clé</Label>
              <Select value={selectedKeyId} onValueChange={setSelectedKeyId}>
                <SelectTrigger>
                  <SelectValue placeholder="Sélectionner une clé" />
                </SelectTrigger>
                <SelectContent>
                  {apiKeys.map((k) => (
                    <SelectItem key={k.id} value={k.id}>
                      {k.name} — {k.key_prefix}…
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <div className="text-xs text-muted-foreground">
                {selectedKeyLabel ? `Sélection: ${selectedKeyLabel}` : "—"}
              </div>
            </div>

            <div className="grid gap-2">
              <Label>API Key (plaintext)</Label>
              <Input
                value={apiKeyValue}
                onChange={(e) => setApiKeyValue(e.target.value)}
                placeholder="sk-inv-..."
                autoComplete="off"
              />
              <div className="flex items-center gap-2">
                <Button variant="secondary" size="sm" onClick={rememberKeyLocally} disabled={!apiKeyValue.trim() || !selectedKeyId}>
                  Mémoriser (local)
                </Button>
                <Button variant="outline" size="sm" onClick={forgetKeyLocally} disabled={!selectedKeyId}>
                  Oublier
                </Button>
                {apiKeyValue.trim() ? <CopyButton text={apiKeyValue.trim()} /> : null}
              </div>
            </div>
          </div>

          <div className="flex items-center gap-2">
            <Button onClick={fetchModels} disabled={!apiKeyValue.trim()}>
              Charger les modèles (GET /v1/models)
            </Button>
            {modelsError ? <span className="text-xs text-red-600">{modelsError}</span> : null}
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardContent className="p-6 space-y-4">
          <div className="flex items-start justify-between gap-3">
            <div>
              <div className="text-sm font-medium">Chat (OpenAI compatible)</div>
              <div className="text-xs text-muted-foreground">
                Envoie des messages via <code className="font-mono">POST /v1/chat/completions</code>.
              </div>
            </div>
          </div>

          <div className="grid gap-2">
            <Label>Model</Label>
            <Select value={selectedModel} onValueChange={setSelectedModel} disabled={models.length === 0}>
              <SelectTrigger>
                <SelectValue placeholder={models.length ? "Choisir un modèle" : "Charger les modèles d’abord"} />
              </SelectTrigger>
              <SelectContent>
                {models.map((m) => (
                  <SelectItem key={m} value={m}>
                    {m}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div className="rounded border bg-muted/20">
            <ScrollArea className="h-[340px]">
              <div className="p-3 space-y-3" ref={scrollRef}>
                {messages.map((m, idx) => (
                  <div key={idx} className="text-sm">
                    <div className="text-[11px] uppercase tracking-wide text-muted-foreground mb-1">
                      {m.role}
                    </div>
                    <div className="whitespace-pre-wrap">{m.content}</div>
                  </div>
                ))}
                {sending ? (
                  <div className="text-xs text-muted-foreground">…</div>
                ) : null}
              </div>
            </ScrollArea>
          </div>

          {chatError ? <div className="text-xs text-red-600">{chatError}</div> : null}

          <div className="flex items-end gap-2">
            <div className="flex-1">
              <Label>Message</Label>
              <Input
                value={prompt}
                onChange={(e) => setPrompt(e.target.value)}
                placeholder="Écris un prompt…"
                onKeyDown={(e) => {
                  if (e.key === "Enter" && !e.shiftKey) {
                    e.preventDefault();
                    void sendChat();
                  }
                }}
                disabled={sending}
              />
            </div>
            <Button onClick={sendChat} disabled={sending || !prompt.trim() || !apiKeyValue.trim() || !selectedModel}>
              Envoyer
            </Button>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardContent className="p-6">
          <Tabs defaultValue="curl">
            <TabsList>
              <TabsTrigger value="curl">curl</TabsTrigger>
              <TabsTrigger value="python">Python</TabsTrigger>
              <TabsTrigger value="js">JS/TS</TabsTrigger>
            </TabsList>

            <TabsContent value="curl" className="mt-4 space-y-2">
              <div className="flex items-center justify-between">
                <div className="text-sm font-medium">Exemples curl</div>
                <CopyButton text={curl} />
              </div>
              <pre className="text-xs bg-muted rounded border p-3 overflow-x-auto">{curl}</pre>
            </TabsContent>

            <TabsContent value="python" className="mt-4 space-y-2">
              <div className="flex items-center justify-between">
                <div className="text-sm font-medium">Exemple Python (SDK OpenAI)</div>
                <CopyButton text={python} />
              </div>
              <pre className="text-xs bg-muted rounded border p-3 overflow-x-auto">{python}</pre>
            </TabsContent>

            <TabsContent value="js" className="mt-4 space-y-2">
              <div className="flex items-center justify-between">
                <div className="text-sm font-medium">Exemple JS/TS (SDK OpenAI)</div>
                <CopyButton text={js} />
              </div>
              <pre className="text-xs bg-muted rounded border p-3 overflow-x-auto">{js}</pre>
            </TabsContent>
          </Tabs>
        </CardContent>
      </Card>
    </div>
  );
}


