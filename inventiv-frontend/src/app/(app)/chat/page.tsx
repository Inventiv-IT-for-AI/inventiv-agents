"use client";

import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { apiUrl } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { ScrollArea } from "@/components/ui/scroll-area";
import type { WorkbenchProject, WorkbenchRun, WorkbenchRunWithMessages } from "@/lib/types";
import { WorkspaceBanner } from "@/components/shared/WorkspaceBanner";
import { AIToggle } from "ia-widgets";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { DropdownMenu, DropdownMenuTrigger, DropdownMenuContent, DropdownMenuItem } from "@/components/ui/dropdown-menu";
import { MoreVertical } from "lucide-react";

type ChatMsg = { role: "system" | "user" | "assistant"; content: string };

type ChatModel = {
  model: string;
  label: string;
  scope: "public" | "org" | string;
  underlying_model_id: string;
};

export default function ChatPage() {
  const [models, setModels] = useState<ChatModel[]>([]);
  const [selectedModel, setSelectedModel] = useState<string>("");
  const [modelsError, setModelsError] = useState<string | null>(null);

  const [messages, setMessages] = useState<ChatMsg[]>([{ role: "system", content: "You are a helpful assistant." }]);
  const [prompt, setPrompt] = useState("");
  const [sending, setSending] = useState(false);
  const [chatError, setChatError] = useState<string | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const abortRef = useRef<AbortController | null>(null);
  const [streamingEnabled, setStreamingEnabled] = useState(true);
  const [streamingNow, setStreamingNow] = useState(false);

  const [runs, setRuns] = useState<WorkbenchRun[]>([]);
  const [runsError, setRunsError] = useState<string | null>(null);
  const [runId, setRunId] = useState<string>("");

  const [projects, setProjects] = useState<WorkbenchProject[]>([]);
  const [projectsError, setProjectsError] = useState<string | null>(null);
  const [selectedProjectId, setSelectedProjectId] = useState<string>("__all__");
  const [createProjectOpen, setCreateProjectOpen] = useState(false);
  const [createProjectName, setCreateProjectName] = useState("");
  const [createProjectShared, setCreateProjectShared] = useState(false);

  const [renameRunOpen, setRenameRunOpen] = useState(false);
  const [renameRunId, setRenameRunId] = useState<string>("");
  const [renameRunTitle, setRenameRunTitle] = useState<string>("");
  const [moveToTopicOpen, setMoveToTopicOpen] = useState(false);
  const [moveToTopicRunId, setMoveToTopicRunId] = useState<string>("");

  const selectedModelLabel = useMemo(() => {
    const m = models.find((x) => x.model === selectedModel);
    return m ? `${m.label} (${m.model})` : selectedModel;
  }, [models, selectedModel]);

  const loadModels = async () => {
    setModelsError(null);
    try {
      const res = await fetch(apiUrl("/chat/models"), { cache: "no-store", credentials: "include" });
      if (!res.ok) {
        const body = await res.json().catch(() => null);
        throw new Error(body?.message || body?.error || `http_${res.status}`);
      }
      const data = (await res.json()) as ChatModel[];
      setModels(Array.isArray(data) ? data : []);
      if (!selectedModel && Array.isArray(data) && data.length > 0) {
        setSelectedModel(data[0].model);
      }
      if (Array.isArray(data) && data.length === 0) {
        setModelsError("Aucun modèle disponible (aucun worker READY).");
      }
    } catch (e) {
      setModels([]);
      setModelsError(e instanceof Error ? e.message : String(e));
    }
  };

  const loadRuns = async () => {
    setRunsError(null);
    try {
      const res = await fetch(apiUrl("workbench/runs?limit=30"), { cache: "no-store", credentials: "include" });
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

  const loadProjects = async () => {
    setProjectsError(null);
    try {
      const res = await fetch(apiUrl("workbench/projects"), { cache: "no-store", credentials: "include" });
      if (!res.ok) {
        const body = await res.json().catch(() => null);
        throw new Error(body?.message || body?.error || `http_${res.status}`);
      }
      const data = (await res.json()) as WorkbenchProject[];
      setProjects(Array.isArray(data) ? data : []);
    } catch (e) {
      setProjects([]);
      setProjectsError(e instanceof Error ? e.message : String(e));
    }
  };

  const loadRun = async (id: string) => {
    setChatError(null);
    try {
      const res = await fetch(apiUrl(`workbench/runs/${id}`), { cache: "no-store", credentials: "include" });
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

  const ensureRun = async (modelId: string): Promise<string> => {
    if (runId) return runId;
    const res = await fetch(apiUrl("workbench/runs"), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      credentials: "include",
      body: JSON.stringify({
        model_id: modelId,
        api_key_id: null,
        mode: "chat",
        metadata: { ui: "chat" },
      }),
    });
    const body = await res.json().catch(() => null);
    if (!res.ok) throw new Error(body?.message || body?.error || `http_${res.status}`);
    const id = body?.run?.id as string;
    if (!id) throw new Error("run_create_failed");
    setRunId(id);
    return id;
  };

  const appendMessage = async (rid: string, messageIndex: number, role: ChatMsg["role"], content: string) => {
    await fetch(apiUrl(`workbench/runs/${rid}/messages`), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      credentials: "include",
      body: JSON.stringify({ message_index: messageIndex, role, content }),
    }).catch(() => null);
  };

  const completeRun = async (rid: string, status: "success" | "failed" | "cancelled", meta?: Record<string, unknown>) => {
    await fetch(apiUrl(`workbench/runs/${rid}/complete`), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      credentials: "include",
      body: JSON.stringify({
        status,
        ttft_ms: null,
        duration_ms: null,
        error_message: status === "failed" ? chatError : null,
        metadata: meta || {},
      }),
    }).catch(() => null);
  };

  const scrollToBottom = () => {
    const el = scrollRef.current;
    if (!el) return;
    el.scrollTop = el.scrollHeight;
  };

  useEffect(() => {
    void loadModels().catch(() => null);
    void loadRuns().catch(() => null);
    void loadProjects().catch(() => null);
  }, []);

  useEffect(() => {
    scrollToBottom();
  }, [messages.length]);

  const stop = () => {
    abortRef.current?.abort();
    abortRef.current = null;
    setStreamingNow(false);
  };

  const send = async () => {
    setChatError(null);
    const p = prompt.trim();
    if (!p) return;
    if (!selectedModel) {
      setChatError("Modèle manquant");
      return;
    }
    if (sending) return;
    setSending(true);
    const controller = new AbortController();
    abortRef.current = controller;

    const nextMessages: ChatMsg[] = [...messages, { role: "user", content: p }];
    setPrompt("");
    setMessages(nextMessages);

    const rid = await ensureRun(selectedModel);
    await appendMessage(rid, nextMessages.length - 1, "user", p);

    // Declare variables before try block so they're accessible in catch (for streaming mode)
    let requestStartTime: number | undefined;
    let chunkCount = 0;
    let sseLineCount = 0;

    try {
      if (!streamingEnabled) {
        const res = await fetch(apiUrl("/v1/chat/completions"), {
          method: "POST",
          headers: { "Content-Type": "application/json", "X-Inventiv-Session": rid },
          credentials: "include",
          body: JSON.stringify({
            model: selectedModel,
            stream: false,
            messages: nextMessages.map((m) => ({ role: m.role, content: m.content })),
          }),
          signal: controller.signal,
        });
        const body = await res.json().catch(() => null);
        if (!res.ok) throw new Error(body?.message || body?.error || `http_${res.status}`);
        const content = body?.choices?.[0]?.message?.content ?? body?.choices?.[0]?.text ?? "";
        const assistantMsg: ChatMsg = { role: "assistant", content: String(content) };
        setMessages((prev) => [...prev, assistantMsg]);
        await appendMessage(rid, nextMessages.length, "assistant", assistantMsg.content);
        await completeRun(rid, "success", { stream: false, model: selectedModel });
        void loadRuns().catch(() => null);
        return;
      }

      // Streaming (SSE)
      setStreamingNow(true);
      // Pre-create assistant slot in UI
      setMessages((prev) => [...prev, { role: "assistant", content: "" }]);

      // Initialize variables for streaming mode
      requestStartTime = performance.now();
      chunkCount = 0;
      sseLineCount = 0;
      
      console.log(`[CHAT] [${rid}] REQUEST_START: model=${selectedModel}, message_count=${nextMessages.length}, stream=true`);
      
      const res = await fetch(apiUrl("/v1/chat/completions"), {
        method: "POST",
        headers: { "Content-Type": "application/json", "X-Inventiv-Session": rid },
        credentials: "include",
        body: JSON.stringify({
          model: selectedModel,
          stream: true,
          messages: nextMessages.map((m) => ({ role: m.role, content: m.content })),
        }),
        signal: controller.signal,
      });

      const requestElapsed = performance.now() - requestStartTime;
      console.log(`[CHAT] [${rid}] REQUEST_RESPONSE: status=${res.status}, elapsed_ms=${Math.round(requestElapsed)}`);

      if (!res.ok || !res.body) {
        const body = await res.json().catch(() => null);
        console.error(`[CHAT] [${rid}] REQUEST_ERROR: status=${res.status}, error=${body?.error || body?.message || 'unknown'}`);
        throw new Error(body?.message || body?.error || `http_${res.status}`);
      }

      const reader = res.body.getReader();
      const decoder = new TextDecoder("utf-8");
      let buf = "";
      let assistant = "";

      const flushDelta = (delta: string) => {
        if (!delta) return;
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
        if (!line.startsWith("data:")) return;
        sseLineCount++;
        const payload = line.slice(5).trim();
        if (!payload) return;
        if (payload === "[DONE]") {
          console.log(`[CHAT] [${rid}] SSE_DONE: total_lines=${sseLineCount}, final_length=${assistant.length}`);
          return;
        }
        try {
          const obj = JSON.parse(payload);
          const delta =
            obj?.choices?.[0]?.delta?.content ??
            obj?.choices?.[0]?.message?.content ??
            obj?.choices?.[0]?.text ??
            "";
          if (typeof delta === "string" && delta.length) {
            if (sseLineCount <= 3) {
              console.log(`[CHAT] [${rid}] SSE_CHUNK: line=${sseLineCount}, delta_length=${delta.length}, delta_preview=${delta.substring(0, 50)}`);
            }
            flushDelta(delta);
          }
        } catch (e) {
          console.warn(`[CHAT] [${rid}] SSE_PARSE_ERROR: line=${sseLineCount}, error=${e}`);
        }
      };

      console.log(`[CHAT] [${rid}] STREAM_START: reading SSE stream`);
      while (true) {
        const { done, value } = await reader.read();
        if (done) {
          console.log(`[CHAT] [${rid}] STREAM_END: chunk_count=${chunkCount}, sse_lines=${sseLineCount}, final_length=${assistant.length}`);
          break;
        }
        chunkCount++;
        if (chunkCount <= 3 || chunkCount % 10 === 0) {
          console.log(`[CHAT] [${rid}] STREAM_CHUNK: count=${chunkCount}, size=${value.length}`);
        }
        buf += decoder.decode(value, { stream: true });
        const lines = buf.split("\n");
        buf = lines.pop() ?? "";
        for (const raw of lines) parseSseLine(raw.replace(/\r$/, ""));
      }

      const totalElapsed = performance.now() - requestStartTime;
      console.log(`[CHAT] [${rid}] REQUEST_COMPLETE: total_ms=${Math.round(totalElapsed)}, chunks=${chunkCount}, sse_lines=${sseLineCount}, response_length=${assistant.length}`);

      setStreamingNow(false);
      abortRef.current = null;

      await appendMessage(rid, nextMessages.length, "assistant", assistant || "(empty)");
      await completeRun(rid, "success", { stream: true, model: selectedModel });
      void loadRuns().catch(() => null);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      const elapsed = requestStartTime ? performance.now() - requestStartTime : 0;
      console.error(`[CHAT] [${rid}] REQUEST_FAILED: elapsed_ms=${Math.round(elapsed)}, error=${msg}`);
      if (msg.toLowerCase().includes("abort")) {
        console.log(`[CHAT] [${rid}] REQUEST_CANCELLED`);
        setChatError("Annulé");
        await completeRun(rid, "cancelled", { stream: streamingEnabled, cancelled: true });
      } else {
        setChatError(msg);
        await completeRun(rid, "failed", { error: msg, stream: streamingEnabled, model: selectedModel });
      }
    } finally {
      setSending(false);
      setStreamingNow(false);
      abortRef.current = null;
    }
  };

  const onPromptKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void send();
    }
  };

  const startNewChat = () => {
    stop();
    setRunId("");
    setMessages([{ role: "system", content: "You are a helpful assistant." }]);
    setChatError(null);
  };

  const filteredRuns = useMemo(() => {
    if (selectedProjectId === "__all__") return runs;
    if (selectedProjectId === "__none__") return runs.filter((r) => !r.project_id);
    return runs.filter((r) => r.project_id === selectedProjectId);
  }, [runs, selectedProjectId]);

  const createProject = async () => {
    const name = createProjectName.trim();
    if (!name) return;
    const res = await fetch(apiUrl("workbench/projects"), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      credentials: "include",
      body: JSON.stringify({ name, shared_with_org: createProjectShared }),
    });
    if (res.ok) {
      setCreateProjectOpen(false);
      setCreateProjectName("");
      setCreateProjectShared(false);
      await loadProjects();
    }
  };

  const updateRun = async (id: string, patch: { title?: string | null; project_id?: string | null; shared_with_org?: boolean }) => {
    await fetch(apiUrl(`workbench/runs/${id}`), {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      credentials: "include",
      body: JSON.stringify(patch),
    }).catch(() => null);
    await loadRuns();
  };

  const deleteRun = async (id: string) => {
    await fetch(apiUrl(`workbench/runs/${id}`), { method: "DELETE", credentials: "include" }).catch(() => null);
    if (runId === id) startNewChat();
    await loadRuns();
  };

  return (
    <div className="p-8 space-y-6">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Chat</h1>
          <p className="text-muted-foreground">Chat via session (pas besoin d’API key).</p>
        </div>
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-2 text-sm">
            <span className="text-muted-foreground">Streaming</span>
            <AIToggle checked={streamingEnabled} onCheckedChange={(c) => setStreamingEnabled(!!c)} />
          </div>
          <Button variant="outline" onClick={loadModels}>
            Refresh models
          </Button>
          <Button variant="outline" onClick={startNewChat}>
            New chat
          </Button>
        </div>
      </div>

      <WorkspaceBanner />

      <div className="grid gap-6 lg:grid-cols-[320px_1fr]">
        <Card className="h-fit">
          <CardContent className="p-4 space-y-4">
            <div className="space-y-2">
              <Label>Modèle</Label>
              <Select value={selectedModel} onValueChange={setSelectedModel}>
                <SelectTrigger>
                  <SelectValue placeholder="Sélectionner un modèle…" />
                </SelectTrigger>
                <SelectContent>
                  {models.map((m) => (
                    <SelectItem key={m.model} value={m.model}>
                      {m.label} — {m.scope}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              {modelsError ? <div className="text-sm text-red-600">{modelsError}</div> : null}
              <div className="text-xs text-muted-foreground break-all">Selected: {selectedModelLabel}</div>
            </div>

            <div className="space-y-2">
              <div className="flex items-center justify-between gap-2">
                <Label>Topics</Label>
                <Button variant="outline" size="sm" onClick={() => setCreateProjectOpen(true)}>
                  New
                </Button>
              </div>
              <Select value={selectedProjectId} onValueChange={setSelectedProjectId}>
                <SelectTrigger>
                  <SelectValue placeholder="Filtrer…" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="__all__">Tous</SelectItem>
                  <SelectItem value="__none__">Sans topic</SelectItem>
                  {projects.map((p) => (
                    <SelectItem key={p.id} value={p.id}>
                      {p.name}
                      {p.organization_id && p.shared_with_org ? " (org)" : ""}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              {projectsError ? <div className="text-sm text-red-600">{projectsError}</div> : null}
            </div>

            <div className="space-y-2">
              <Label>Sessions</Label>
              {runsError ? <div className="text-sm text-red-600">{runsError}</div> : null}
              <div className="space-y-2">
                {filteredRuns.length === 0 ? (
                  <div className="text-sm text-muted-foreground">Aucune session.</div>
                ) : (
                  filteredRuns.map((r) => (
                    <div key={r.id} className="flex items-center gap-2 min-w-0">
                      <Button
                        variant={r.id === runId ? "secondary" : "ghost"}
                        className="flex-1 min-w-0 justify-start"
                        onClick={() => loadRun(r.id)}
                      >
                        <div className="min-w-0 text-left flex-1">
                          <div className="text-sm font-medium truncate">{r.title || r.model_id}</div>
                          <div className="text-xs text-muted-foreground truncate">
                            {new Date(r.created_at).toLocaleString()}
                            {r.organization_id && r.shared_with_org ? " · shared(org)" : ""}
                          </div>
                        </div>
                      </Button>
                      <DropdownMenu>
                        <DropdownMenuTrigger asChild>
                          <Button variant="ghost" size="sm" className="flex-shrink-0 h-8 w-8 p-0">
                            <MoreVertical className="h-4 w-4" />
                            <span className="sr-only">Open menu</span>
                          </Button>
                        </DropdownMenuTrigger>
                        <DropdownMenuContent align="end">
                          <DropdownMenuItem
                            onClick={() => {
                              setRenameRunId(r.id);
                              setRenameRunTitle(r.title || "");
                              setRenameRunOpen(true);
                            }}
                          >
                            Rename
                          </DropdownMenuItem>
                          <DropdownMenuItem
                            onClick={() => {
                              setMoveToTopicRunId(r.id);
                              setMoveToTopicOpen(true);
                            }}
                          >
                            Move to Topic
                          </DropdownMenuItem>
                          <DropdownMenuItem
                            onClick={() => void deleteRun(r.id)}
                            className="text-red-600 focus:text-red-600"
                          >
                            Delete
                          </DropdownMenuItem>
                        </DropdownMenuContent>
                      </DropdownMenu>
                    </div>
                  ))
                )}
              </div>
            </div>

            {runId ? (
              <div className="space-y-2">
                <Label>Session settings</Label>
                <div className="grid gap-2">
                  <Select
                    value={runs.find((r) => r.id === runId)?.project_id || "__none__"}
                    onValueChange={(v) => void updateRun(runId, { project_id: v === "__none__" ? null : v })}
                  >
                    <SelectTrigger>
                      <SelectValue placeholder="Affecter à un topic…" />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="__none__">Sans topic</SelectItem>
                      {projects.map((p) => (
                        <SelectItem key={p.id} value={p.id}>
                          {p.name}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>

                  <div className="flex items-center justify-between text-sm">
                    <span className="text-muted-foreground">Share with org</span>
                    <AIToggle
                      checked={!!runs.find((r) => r.id === runId)?.shared_with_org}
                      onCheckedChange={(c) => void updateRun(runId, { shared_with_org: !!c })}
                    />
                  </div>
                </div>
              </div>
            ) : null}
          </CardContent>
        </Card>

        <Card>
          <CardContent className="p-4 space-y-4">
            <ScrollArea className="h-[520px] border rounded-md">
              <div ref={scrollRef} className="p-4 space-y-3">
                {messages.map((m, idx) => (
                  <div key={idx} className="space-y-1">
                    <div className="text-xs text-muted-foreground">{m.role}</div>
                    <div className="whitespace-pre-wrap text-sm">{m.content}</div>
                  </div>
                ))}
              </div>
            </ScrollArea>

            {chatError ? <div className="text-sm text-red-600">{chatError}</div> : null}

            <div className="flex gap-2">
              <Input
                value={prompt}
                onChange={(e) => setPrompt(e.target.value)}
                onKeyDown={onPromptKeyDown}
                placeholder="Écris ton message…"
                disabled={sending}
              />
              <Button onClick={send} disabled={sending || streamingNow || !prompt.trim()}>
                {sending ? "Sending…" : streamingEnabled ? "Send (stream)" : "Send"}
              </Button>
              <Button variant="outline" onClick={stop} disabled={!sending && !streamingNow}>
                Stop
              </Button>
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Create project dialog */}
      <Dialog open={createProjectOpen} onOpenChange={setCreateProjectOpen}>
        <DialogContent showCloseButton={false} className="sm:max-w-[520px]">
          <DialogHeader>
            <DialogTitle>Nouveau topic</DialogTitle>
          </DialogHeader>
          <div className="grid gap-3 py-2">
            <div className="grid grid-cols-4 items-center gap-4">
              <Label className="text-right">Nom</Label>
              <Input className="col-span-3" value={createProjectName} onChange={(e) => setCreateProjectName(e.target.value)} />
            </div>
            <div className="flex items-center justify-between text-sm">
              <span className="text-muted-foreground">Share with org</span>
              <AIToggle checked={createProjectShared} onCheckedChange={(c) => setCreateProjectShared(!!c)} />
            </div>
          </div>
          <DialogFooter className="sm:justify-between">
            <Button variant="outline" onClick={() => setCreateProjectOpen(false)}>
              Cancel
            </Button>
            <Button onClick={() => void createProject()} disabled={!createProjectName.trim()}>
              Create
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Move to topic dialog */}
      <Dialog open={moveToTopicOpen} onOpenChange={setMoveToTopicOpen}>
        <DialogContent showCloseButton={false} className="sm:max-w-[520px]">
          <DialogHeader>
            <DialogTitle>Déplacer vers un topic</DialogTitle>
          </DialogHeader>
          <div className="grid gap-3 py-2">
            <Select
              value={runs.find((r) => r.id === moveToTopicRunId)?.project_id || "__none__"}
              onValueChange={(v) => {
                if (moveToTopicRunId) {
                  void updateRun(moveToTopicRunId, { project_id: v === "__none__" ? null : v });
                  setMoveToTopicOpen(false);
                }
              }}
            >
              <SelectTrigger>
                <SelectValue placeholder="Sélectionner un topic…" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="__none__">Sans topic</SelectItem>
                {projects.map((p) => (
                  <SelectItem key={p.id} value={p.id}>
                    {p.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <DialogFooter className="sm:justify-between">
            <Button variant="outline" onClick={() => setMoveToTopicOpen(false)}>
              Cancel
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Rename run dialog */}
      <Dialog open={renameRunOpen} onOpenChange={setRenameRunOpen}>
        <DialogContent showCloseButton={false} className="sm:max-w-[520px]">
          <DialogHeader>
            <DialogTitle>Renommer la session</DialogTitle>
          </DialogHeader>
          <div className="grid gap-3 py-2">
            <div className="grid grid-cols-4 items-center gap-4">
              <Label className="text-right">Titre</Label>
              <Input className="col-span-3" value={renameRunTitle} onChange={(e) => setRenameRunTitle(e.target.value)} />
            </div>
          </div>
          <DialogFooter className="sm:justify-between">
            <Button variant="outline" onClick={() => setRenameRunOpen(false)}>
              Cancel
            </Button>
            <Button
              onClick={() => {
                void updateRun(renameRunId, { title: renameRunTitle || null });
                setRenameRunOpen(false);
              }}
              disabled={!renameRunId}
            >
              Save
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}


