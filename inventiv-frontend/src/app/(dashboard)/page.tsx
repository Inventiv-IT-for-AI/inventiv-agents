"use client";

import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { StatsCard } from "@/components/shared/StatsCard";
import { useInstances } from "@/hooks/useInstances";
import { useFinopsCosts } from "@/hooks/useFinops";
import { useCatalog } from "@/hooks/useCatalog";
import { Server, Activity, DollarSign, Zap, TrendingUp, Clock } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { displayOrDash, formatEur } from "@/lib/utils";
import { useEffect, useMemo } from "react";

export default function DashboardPage() {
  const { instances } = useInstances();
  const finops = useFinopsCosts();
  const catalog = useCatalog();

  useEffect(() => {
    catalog.fetchCatalog();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

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

  const forecastTotal = finops.current?.forecast?.find((r) => r.provider_id === null) ?? null;

  const providersById = useMemo(() => {
    const map = new Map<string, string>();
    for (const p of catalog.providers) map.set(p.id, p.name);
    return map;
  }, [catalog.providers]);

  const forecastProviders = useMemo(() => {
    const rows = finops.current?.forecast ?? [];
    return rows
      .filter((r) => r.provider_id !== null)
      .sort((a, b) => (b.burn_rate_eur_per_hour ?? 0) - (a.burn_rate_eur_per_hour ?? 0));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [finops.current]);

  const latestActualTotal = finops.actualTotalSeries?.[0]?.amount_eur ?? null;
  const cumulativeTotal = finops.current?.cumulative_total?.cumulative_amount_eur ?? null;

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
          value={forecastTotal ? `${formatEur(forecastTotal.burn_rate_eur_per_hour, { minFrac: 4, maxFrac: 4 })}/h` : "-"}
          description="Current allocation (forecast)"
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
                <span className="text-sm font-medium">Last Minute Cost</span>
                <span className="text-2xl font-bold text-purple-600">
                  {formatEur(latestActualTotal, { minFrac: 4, maxFrac: 4 })}
                </span>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>

      {/* FinOps breakdown */}
      <Card>
        <CardHeader>
          <CardTitle>FinOps – Costs & Forecast</CardTitle>
        </CardHeader>
        <CardContent>
          {finops.loading && !finops.current ? (
            <p className="text-sm text-muted-foreground">Loading FinOps data…</p>
          ) : finops.error ? (
            <p className="text-sm text-red-600">{finops.error}</p>
          ) : (
            <div className="space-y-3">
              <div className="grid gap-3 md:grid-cols-4">
                <div className="p-3 border rounded-lg">
                  <div className="text-xs text-muted-foreground">Forecast / day</div>
                  <div className="text-xl font-bold">
                    {forecastTotal ? formatEur(forecastTotal.forecast_eur_per_day, { minFrac: 4, maxFrac: 4 }) : "-"}
                  </div>
                </div>
                <div className="p-3 border rounded-lg">
                  <div className="text-xs text-muted-foreground">Forecast / month (30d)</div>
                  <div className="text-xl font-bold">
                    {forecastTotal ? formatEur(forecastTotal.forecast_eur_per_month_30d, { minFrac: 4, maxFrac: 4 }) : "-"}
                  </div>
                </div>
                <div className="p-3 border rounded-lg">
                  <div className="text-xs text-muted-foreground">Forecast / minute</div>
                  <div className="text-xl font-bold">
                    {forecastTotal ? formatEur(forecastTotal.forecast_eur_per_minute, { minFrac: 6, maxFrac: 6 }) : "-"}
                  </div>
                </div>
                <div className="p-3 border rounded-lg">
                  <div className="text-xs text-muted-foreground">Forecast / year (365d)</div>
                  <div className="text-xl font-bold">
                    {forecastTotal ? formatEur(forecastTotal.forecast_eur_per_year_365d, { minFrac: 2, maxFrac: 2 }) : "-"}
                  </div>
                </div>
              </div>

              {forecastProviders.length === 0 ? (
                <p className="text-sm text-muted-foreground">No provider forecast yet.</p>
              ) : (
                <div className="space-y-2">
                  {forecastProviders.map((r) => (
                    <div
                      key={r.provider_id as string}
                      className="flex items-center justify-between p-3 border rounded-lg"
                    >
                      <div>
                        <div className="font-medium text-sm">
                          {providersById.get(r.provider_id as string) ??
                            (r.provider_id as string)}
                        </div>
                        <div className="text-xs text-muted-foreground">
                          Burn rate: {formatEur(r.burn_rate_eur_per_hour, { minFrac: 4, maxFrac: 4 })}/h
                        </div>
                      </div>
                      <div className="text-right">
                        <div className="text-sm font-semibold">
                          {formatEur(r.forecast_eur_per_month_30d, { minFrac: 4, maxFrac: 4 })}/mo
                        </div>
                        <div className="text-xs text-muted-foreground">
                          {formatEur(r.forecast_eur_per_day, { minFrac: 4, maxFrac: 4 })}/day
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              )}

              <div className="pt-2">
                <div className="text-sm font-medium mb-2">Current costs (last full minute)</div>
                {finops.dashboardCurrent?.by_provider_minute?.length ? (
                  <div className="space-y-2">
                    {finops.dashboardCurrent.by_provider_minute.slice(0, 6).map((p) => (
                      <div key={p.provider_id} className="flex items-center justify-between p-3 border rounded-lg">
                        <div className="text-sm">
                          {p.provider_name}{" "}
                          <span className="text-xs text-muted-foreground">
                            ({p.provider_code ?? p.provider_id.slice(0, 8)})
                          </span>
                        </div>
                        <div className="font-semibold">
                          {formatEur(p.amount_eur, { minFrac: 6, maxFrac: 6 })}/min
                        </div>
                      </div>
                    ))}
                  </div>
                ) : (
                  <p className="text-sm text-muted-foreground">No cost data yet.</p>
                )}
              </div>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}


