"use client";

import { useEffect, useMemo, useState, type ChangeEvent } from "react";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { RefreshCcw } from "lucide-react";
import { apiUrl } from "@/lib/api";
import type { GpuActivityResponse, Instance, SystemActivityResponse } from "@/lib/types";
import { displayOrDash } from "@/lib/utils";
import { IA_COLORS, IASparklineDual } from "ia-widgets";

function withAlpha(hex: string, alpha: number) {
  const a = Math.max(0, Math.min(1, alpha));
  const h = hex.replace("#", "").trim();
  const full = h.length === 3 ? h.split("").map((c) => c + c).join("") : h;
  const r = parseInt(full.slice(0, 2), 16);
  const g = parseInt(full.slice(2, 4), 16);
  const b = parseInt(full.slice(4, 6), 16);
  return `rgba(${r}, ${g}, ${b}, ${a})`;
}

function stableColorForInstance(instanceId: string, sortedIds: string[]) {
  const palette = [
    IA_COLORS.teal,
    IA_COLORS.blue,
    IA_COLORS.orange,
    IA_COLORS.purple,
    IA_COLORS.green,
    IA_COLORS.cyan,
    IA_COLORS.violet,
    IA_COLORS.amber,
    IA_COLORS.pink,
    IA_COLORS.lime,
    IA_COLORS.indigo,
    IA_COLORS.red,
  ];
  const idx = Math.max(0, sortedIds.indexOf(instanceId));
  return palette[idx % palette.length];
}

export default function ObservabilityPage() {
  type WindowKey = "5m" | "1h" | "24h";
  const [instances, setInstances] = useState<Instance[]>([]);
  const [system, setSystem] = useState<SystemActivityResponse | null>(null);
  const [gpu, setGpu] = useState<GpuActivityResponse | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [tick, setTick] = useState(0);
  const [windowKey, setWindowKey] = useState<WindowKey>("5m");

  const query = useMemo(() => {
    const window_s = windowKey === "5m" ? 300 : windowKey === "1h" ? 3600 : 24 * 3600;
    const granularity = windowKey === "5m" ? "second" : windowKey === "1h" ? "minute" : "minute";
    const params = new URLSearchParams();
    params.set("window_s", String(window_s));
    params.set("granularity", granularity);
    return params.toString();
  }, [windowKey]);

  useEffect(() => {
    const t = setInterval(() => setTick((v) => v + 1), 4000);
    return () => clearInterval(t);
  }, []);

  useEffect(() => {
    let cancelled = false;
    const run = async () => {
      try {
        const res = await fetch(apiUrl(`instances`));
        if (!res.ok) return;
        const json = (await res.json()) as Instance[];
        if (!cancelled) setInstances(Array.isArray(json) ? json : []);
      } catch {
        // ignore
      }
    };
    void run();
    return () => {
      cancelled = true;
    };
  }, []);

  const activeInstances = useMemo(() => {
    const isActive = (s: string) => {
      const v = s.toLowerCase();
      return v !== "terminated";
    };
    return instances.filter((i) => isActive(i.status));
  }, [instances]);

  const activeIdsSorted = useMemo(
    () => activeInstances.map((i) => i.id).slice().sort((a, b) => a.localeCompare(b)),
    [activeInstances]
  );

  useEffect(() => {
    let cancelled = false;
    const run = async () => {
      setIsLoading(true);
      setErr(null);
      try {
        const [sysRes, gpuRes] = await Promise.all([
          fetch(apiUrl(`system/activity?${query}`)),
          fetch(apiUrl(`gpu/activity?${query}`)),
        ]);

        if (!sysRes.ok) {
          const txt = await sysRes.text().catch(() => "");
          throw new Error(txt || `system/activity HTTP ${sysRes.status}`);
        }
        if (!gpuRes.ok) {
          const txt = await gpuRes.text().catch(() => "");
          throw new Error(txt || `gpu/activity HTTP ${gpuRes.status}`);
        }

        const [sysJson, gpuJson] = (await Promise.all([sysRes.json(), gpuRes.json()])) as [
          SystemActivityResponse,
          GpuActivityResponse,
        ];
        if (!cancelled) {
          setSystem(sysJson);
          setGpu(gpuJson);
        }
      } catch (e: unknown) {
        const msg =
          e instanceof Error ? e.message : typeof e === "string" ? e : e && typeof e === "object" ? JSON.stringify(e) : String(e);
        if (!cancelled) setErr(msg);
      } finally {
        if (!cancelled) setIsLoading(false);
      }
    };
    void run();
    return () => {
      cancelled = true;
    };
  }, [tick, query]);

  const systemByInstance = useMemo(() => {
    const map = new Map<string, SystemActivityResponse["instances"][number]>();
    for (const s of system?.instances ?? []) map.set(String(s.instance_id), s);
    return map;
  }, [system]);

  const gpuByInstance = useMemo(() => {
    const map = new Map<string, GpuActivityResponse["instances"][number]>();
    for (const s of gpu?.instances ?? []) map.set(String(s.instance_id), s);
    return map;
  }, [gpu]);

  return (
    <div className="p-8 space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Observability</h1>
          <p className="text-muted-foreground">
            Refresh every 4s · Une couleur par instance · Local mock: GPU peut être vide sans NVIDIA runtime
          </p>
        </div>
        <div className="flex items-center gap-2">
          <select
            className="h-9 rounded-md border border-input bg-background px-3 text-sm"
            value={windowKey}
            onChange={(e: ChangeEvent<HTMLSelectElement>) => {
              const v = e.target.value;
              const isWindowKey = (x: string): x is WindowKey => x === "5m" || x === "1h" || x === "24h";
              if (isWindowKey(v)) setWindowKey(v);
            }}
          >
            <option value="5m">5 min (sec)</option>
            <option value="1h">1 h (min)</option>
            <option value="24h">24 h (min)</option>
          </select>
          <Button variant="outline" onClick={() => setTick((v) => v + 1)} disabled={isLoading}>
            <RefreshCcw className="h-4 w-4 mr-2" />
            Refresh
          </Button>
        </div>
      </div>

      {err ? <div className="text-sm text-red-600">Erreur: {err}</div> : null}
      {!err && (!system || !gpu) ? <div className="text-sm text-muted-foreground">Chargement…</div> : null}

      {!err && system && gpu && activeInstances.length === 0 ? (
        <div className="text-sm text-muted-foreground">Aucune instance active.</div>
      ) : null}

      <div className="grid gap-4 md:grid-cols-2">
        {activeInstances.map((inst) => {
          const color = stableColorForInstance(inst.id, activeIdsSorted);
          const sys = systemByInstance.get(inst.id) ?? null;
          const gpuSeries = gpuByInstance.get(inst.id) ?? null;

          const sysSamples = sys?.samples ?? [];
          const cpuMem = sysSamples.map((s) => ({ a: s.cpu_pct ?? null, b: s.mem_pct ?? null }));
          const net = sysSamples.map((s) => ({ a: s.net_rx_mbps ?? null, b: s.net_tx_mbps ?? null }));
          const disk = sysSamples.map((s) => ({ a: s.disk_pct ?? null, b: null }));

          const lastSys = sysSamples.length ? sysSamples[sysSamples.length - 1] : null;
          const hb = inst.worker_last_heartbeat ? new Date(inst.worker_last_heartbeat) : null;
          const hbMs = hb && !Number.isNaN(hb.getTime()) ? hb.getTime() : null;
          const hbAgeS = hbMs != null ? Math.max(0, (Date.now() - hbMs) / 1000) : null;
          const hasHeartbeat = hbAgeS != null;
          const isHeartbeatStale = hbAgeS != null ? hbAgeS > 30 : true;
          const hasAnySamples = sysSamples.length > 0 || (gpuSeries?.gpus?.some((g) => (g.samples ?? []).length > 0) ?? false);

          return (
            <Card
              key={inst.id}
              className="overflow-hidden"
              style={{
                borderColor: withAlpha(color, 0.35),
                boxShadow: `0 0 0 1px ${withAlpha(color, 0.18)} inset`,
              }}
            >
              <CardContent className="p-4 space-y-3">
                <div className="flex items-start justify-between gap-3">
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      <span
                        className="inline-block size-2.5 rounded-full"
                        style={{ backgroundColor: color }}
                        title="instance color"
                      />
                      <div className="font-mono text-xs truncate">{inst.id.slice(0, 8)}…</div>
                      <div className="text-xs text-muted-foreground truncate">
                        {displayOrDash(inst.provider_name)} · {displayOrDash(inst.region)} · {displayOrDash(inst.zone)}
                      </div>
                    </div>
                    <div className="text-xs text-muted-foreground mt-1">
                      status: <span className="font-medium text-foreground">{displayOrDash(inst.status)}</span>
                      {typeof lastSys?.load1 === "number" ? (
                        <>
                          {" "}
                          · load1: <span className="font-mono">{lastSys.load1.toFixed(2)}</span>
                        </>
                      ) : null}
                    </div>
                    <div className="mt-2 flex flex-wrap items-center gap-2">
                      {!hasHeartbeat ? (
                        <span
                          className="inline-flex items-center rounded-full border px-2 py-0.5 text-[11px]"
                          style={{ borderColor: withAlpha(IA_COLORS.amber, 0.55), color: IA_COLORS.amber }}
                          title="Le worker n'a pas encore envoyé de heartbeat"
                        >
                          No worker heartbeat yet
                        </span>
                      ) : isHeartbeatStale ? (
                        <span
                          className="inline-flex items-center rounded-full border px-2 py-0.5 text-[11px]"
                          style={{ borderColor: withAlpha(IA_COLORS.red, 0.55), color: IA_COLORS.red }}
                          title="Dernier heartbeat trop ancien"
                        >
                          Heartbeat stale ({Math.round(hbAgeS ?? 0)}s)
                        </span>
                      ) : (
                        <span
                          className="inline-flex items-center rounded-full border px-2 py-0.5 text-[11px]"
                          style={{ borderColor: withAlpha(IA_COLORS.green, 0.55), color: IA_COLORS.green }}
                          title="Heartbeat récent"
                        >
                          Heartbeat OK ({Math.round(hbAgeS ?? 0)}s)
                        </span>
                      )}
                      {!hasAnySamples ? (
                        <span
                          className="inline-flex items-center rounded-full border px-2 py-0.5 text-[11px] text-muted-foreground"
                          title="Pas encore de time-series dans system_samples/gpu_samples"
                        >
                          No samples yet
                        </span>
                      ) : null}
                    </div>
                  </div>
                  <div className="text-right text-xs text-muted-foreground">
                    GPU: {inst.gpu_count ?? "—"} · VRAM: {inst.gpu_vram ? `${inst.gpu_vram}GB` : "—"}
                  </div>
                </div>

                <div className="grid gap-3 md:grid-cols-2">
                  <div className="rounded-lg border p-3">
                    <div className="flex items-center justify-between">
                      <div className="text-xs font-medium">CPU% / Mem%</div>
                      <div className="text-[11px] text-muted-foreground">{cpuMem.length ? `${cpuMem.length} pts` : "No data"}</div>
                    </div>
                    <div className="mt-2">
                      {cpuMem.length ? (
                        <IASparklineDual points={cpuMem} />
                      ) : (
                        <div className="text-xs text-muted-foreground">No system samples.</div>
                      )}
                    </div>
                  </div>

                  <div className="rounded-lg border p-3">
                    <div className="flex items-center justify-between">
                      <div className="text-xs font-medium">Net Rx/Tx (Mbps)</div>
                      <div className="text-[11px] text-muted-foreground">{net.length ? `${net.length} pts` : "No data"}</div>
                    </div>
                    <div className="mt-2">
                      {net.length ? <IASparklineDual points={net} /> : <div className="text-xs text-muted-foreground">No net samples.</div>}
                    </div>
                  </div>
                </div>

                <div className="rounded-lg border p-3">
                  <div className="flex items-center justify-between">
                    <div className="text-xs font-medium">Disk%</div>
                    <div className="text-[11px] text-muted-foreground">{disk.length ? `${disk.length} pts` : "No data"}</div>
                  </div>
                  <div className="mt-2">
                    {disk.length ? (
                      <IASparklineDual points={disk} />
                    ) : (
                      <div className="text-xs text-muted-foreground">No disk samples.</div>
                    )}
                  </div>
                </div>

                <div className="rounded-lg border p-3">
                  <div className="flex items-center justify-between">
                    <div className="text-xs font-medium">GPU activity</div>
                    <div className="text-[11px] text-muted-foreground">
                      {gpuSeries?.gpus?.length ? `${gpuSeries.gpus.length} GPU` : "No data"}
                    </div>
                  </div>

                  {!gpuSeries?.gpus?.length ? (
                    <div className="text-xs text-muted-foreground mt-2">
                      Aucune métrique GPU reçue (normal en mock local sans GPU / NVIDIA runtime).
                    </div>
                  ) : (
                    <div className="mt-3 grid gap-3 md:grid-cols-2">
                      {(gpuSeries.gpus ?? []).map((g) => {
                        const points = (g.samples ?? []).map((s) => ({ a: s.gpu_pct ?? null, b: s.vram_pct ?? null }));
                        const last = (g.samples ?? [])[Math.max(0, (g.samples ?? []).length - 1)];
                        const temp = typeof last?.temp_c === "number" ? `${last.temp_c.toFixed(0)}°C` : "—";
                        const power =
                          typeof last?.power_w === "number"
                            ? `${last.power_w.toFixed(0)}W${typeof last?.power_limit_w === "number" ? `/${last.power_limit_w.toFixed(0)}W` : ""}`
                            : "—";
                        return (
                          <div key={`${inst.id}:${g.gpu_index}`} className="rounded-md border p-3">
                            <div className="flex items-center justify-between">
                              <div className="font-mono text-xs">GPU {g.gpu_index}</div>
                              <div className="text-[11px] text-muted-foreground">
                                {temp} · {power}
                              </div>
                            </div>
                            <div className="mt-2">
                              {points.length ? (
                                <IASparklineDual points={points} />
                              ) : (
                                <div className="text-xs text-muted-foreground">No samples.</div>
                              )}
                            </div>
                          </div>
                        );
                      })}
                    </div>
                  )}
                </div>
              </CardContent>
            </Card>
          );
        })}
      </div>
    </div>
  );
}


