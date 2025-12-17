"use client";

import { useEffect, useMemo, useState } from "react";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { RefreshCcw } from "lucide-react";
import { apiUrl } from "@/lib/api";
import type { GpuActivityResponse } from "@/lib/types";
import { SparklineDual } from "@/components/shared/SparklineDual";
import { displayOrDash } from "@/lib/utils";

export default function GpuActivityPage() {
  const [data, setData] = useState<GpuActivityResponse | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [tick, setTick] = useState(0);

  useEffect(() => {
    const t = setInterval(() => setTick((v) => v + 1), 4000);
    return () => clearInterval(t);
  }, []);

  useEffect(() => {
    let cancelled = false;
    const run = async () => {
      setIsLoading(true);
      setErr(null);
      try {
        const res = await fetch(apiUrl(`gpu/activity?window_s=300`));
        if (!res.ok) {
          const txt = await res.text().catch(() => "");
          throw new Error(txt || `HTTP ${res.status}`);
        }
        const json = (await res.json()) as GpuActivityResponse;
        if (!cancelled) setData(json);
      } catch (e: any) {
        if (!cancelled) setErr(String(e?.message || e));
      } finally {
        if (!cancelled) setIsLoading(false);
      }
    };
    void run();
    return () => {
      cancelled = true;
    };
  }, [tick]);

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
        out.push({
          key: `${inst.instance_id}:${g.gpu_index}`,
          title: `${instLabel}  GPU ${g.gpu_index}`,
          subtitle: `${provider}${name ? ` · ${name}` : ""}`,
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
        <Button
          variant="outline"
          onClick={() => setTick((v) => v + 1)}
          disabled={isLoading}
        >
          <RefreshCcw className="h-4 w-4 mr-2" />
          Refresh
        </Button>
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
                  <SparklineDual points={t.points} />
                </div>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}


