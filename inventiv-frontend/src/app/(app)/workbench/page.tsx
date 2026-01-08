"use client";

import { useEffect, useMemo, useRef, useState, type ChangeEvent, type KeyboardEvent } from "react";
import { Card, CardContent } from "@/components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { IACopyButton } from "ia-widgets";
import { Button } from "@/components/ui/button";
import Link from "next/link";
import { apiUrl } from "@/lib/api";
import type { ApiKey, WorkbenchRun, WorkbenchRunWithMessages } from "@/lib/types";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { ScrollArea } from "@/components/ui/scroll-area";
import { AIToggle } from "ia-widgets";

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
  const abortRef = useRef<AbortController | null>(null);

  const [streamingEnabled, setStreamingEnabled] = useState(true);
  const [streamingNow, setStreamingNow] = useState(false);
  const [ttftMs, setTtftMs] = useState<number | null>(null);
  const [lastDurationMs, setLastDurationMs] = useState<number | null>(null);

  const [runs, setRuns] = useState<WorkbenchRun[]>([]);
  const [runsError, setRunsError] = useState<string | null>(null);
  const [runId, setRunId] = useState<string>("");

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

  const stopStreaming = async () => {
    abortRef.current?.abort();
    abortRef.current = null;
    setStreamingNow(false);
  };

  const loadRuns = async () => {
    setRunsError(null);
    try {
      const res = await fetch(apiUrl("workbench/runs?limit=30"), { cache: "no-store" });
      if (!res.ok) {
        const body = await res.json().catch(() => null);
        throw new Error(body?.message || body?.error || `http_${res.status}`);
      }
      const data = (await res.json()) as WorkbenchRun[];
      setRuns(data);
    } catch (e) {
      setRuns([]);
      setRunsError(e instanceof Error ? e.message : String(e));
    }
  };

  const loadRun = async (id: string) => {
    setChatError(null);
    try {
      const res = await fetch(apiUrl(`workbench/runs/${id}`), { cache: "no-store" });
      if (!res.ok) {
        const body = await res.json().catch(() => null);
        throw new Error(body?.message || body?.error || `http_${res.status}`);
      }
      const data = (await res.json()) as WorkbenchRunWithMessages;
      setRunId(data.run.id);
      setSelectedModel(data.run.model_id);
      setMessages(
        (data.messages || []).map((m) => ({
          role: m.role as ChatMsg["role"],
          content: m.content,
        }))
      );
    } catch (e) {
      setChatError(e instanceof Error ? e.message : String(e));
    }
  };

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
      const ids: string[] = (Array.isArray(body?.data) ? body.data : [])
        .map((x: unknown) => (x && typeof x === "object" ? (x as { id?: unknown }).id : undefined))
        .filter((x: unknown): x is string => typeof x === "string" && x.trim().length > 0);
      setModels(ids);
      if (ids.length > 0) setSelectedModel(ids[0]);
      if (ids.length === 0) setModelsError("Aucun modèle disponible (aucun worker READY).");
    } catch (e) {
      setModelsError(e instanceof Error ? e.message : String(e));
    }
  };

  const ensureRun = async (modelId: string): Promise<string> => {
    if (runId) return runId;
    const res = await fetch(apiUrl("workbench/runs"), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        model_id: modelId,
        api_key_id: selectedKeyId || null,
        mode: "chat",
        metadata: { ui: "workbench", streaming_enabled: streamingEnabled },
      }),
    });
    const body = await res.json().catch(() => null);
    if (!res.ok) {
      throw new Error(body?.message || body?.error || `http_${res.status}`);
    }
    const id = body?.run?.id as string;
    if (!id) throw new Error("run_create_failed");
    setRunId(id);
    return id;
  };

  const appendMessage = async (rid: string, messageIndex: number, role: ChatMsg["role"], content: string) => {
    await fetch(apiUrl(`workbench/runs/${rid}/messages`), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        message_index: messageIndex,
        role,
        content,
      }),
    }).catch(() => null);
  };

  const completeRun = async (rid: string, status: "success" | "failed" | "cancelled", meta?: Record<string, unknown>) => {
    await fetch(apiUrl(`workbench/runs/${rid}/complete`), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        status,
        ttft_ms: ttftMs,
        duration_ms: lastDurationMs,
        error_message: status === "failed" ? chatError : null,
        metadata: meta || {},
      }),
    }).catch(() => null);
  };

  const sendChat = async () => {
    setChatError(null);
    setTtftMs(null);
    setLastDurationMs(null);
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
      const startedAt = performance.now();
      const nextMsgs = [...messages, { role: "user" as const, content: p }];
      setMessages(nextMsgs);
      setPrompt("");

      const rid = await ensureRun(selectedModel);
      // Persist system message once (best-effort)
      if (nextMsgs.length === 2 && nextMsgs[0]?.role === "system") {
        await appendMessage(rid, 0, "system", nextMsgs[0].content);
      }
      await appendMessage(rid, nextMsgs.length - 1, "user", p);

      if (!streamingEnabled) {
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
        const dur = Math.max(0, Math.round(performance.now() - startedAt));
        setLastDurationMs(dur);
        setMessages((prev) => [...prev, { role: "assistant", content: String(content) }]);
        await appendMessage(rid, nextMsgs.length, "assistant", String(content));
        await completeRun(rid, "success", { stream: false });
        void loadRuns();
        return;
      }

      // Streaming (SSE)
      setStreamingNow(true);
      const ac = new AbortController();
      abortRef.current = ac;

      // Pre-create assistant slot in UI
      setMessages((prev) => [...prev, { role: "assistant", content: "" }]);

      const res = await fetch(apiUrl("v1/chat/completions"), {
        method: "POST",
        headers: {
          Authorization: authHeader,
          "Content-Type": "application/json",
          // Stable session id for sticky routing (optional).
          "X-Inventiv-Session": rid,
        },
        body: JSON.stringify({
          model: selectedModel,
          messages: nextMsgs.map((m) => ({ role: m.role, content: m.content })),
          stream: true,
        }),
        signal: ac.signal,
      });

      if (!res.ok || !res.body) {
        const body = await res.json().catch(() => null);
        throw new Error(body?.message || body?.error || `http_${res.status}`);
      }

      const reader = res.body.getReader();
      const decoder = new TextDecoder("utf-8");
      let buf = "";
      let assistant = "";
      let firstTokenAt: number | null = null;

      const flushDelta = (delta: string) => {
        if (!delta) return;
        if (firstTokenAt == null) {
          firstTokenAt = performance.now();
          const ttft = Math.max(0, Math.round(firstTokenAt - startedAt));
          setTtftMs(ttft);
        }
        assistant += delta;
        setMessages((prev) => {
          if (prev.length === 0) return prev;
          const out = [...prev];
          const last = out[out.length - 1];
          if (!last || last.role !== "assistant") return out;
          out[out.length - 1] = { ...last, content: assistant };
          return out;
        });
      };

      const parseSseLine = (line: string) => {
        // We only care about "data:" lines.
        if (!line.startsWith("data:")) return;
        const payload = line.slice(5).trim();
        if (!payload) return;
        if (payload === "[DONE]") return;
        try {
          const obj = JSON.parse(payload);
          const delta =
            obj?.choices?.[0]?.delta?.content ??
            obj?.choices?.[0]?.message?.content ??
            obj?.choices?.[0]?.text ??
            "";
          if (typeof delta === "string" && delta.length) flushDelta(delta);
        } catch {
          // ignore parse errors (some implementations may send non-JSON lines)
        }
      };

      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        buf += decoder.decode(value, { stream: true });
        // SSE frames separated by \n\n; but we handle line-by-line for robustness.
        const lines = buf.split("\n");
        buf = lines.pop() ?? "";
        for (const raw of lines) {
          const line = raw.replace(/\r$/, "");
          parseSseLine(line);
        }
      }

      const dur = Math.max(0, Math.round(performance.now() - startedAt));
      setLastDurationMs(dur);
      setStreamingNow(false);
      abortRef.current = null;

      // Persist assistant message + complete run (best-effort)
      await appendMessage(rid, nextMsgs.length, "assistant", assistant || "(empty)");
      await completeRun(rid, "success", { stream: true });
      void loadRuns();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      // Abort is a user action -> cancelled
      if (msg.toLowerCase().includes("abort")) {
        setChatError("Annulé");
        if (runId) {
          await completeRun(runId, "cancelled", { stream: true, cancelled: true });
          void loadRuns();
        }
      } else {
        setChatError(msg);
        if (runId) {
          await completeRun(runId, "failed", { stream: streamingEnabled });
          void loadRuns();
        }
      }
    } finally {
      setSending(false);
      setStreamingNow(false);
      abortRef.current = null;
    }
  };

  useEffect(() => {
    void loadApiKeys();
    void loadRuns();
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
        <CardContent className="p-6 space-y-3">
          <div className="flex items-start justify-between gap-3">
            <div>
              <div className="text-sm font-medium">Historique (persisté)</div>
              <div className="text-xs text-muted-foreground">
                Conversations sauvegardées côté serveur (runs + messages). Clique pour recharger.
              </div>
            </div>
            <Button variant="outline" size="sm" onClick={loadRuns}>
              Recharger
            </Button>
          </div>
          {runsError ? <div className="text-xs text-red-600">{runsError}</div> : null}
          <div className="grid gap-2 md:grid-cols-2">
            <div className="rounded border bg-muted/20">
              <ScrollArea className="h-[160px]">
                <div className="p-3 space-y-2">
                  {runs.length === 0 ? (
                    <div className="text-xs text-muted-foreground">Aucun run.</div>
                  ) : (
                    runs.map((r) => (
                      <Button
                        key={r.id}
                        type="button"
                        variant="outline"
                        className="w-full justify-start h-auto py-2 px-3"
                        onClick={() => void loadRun(r.id)}
                      >
                        <div className="text-left">
                          <div className="text-xs font-medium truncate">{r.model_id}</div>
                          <div className="text-[11px] text-muted-foreground">
                            {new Date(r.created_at).toLocaleString()} · {r.status}
                            {typeof r.ttft_ms === "number" ? ` · TTFT ${r.ttft_ms}ms` : ""}
                          </div>
                        </div>
                      </Button>
                    ))
                  )}
                </div>
              </ScrollArea>
            </div>
            <div className="text-xs text-muted-foreground space-y-1">
              <div>
                Run courant: <code className="font-mono">{runId || "—"}</code>
              </div>
              <div>
                Streaming: <code className="font-mono">{streamingEnabled ? "on" : "off"}</code>
              </div>
              <div>
                TTFT: <code className="font-mono">{ttftMs ?? "—"}ms</code> · Durée:{" "}
                <code className="font-mono">{lastDurationMs ?? "—"}ms</code>
              </div>
              <div>
                Docs: <Link className="underline" href="/swagger-ui">Swagger</Link>
              </div>
            </div>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardContent className="p-6 space-y-2">
          <div className="text-sm font-medium">Base URL (OpenAI compatible)</div>
          <div className="flex items-center gap-2">
            <code className="text-xs bg-muted px-2 py-1 rounded border flex-1 overflow-x-auto">
              {baseUrl || "—"}
            </code>
            {baseUrl ? <IACopyButton text={baseUrl} /> : null}
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
                onChange={(e: ChangeEvent<HTMLInputElement>) => setApiKeyValue(e.target.value)}
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
                {apiKeyValue.trim() ? <IACopyButton text={apiKeyValue.trim()} /> : null}
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

          <div className="flex flex-wrap items-center gap-3">
            <div className="flex items-center gap-2">
              <AIToggle
                checked={streamingEnabled}
                onCheckedChange={setStreamingEnabled}
                aria-label="Activer le streaming"
              />
              <div className="text-sm">Streaming</div>
            </div>
            {streamingNow ? (
              <Button type="button" variant="destructive" size="sm" onClick={() => void stopStreaming()}>
                Stop
              </Button>
            ) : null}
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
                onChange={(e: ChangeEvent<HTMLInputElement>) => setPrompt(e.target.value)}
                placeholder="Écris un prompt…"
                onKeyDown={(e: KeyboardEvent<HTMLInputElement>) => {
                  if (e.key === "Enter" && !e.shiftKey) {
                    e.preventDefault();
                    void sendChat();
                  }
                }}
                disabled={sending || streamingNow}
              />
            </div>
            <Button onClick={sendChat} disabled={sending || streamingNow || !prompt.trim() || !apiKeyValue.trim() || !selectedModel}>
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
                <IACopyButton text={curl} />
              </div>
              <pre className="text-xs bg-muted rounded border p-3 overflow-x-auto">{curl}</pre>
            </TabsContent>

            <TabsContent value="python" className="mt-4 space-y-2">
              <div className="flex items-center justify-between">
                <div className="text-sm font-medium">Exemple Python (SDK OpenAI)</div>
                <IACopyButton text={python} />
              </div>
              <pre className="text-xs bg-muted rounded border p-3 overflow-x-auto">{python}</pre>
            </TabsContent>

            <TabsContent value="js" className="mt-4 space-y-2">
              <div className="flex items-center justify-between">
                <div className="text-sm font-medium">Exemple JS/TS (SDK OpenAI)</div>
                <IACopyButton text={js} />
              </div>
              <pre className="text-xs bg-muted rounded border p-3 overflow-x-auto">{js}</pre>
            </TabsContent>
          </Tabs>
        </CardContent>
      </Card>
    </div>
  );
}


