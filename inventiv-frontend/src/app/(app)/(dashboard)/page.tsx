"use client";

import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { useInstances } from "@/hooks/useInstances";
import { useFinopsCosts } from "@/hooks/useFinops";
import { Server, Activity, TrendingUp, Clock, AlertTriangle, Loader2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { displayOrDash, formatEur } from "@/lib/utils";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useMemo } from "react";
import { IA_COLORS, IADonutMiniChart, IAHistogramTimeSeries, IAStatCell } from "ia-widgets";

export default function DashboardPage() {
  const { instances } = useInstances();
  const finops = useFinopsCosts();

  // Calculate statistics
  const stats = {
    total: instances.length,
    active: instances.filter((i) => i.status.toLowerCase() === "ready").length,
    provisioning: instances.filter((i) =>
      ["provisioning", "booting"].includes(i.status.toLowerCase())
    ).length,
    failed: instances.filter((i) =>
      ["failed", "startup_failed", "provisioning_failed"].includes(i.status.toLowerCase())
    ).length,
  };

  const allocationTotal = finops.summary?.allocation?.total ?? null;
  const cumulativeTotal = finops.summary?.cumulative_total?.cumulative_amount_eur ?? null;

  const spendWindows = useMemo(() => {
    const rows = finops.summary?.actual_spend_windows ?? [];
    const order = ["minute", "hour", "day", "month_30d", "year_365d"];
    return [...rows].sort((a, b) => order.indexOf(a.window) - order.indexOf(b.window));
  }, [finops.summary]);

  const spendSeries = useMemo(() => {
    const rows = finops.series ?? [];
    return rows
      .slice()
      .sort((a, b) => new Date(a.bucket).getTime() - new Date(b.bucket).getTime())
      .map((r) => ({ t: r.bucket, v: r.amount_eur }));
  }, [finops.series]);

  const spendTicks = useMemo(() => {
    const n = spendSeries.length;
    if (n <= 1) return [];
    const fmt = (iso: string) => {
      const d = new Date(iso);
      // Minimal labels, based on selected window.
      if (finops.window === "hour") return d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
      if (finops.window === "day") return d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
      if (finops.window === "week_7d") return d.toLocaleDateString(undefined, { weekday: "short" });
      if (finops.window === "month_30d") return d.toLocaleDateString(undefined, { month: "short", day: "2-digit" });
      return d.toLocaleDateString(undefined, { month: "short" });
    };
    const i0 = 0;
    const i1 = Math.floor((n - 1) / 2);
    const i2 = n - 1;
    return [
      { index: i0, label: fmt(spendSeries[i0].t) },
      { index: i1, label: fmt(spendSeries[i1].t) },
      { index: i2, label: fmt(spendSeries[i2].t) },
    ];
  }, [finops.window, spendSeries]);

  const allocationDonut = useMemo(() => {
    const rows = finops.summary?.allocation?.by_provider ?? [];
    const top = [...rows].sort((a, b) => b.burn_rate_eur_per_hour - a.burn_rate_eur_per_hour).slice(0, 4);
    const palette = [IA_COLORS.teal, IA_COLORS.blue, IA_COLORS.orange, IA_COLORS.purple];
    return top.map((p, idx) => ({
      label: p.provider_code ?? p.provider_name ?? p.provider_id.slice(0, 8),
      value: p.burn_rate_eur_per_hour,
      color: palette[idx % palette.length],
    }));
  }, [finops.summary]);

  // Recent instances (last 5)
  const recentInstances = [...instances]
    .sort(
      (a, b) =>
        new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
    )
    .slice(0, 5);

  return (
    <div className="p-8 space-y-8">
      {/* Header */}
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Dashboard</h1>
        <p className="text-muted-foreground">
          Overview of your GPU infrastructure and usage statistics
        </p>
      </div>

      {/* Instances overview + Recent instances */}
      <div className="grid gap-4 md:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Server className="h-5 w-5" />
              Instances
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="grid gap-3 sm:grid-cols-2">
              <div className="p-3 border rounded-lg">
                <div className="flex items-center justify-between">
                  <div className="text-xs text-muted-foreground">Total</div>
                  <Server className="h-4 w-4 text-muted-foreground" />
                </div>
                <div className="text-2xl font-bold">{stats.total}</div>
                <div className="text-xs text-muted-foreground">All time managed</div>
              </div>
              <div className="p-3 border rounded-lg">
                <div className="flex items-center justify-between">
                  <div className="text-xs text-muted-foreground">Active</div>
                  <Activity className="h-4 w-4 text-green-600" />
                </div>
                <div className="text-2xl font-bold text-green-600">{stats.active}</div>
                <div className="text-xs text-muted-foreground">Currently running</div>
              </div>
              <div className="p-3 border rounded-lg">
                <div className="flex items-center justify-between">
                  <div className="text-xs text-muted-foreground">Provisioning</div>
                  <Loader2 className="h-4 w-4 text-blue-600" />
                </div>
                <div className="text-2xl font-bold text-blue-600">{stats.provisioning}</div>
                <div className="text-xs text-muted-foreground">Provisioning / booting</div>
              </div>
              <div className="p-3 border rounded-lg">
                <div className="flex items-center justify-between">
                  <div className="text-xs text-muted-foreground">Failed</div>
                  <AlertTriangle className={`h-4 w-4 ${stats.failed > 0 ? "text-red-600" : "text-muted-foreground"}`} />
                </div>
                <div className={`text-2xl font-bold ${stats.failed > 0 ? "text-red-600" : "text-muted-foreground"}`}>
                  {stats.failed}
                </div>
                <div className="text-xs text-muted-foreground">Needs attention</div>
              </div>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Clock className="h-5 w-5" />
              Recent Instances
            </CardTitle>
          </CardHeader>
          <CardContent>
            {recentInstances.length === 0 ? (
              <p className="text-sm text-muted-foreground text-center py-4">
                No instances yet
              </p>
            ) : (
              <div className="space-y-3">
                {recentInstances.map((instance) => (
                  <div
                    key={instance.id}
                    className="flex items-center justify-between p-3 border rounded-lg hover:bg-muted/50 transition-colors"
                  >
                    <div>
                      <p className="font-medium text-sm">
                        {displayOrDash(instance.instance_type)}
                      </p>
                      <p className="text-xs text-muted-foreground">
                        {displayOrDash(instance.zone)} • {displayOrDash(instance.provider_name)}
                      </p>
                    </div>
                    <div className="flex items-center gap-2">
                      <Badge
                        variant={
                          instance.status.toLowerCase() === "ready"
                            ? "default"
                            : "secondary"
                        }
                      >
                        {instance.status}
                      </Badge>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </CardContent>
        </Card>
      </div>

      {/* FinOps */}
      <Card>
        <CardHeader>
          <CardTitle>FinOps – Allocation & Spend</CardTitle>
        </CardHeader>
        <CardContent>
          {finops.loading && !finops.summary ? (
            <p className="text-sm text-muted-foreground">Loading FinOps data…</p>
          ) : finops.error ? (
            <p className="text-sm text-red-600">{finops.error}</p>
          ) : (
            <div className="space-y-3">
              {/* Allocation totals (current) */}
              <div className="grid gap-3 md:grid-cols-4">
                <IAStatCell
                  title="Allocation / minute"
                  value={allocationTotal ? formatEur(allocationTotal.forecast_eur_per_minute, { minFrac: 6, maxFrac: 6 }) : "—"}
                  subtitle="forecast"
                  icon={TrendingUp}
                  accent="amber"
                />
                <IAStatCell
                  title="Allocation / hour"
                  value={allocationTotal ? formatEur(allocationTotal.forecast_eur_per_hour, { minFrac: 4, maxFrac: 4 }) : "—"}
                  subtitle="forecast"
                  icon={TrendingUp}
                  accent="red"
                />
                <IAStatCell
                  title="Allocation / day"
                  value={allocationTotal ? formatEur(allocationTotal.forecast_eur_per_day, { minFrac: 4, maxFrac: 4 }) : "—"}
                  subtitle="forecast"
                  icon={TrendingUp}
                  accent="blue"
                />
                <IAStatCell
                  title="Allocation / 30d"
                  value={allocationTotal ? formatEur(allocationTotal.forecast_eur_per_month_30d, { minFrac: 2, maxFrac: 2 }) : "—"}
                  subtitle="forecast"
                  icon={TrendingUp}
                  accent="purple"
                />
              </div>

              <div className="grid gap-3 md:grid-cols-[1fr_260px]">
                <div className="rounded-xl border border-border bg-card text-card-foreground p-4">
                  <div className="flex items-center justify-between mb-2">
                    <div>
                      <div className="text-sm font-medium">Actual spend (curve)</div>
                    </div>
                    <Tabs value={finops.window} onValueChange={(v: string) => finops.setWindow(v)}>
                      <TabsList>
                        <TabsTrigger value="hour">1h</TabsTrigger>
                        <TabsTrigger value="day">1d</TabsTrigger>
                        <TabsTrigger value="week_7d">1w</TabsTrigger>
                        <TabsTrigger value="month_30d">1m</TabsTrigger>
                        <TabsTrigger value="year_365d">1y</TabsTrigger>
                      </TabsList>
                    </Tabs>
                  </div>
                  <div className="flex items-center gap-3">
                    <IAHistogramTimeSeries
                      points={spendSeries}
                      width={420}
                      height={90}
                      color={IA_COLORS.blue}
                      ticks={spendTicks}
                    />
                    <div className="min-w-0">
                      <div className="text-xs text-muted-foreground">Cumulative total</div>
                      <div className="text-lg font-semibold tabular-nums">
                        {cumulativeTotal !== null ? formatEur(cumulativeTotal, { minFrac: 2, maxFrac: 2 }) : "—"}
                      </div>
                      <div className="text-xs text-muted-foreground mt-1">Auto-refresh 10s</div>
                    </div>
                  </div>
                </div>

                <div className="rounded-xl border border-border bg-card text-card-foreground p-4 flex items-center gap-3">
                  <IADonutMiniChart
                    segments={allocationDonut}
                    size={140}
                    centerLabel={
                      allocationTotal
                        ? formatEur(allocationTotal.forecast_eur_per_hour, { minFrac: 4, maxFrac: 4 })
                        : "—"
                    }
                    subLabel={allocationTotal ? "Total €/h" : "No data"}
                    showSegmentLabels={true}
                  />
                  <div className="min-w-0">
                    <div className="text-sm font-medium">Top providers</div>
                    <div className="mt-2 space-y-1">
                      {allocationDonut.length ? (
                        allocationDonut.map((s) => (
                          <div key={s.label} className="flex items-center justify-between gap-2 text-xs">
                            <div className="flex items-center gap-2 min-w-0">
                              <span className="inline-block size-2.5 rounded-full" style={{ backgroundColor: s.color }} />
                              <span className="truncate text-muted-foreground">{s.label}</span>
                            </div>
                            <span className="tabular-nums">{formatEur(s.value, { minFrac: 4, maxFrac: 4 })}/h</span>
                          </div>
                        ))
                      ) : (
                        <div className="text-xs text-muted-foreground">No allocation data yet.</div>
                      )}
                    </div>
                  </div>
                </div>
              </div>

              {/* Allocation breakdown (current) */}
              <div className="pt-2">
                <div className="text-sm font-medium mb-2">Allocation breakdown (current burn rate)</div>
                {finops.summary?.allocation?.by_provider?.length ? (
                  <div className="space-y-2">
                    {finops.summary.allocation.by_provider.slice(0, 6).map((p) => (
                      <div key={p.provider_id} className="flex items-center justify-between p-3 border rounded-lg">
                        <div className="text-sm">
                          {p.provider_name}{" "}
                          <span className="text-xs text-muted-foreground">
                            ({p.provider_code ?? p.provider_id.slice(0, 8)})
                          </span>
                        </div>
                        <div className="font-semibold">
                          {formatEur(p.burn_rate_eur_per_hour, { minFrac: 4, maxFrac: 4 })}/h
                        </div>
                      </div>
                    ))}
                  </div>
                ) : (
                  <p className="text-sm text-muted-foreground">No allocation data yet.</p>
                )}
              </div>

              <div className="pt-2">
                <div className="text-sm font-medium mb-2">Actual spend (windowed)</div>
                {spendWindows.length ? (
                  <div className="grid gap-3 md:grid-cols-5">
                    {spendWindows.map((w) => (
                      <div key={w.window} className="p-3 border rounded-lg">
                        <div className="text-xs text-muted-foreground">
                          {w.window === "minute"
                            ? "1m"
                            : w.window === "hour"
                            ? "1h"
                            : w.window === "day"
                            ? "1d"
                            : w.window === "month_30d"
                            ? "30d"
                            : "365d"}
                        </div>
                        <div className="text-lg font-bold">
                          {formatEur(w.actual_spend_eur, { minFrac: 4, maxFrac: 4 })}
                        </div>
                      </div>
                    ))}
                  </div>
                ) : (
                  <p className="text-sm text-muted-foreground">No spend data yet.</p>
                )}
              </div>

              {/* Spend breakdown (selected window) */}
              <div className="pt-4 space-y-3">
                <div className="flex items-center justify-between">
                  <div className="text-sm font-medium">Spend breakdown</div>
                  <Tabs value={finops.window} onValueChange={(v: string) => finops.setWindow(v)}>
                    <TabsList>
                      <TabsTrigger value="minute">1m</TabsTrigger>
                      <TabsTrigger value="hour">1h</TabsTrigger>
                      <TabsTrigger value="day">1d</TabsTrigger>
                      <TabsTrigger value="month_30d">30d</TabsTrigger>
                      <TabsTrigger value="year_365d">365d</TabsTrigger>
                    </TabsList>
                  </Tabs>
                </div>

                <div className="grid gap-3 md:grid-cols-2">
                  <div className="p-3 border rounded-lg">
                    <div className="text-xs text-muted-foreground">Total spend ({finops.breakdown?.window ?? finops.window})</div>
                    <div className="text-xl font-bold">
                      {formatEur(finops.breakdown?.total_eur ?? null, { minFrac: 4, maxFrac: 4 })}
                    </div>
                  </div>
                  <div className="p-3 border rounded-lg bg-purple-50/70 border-purple-200">
                    <div className="flex items-center justify-between">
                      <div className="text-xs text-muted-foreground">Cumulative (all time)</div>
                      <TrendingUp className="h-4 w-4 text-purple-700" />
                    </div>
                    <div className="text-xl font-bold text-purple-700">
                      {formatEur(cumulativeTotal, { minFrac: 4, maxFrac: 4 })}
                    </div>
                    <div className="text-xs text-muted-foreground">Real spend since inception</div>
                  </div>
                </div>

                <div className="grid gap-3 md:grid-cols-2">
                  <div className="space-y-2">
                    <div className="text-sm font-medium">By provider</div>
                    {finops.breakdown?.by_provider_eur?.length ? (
                      finops.breakdown.by_provider_eur.slice(0, 6).map((p) => (
                        <div key={p.provider_id} className="flex items-center justify-between p-3 border rounded-lg">
                          <div className="text-sm">
                            {p.provider_name}{" "}
                            <span className="text-xs text-muted-foreground">
                              ({p.provider_code ?? p.provider_id.slice(0, 8)})
                            </span>
                          </div>
                          <div className="font-semibold">{formatEur(p.amount_eur, { minFrac: 4, maxFrac: 4 })}</div>
                        </div>
                      ))
                    ) : (
                      <p className="text-sm text-muted-foreground">No data.</p>
                    )}
                  </div>

                  <div className="space-y-2">
                    <div className="text-sm font-medium">By instance (top)</div>
                    {finops.breakdown?.by_instance_eur?.length ? (
                      finops.breakdown.by_instance_eur.slice(0, 6).map((r) => (
                        <div key={r.instance_id} className="flex items-center justify-between p-3 border rounded-lg">
                          <div className="text-sm">
                            {displayOrDash(r.instance_type_name)}{" "}
                            <span className="text-xs text-muted-foreground">
                              {displayOrDash(r.zone_name)} • {r.provider_name}
                            </span>
                          </div>
                          <div className="font-semibold">{formatEur(r.amount_eur, { minFrac: 4, maxFrac: 4 })}</div>
                        </div>
                      ))
                    ) : (
                      <p className="text-sm text-muted-foreground">No data.</p>
                    )}
                  </div>
                </div>

                <div className="grid gap-3 md:grid-cols-2">
                  <div className="space-y-2">
                    <div className="text-sm font-medium">By region</div>
                    {finops.breakdown?.by_region_eur?.length ? (
                      finops.breakdown.by_region_eur.slice(0, 6).map((r) => (
                        <div key={`${r.provider_id}-${r.region_id}`} className="flex items-center justify-between p-3 border rounded-lg">
                          <div className="text-sm">
                            {displayOrDash(r.region_name)}{" "}
                            <span className="text-xs text-muted-foreground">{r.provider_code ?? r.provider_id.slice(0, 8)}</span>
                          </div>
                          <div className="font-semibold">{formatEur(r.amount_eur, { minFrac: 4, maxFrac: 4 })}</div>
                        </div>
                      ))
                    ) : (
                      <p className="text-sm text-muted-foreground">No data.</p>
                    )}
                  </div>

                  <div className="space-y-2">
                    <div className="text-sm font-medium">By instance type</div>
                    {finops.breakdown?.by_instance_type_eur?.length ? (
                      finops.breakdown.by_instance_type_eur.slice(0, 6).map((t) => (
                        <div key={`${t.provider_id}-${t.instance_type_id}`} className="flex items-center justify-between p-3 border rounded-lg">
                          <div className="text-sm">
                            {displayOrDash(t.instance_type_name)}{" "}
                            <span className="text-xs text-muted-foreground">{t.provider_code ?? t.provider_id.slice(0, 8)}</span>
                          </div>
                          <div className="font-semibold">{formatEur(t.amount_eur, { minFrac: 4, maxFrac: 4 })}</div>
                        </div>
                      ))
                    ) : (
                      <p className="text-sm text-muted-foreground">No data.</p>
                    )}
                  </div>
                </div>
              </div>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}


