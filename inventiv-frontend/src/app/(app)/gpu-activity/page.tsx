"use client";

import { useEffect, useMemo, useState, type ChangeEvent } from "react";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { RefreshCcw } from "lucide-react";
import { apiUrl } from "@/lib/api";
import type { GpuActivityResponse, Instance } from "@/lib/types";
import { IASparklineDual } from "ia-widgets";
import { displayOrDash } from "@/lib/utils";

export default function GpuActivityPage() {
  type WindowKey = "5m" | "1h" | "24h" | "7d" | "30d";
  const [data, setData] = useState<GpuActivityResponse | null>(null);
  const [instances, setInstances] = useState<Instance[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [tick, setTick] = useState(0);
  const [windowKey, setWindowKey] = useState<WindowKey>("5m");
  const [instanceId, setInstanceId] = useState<string>("all");

  const query = useMemo(() => {
    const window_s =
      windowKey === "5m"
        ? 300
        : windowKey === "1h"
          ? 3600
          : windowKey === "24h"
            ? 24 * 3600
            : windowKey === "7d"
              ? 7 * 24 * 3600
              : 30 * 24 * 3600;
    const granularity =
      windowKey === "5m" ? "second" : windowKey === "1h" ? "minute" : windowKey === "24h" ? "minute" : windowKey === "7d" ? "hour" : "day";
    const params = new URLSearchParams();
    params.set("window_s", String(window_s));
    params.set("granularity", granularity);
    if (instanceId !== "all") params.set("instance_id", instanceId);
    return params.toString();
  }, [instanceId, windowKey]);

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

  useEffect(() => {
    let cancelled = false;
    const run = async () => {
      setIsLoading(true);
      setErr(null);
      try {
        const res = await fetch(apiUrl(`gpu/activity?${query}`));
        if (!res.ok) {
          const txt = await res.text().catch(() => "");
          throw new Error(txt || `HTTP ${res.status}`);
        }
        const json = (await res.json()) as GpuActivityResponse;
        if (!cancelled) setData(json);
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

  const tiles = useMemo(() => {
    const out: Array<{
      key: string;
      title: string;
      subtitle: string;
      points: { a: number | null; b: number | null }[];
    }> = [];
    for (const inst of data?.instances ?? []) {
      const instLabel = `${inst.instance_id.slice(0, 8)}…`;
      const provider = displayOrDash(inst.provider_name);
      const name = inst.instance_name ? String(inst.instance_name).slice(0, 12) : null;
      for (const g of inst.gpus ?? []) {
        const points = (g.samples ?? []).map((s) => ({
          a: s.gpu_pct ?? null,
          b: s.vram_pct ?? null,
        }));
        const last = (g.samples ?? [])[Math.max(0, (g.samples ?? []).length - 1)];
        const temp = typeof last?.temp_c === "number" ? `${last.temp_c.toFixed(0)}°C` : "-";
        const power =
          typeof last?.power_w === "number"
            ? `${last.power_w.toFixed(0)}W${typeof last?.power_limit_w === "number" ? `/${last.power_limit_w.toFixed(0)}W` : ""}`
            : "-";
        out.push({
          key: `${inst.instance_id}:${g.gpu_index}`,
          title: `${instLabel}  GPU ${g.gpu_index}`,
          subtitle: `${provider}${name ? ` · ${name}` : ""} · ${temp} · ${power}`,
          points,
        });
      }
    }
    return out;
  }, [data]);

  return (
    <div className="p-8 space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">GPU Activity</h1>
          <p className="text-muted-foreground">Refresh every 4s · Green = GPU% · Orange = VRAM%</p>
        </div>
        <div className="flex items-center gap-2">
          <select
            className="h-9 rounded-md border border-input bg-background px-3 text-sm"
            value={instanceId}
            onChange={(e: ChangeEvent<HTMLSelectElement>) => setInstanceId(e.target.value)}
          >
            <option value="all">All instances</option>
            {instances.map((i) => (
              <option key={i.id} value={i.id}>
                {i.id.slice(0, 8)}… · {i.status}
              </option>
            ))}
          </select>
          <select
            className="h-9 rounded-md border border-input bg-background px-3 text-sm"
            value={windowKey}
            onChange={(e: ChangeEvent<HTMLSelectElement>) => {
              const v = e.target.value;
              const isWindowKey = (x: string): x is WindowKey =>
                x === "5m" || x === "1h" || x === "24h" || x === "7d" || x === "30d";
              if (isWindowKey(v)) setWindowKey(v);
            }}
          >
            <option value="5m">5 min (sec)</option>
            <option value="1h">1 h (min)</option>
            <option value="24h">24 h (min)</option>
            <option value="7d">7 d (hour)</option>
            <option value="30d">30 d (day)</option>
          </select>
          <Button variant="outline" onClick={() => setTick((v) => v + 1)} disabled={isLoading}>
            <RefreshCcw className="h-4 w-4 mr-2" />
            Refresh
          </Button>
        </div>
      </div>

      <Card className="bg-black text-white border-zinc-900">
        <CardContent className="py-6">
          {err ? (
            <div className="text-red-400 text-sm">Erreur: {err}</div>
          ) : null}
          {!err && !data ? (
            <div className="text-zinc-400 text-sm">Chargement…</div>
          ) : null}
          {!err && data && tiles.length === 0 ? (
            <div className="text-zinc-400 text-sm">
              Aucune métrique GPU reçue (attends les heartbeats worker / GPU samples).
            </div>
          ) : null}

          <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
            {tiles.map((t) => (
              <div key={t.key} className="rounded-lg border border-zinc-900 bg-black p-3">
                <div className="flex items-baseline justify-between gap-2">
                  <div className="font-mono text-xs text-zinc-200 truncate">{t.title}</div>
                </div>
                <div className="text-[11px] text-zinc-500 truncate">{t.subtitle}</div>
                <div className="mt-2">
                  <IASparklineDual points={t.points} />
                </div>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}


