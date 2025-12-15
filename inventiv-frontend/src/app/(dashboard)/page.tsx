"use client";

import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { StatsCard } from "@/components/shared/StatsCard";
import { useInstances } from "@/hooks/useInstances";
import { useFinopsCosts } from "@/hooks/useFinops";
import { Server, Activity, DollarSign, Zap, TrendingUp, Clock } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { displayOrDash, formatEur } from "@/lib/utils";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useMemo } from "react";

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
    totalCost: instances.reduce((sum, i) => sum + (i.total_cost || 0), 0),
    avgCostPerHour:
      instances.length > 0
        ? instances.reduce((sum, i) => sum + (i.cost_per_hour || 0), 0) /
          instances.length
        : 0,
  };

  const allocationTotal = finops.summary?.allocation?.total ?? null;
  const cumulativeTotal = finops.summary?.cumulative_total?.cumulative_amount_eur ?? null;
  const spend1m = finops.summary?.actual_spend_windows?.find((w) => w.window === "minute")?.actual_spend_eur ?? null;

  const spendWindows = useMemo(() => {
    const rows = finops.summary?.actual_spend_windows ?? [];
    const order = ["minute", "hour", "day", "month_30d", "year_365d"];
    return [...rows].sort((a, b) => order.indexOf(a.window) - order.indexOf(b.window));
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

      {/* Stats Grid */}
      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
        <StatsCard
          title="Total Instances"
          value={stats.total}
          description="All time managed"
          icon={Server}
        />
        <StatsCard
          title="Active Instances"
          value={stats.active}
          description="Currently running"
          icon={Activity}
          valueClassName="text-green-600"
          iconClassName="text-green-600"
        />
        <StatsCard
          title="Burn Rate"
          value={allocationTotal ? `${formatEur(allocationTotal.burn_rate_eur_per_hour, { minFrac: 4, maxFrac: 4 })}/h` : "-"}
          description="Current allocation (instances pricing)"
          icon={DollarSign}
          valueClassName="text-blue-600"
          iconClassName="text-blue-600"
        />
        <StatsCard
          title="Cumulative Spend"
          value={formatEur(cumulativeTotal, { minFrac: 4, maxFrac: 4 })}
          description="Sum of actual minute costs"
          icon={TrendingUp}
          valueClassName="text-purple-600"
          iconClassName="text-purple-600"
        />
      </div>

      {/* Quick Stats Cards */}
      <div className="grid gap-4 md:grid-cols-2">
        {/* Recent Activity */}
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

        {/* Quick Stats */}
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Zap className="h-5 w-5" />
              Quick Stats
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-4">
              <div className="flex items-center justify-between p-3 border rounded-lg">
                <span className="text-sm font-medium">Provisioning</span>
                <span className="text-2xl font-bold text-blue-600">
                  {stats.provisioning}
                </span>
              </div>
              <div className="flex items-center justify-between p-3 border rounded-lg">
                <span className="text-sm font-medium">Active Instances</span>
                <span className="text-2xl font-bold text-green-600">
                  {stats.active}
                </span>
              </div>
              <div className="flex items-center justify-between p-3 border rounded-lg">
                <span className="text-sm font-medium">Actual Spend (last minute)</span>
                <span className="text-2xl font-bold text-purple-600">
                  {formatEur(spend1m, { minFrac: 6, maxFrac: 6 })}
                </span>
              </div>
            </div>
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
                <div className="p-3 border rounded-lg">
                  <div className="text-xs text-muted-foreground">Allocation / minute</div>
                  <div className="text-xl font-bold">
                    {allocationTotal ? formatEur(allocationTotal.forecast_eur_per_minute, { minFrac: 6, maxFrac: 6 }) : "-"}
                  </div>
                </div>
                <div className="p-3 border rounded-lg">
                  <div className="text-xs text-muted-foreground">Allocation / hour</div>
                  <div className="text-xl font-bold">
                    {allocationTotal ? formatEur(allocationTotal.forecast_eur_per_hour, { minFrac: 4, maxFrac: 4 }) : "-"}
                  </div>
                </div>
                <div className="p-3 border rounded-lg">
                  <div className="text-xs text-muted-foreground">Allocation / day</div>
                  <div className="text-xl font-bold">
                    {allocationTotal ? formatEur(allocationTotal.forecast_eur_per_day, { minFrac: 4, maxFrac: 4 }) : "-"}
                  </div>
                </div>
                <div className="p-3 border rounded-lg">
                  <div className="text-xs text-muted-foreground">Allocation / 30d</div>
                  <div className="text-xl font-bold">
                    {allocationTotal ? formatEur(allocationTotal.forecast_eur_per_month_30d, { minFrac: 2, maxFrac: 2 }) : "-"}
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
                  <Tabs value={finops.window} onValueChange={(v) => finops.setWindow(v)}>
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
                  <div className="p-3 border rounded-lg">
                    <div className="text-xs text-muted-foreground">Cumulative (all time)</div>
                    <div className="text-xl font-bold">{formatEur(cumulativeTotal, { minFrac: 4, maxFrac: 4 })}</div>
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


