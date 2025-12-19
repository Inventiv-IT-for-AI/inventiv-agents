"use client";

import type { LucideIcon } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/card";

export type IAStatsCardProps = {
  title: string;
  value: string | number;
  description?: string;
  icon: LucideIcon;
  iconClassName?: string;
  valueClassName?: string;
};

export function IAStatsCard({
  title,
  value,
  description,
  icon: Icon,
  iconClassName = "text-muted-foreground",
  valueClassName = "",
}: IAStatsCardProps) {
  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
        <CardTitle className="text-sm font-medium">{title}</CardTitle>
        <Icon className={`h-4 w-4 ${iconClassName}`} />
      </CardHeader>
      <CardContent>
        <div className={`text-2xl font-bold ${valueClassName}`}>{value}</div>
        {description ? <p className="text-xs text-muted-foreground">{description}</p> : null}
      </CardContent>
    </Card>
  );
}


