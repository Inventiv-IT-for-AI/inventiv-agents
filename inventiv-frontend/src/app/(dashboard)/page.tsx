"use client";

import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { StatsCard } from "@/components/shared/StatsCard";
import { useInstances } from "@/hooks/useInstances";
import { Server, Activity, DollarSign, Zap, TrendingUp, Clock } from "lucide-react";
import { Badge } from "@/components/ui/badge";

export default function DashboardPage() {
  const { instances } = useInstances();

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
          title="Total Cost"
          value={`$${stats.totalCost.toFixed(2)}`}
          description="Accumulated spend"
          icon={DollarSign}
          valueClassName="text-blue-600"
          iconClassName="text-blue-600"
        />
        <StatsCard
          title="Avg Cost/Hour"
          value={`$${stats.avgCostPerHour.toFixed(2)}`}
          description="Across all instances"
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
                        {instance.instance_type}
                      </p>
                      <p className="text-xs text-muted-foreground">
                        {instance.zone} • {instance.provider_name}
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
                <span className="text-sm font-medium">Total Spend</span>
                <span className="text-2xl font-bold text-purple-600">
                  ${stats.totalCost.toFixed(2)}
                </span>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Placeholder for future charts */}
      <Card>
        <CardHeader>
          <CardTitle>Usage & Cost Trends</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="h-64 flex items-center justify-center text-muted-foreground">
            <div className="text-center">
              <TrendingUp className="h-12 w-12 mx-auto mb-2 opacity-50" />
              <p>Charts and graphs coming soon...</p>
              <p className="text-sm">
                Track your usage patterns and costs over time
              </p>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}


