"use client";

import * as React from "react";
import type { LucideIcon } from "lucide-react";
import { TrendingDown, TrendingUp } from "lucide-react";
import { cn } from "./utils/cn";
import { IA_COLORS, type IAColorName, withAlpha } from "./iaColors";

export type IAStatDelta = {
  /** e.g. 12.5 means +12.5% */
  pct: number;
  label?: string; // e.g. "vs last month"
};

export type IAStatCellProps = {
  title: string;
  value: string | number;
  subtitle?: string;
  icon?: LucideIcon;
  /** Flashy accent color */
  accent?: IAColorName | string;
  /** Optional delta pill */
  delta?: IAStatDelta;
  className?: string;
};

function formatDelta(pct: number) {
  const sign = pct > 0 ? "+" : "";
  return `${sign}${pct.toFixed(1)}%`;
}

export function IAStatCell({
  title,
  value,
  subtitle,
  icon: Icon,
  accent = "blue",
  delta,
  className,
}: IAStatCellProps) {
  const accentHex = (accent in IA_COLORS ? IA_COLORS[accent as IAColorName] : String(accent)) as string;
  const up = typeof delta?.pct === "number" && delta.pct >= 0;
  const DeltaIcon = up ? TrendingUp : TrendingDown;

  return (
    <div
      className={cn(
        // Theme-aware surface
        "relative overflow-hidden rounded-xl border border-border bg-card text-card-foreground",
        "px-4 py-3",
        className
      )}
    >
      {/* Subtle accent glow overlay (works in light/dark) */}
      <div
        className="absolute inset-0 pointer-events-none"
        style={{
          backgroundImage: `radial-gradient(120px 80px at 12% 10%, ${withAlpha(
            accentHex,
            0.22
          )} 0%, transparent 60%), radial-gradient(220px 120px at 100% 0%, ${withAlpha(
            accentHex,
            0.12
          )} 0%, transparent 55%)`,
        }}
      />
      {/* Accent rail */}
      <div
        className="absolute left-0 top-0 h-full w-1"
        style={{ background: `linear-gradient(180deg, ${accentHex} 0%, ${withAlpha(accentHex, 0.3)} 100%)` }}
      />

      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="text-xs text-muted-foreground truncate">{title}</div>
          <div className="mt-1 text-2xl font-semibold tracking-tight tabular-nums truncate">{value}</div>
          {subtitle ? <div className="mt-1 text-xs text-muted-foreground truncate">{subtitle}</div> : null}
        </div>

        <div className="flex flex-col items-end gap-2 shrink-0">
          {Icon ? (
            <div
              className="rounded-lg p-2 border border-white/10"
              style={{ backgroundColor: withAlpha(accentHex, 0.12) }}
              title={title}
            >
              <Icon className="h-5 w-5" style={{ color: accentHex }} />
            </div>
          ) : null}

          {delta ? (
            <div
              className={cn(
                "inline-flex items-center gap-1 rounded-full px-2 py-1 text-[11px] font-medium border",
                up ? "text-emerald-200" : "text-red-200"
              )}
              style={{
                backgroundColor: up ? withAlpha(IA_COLORS.green, 0.12) : withAlpha(IA_COLORS.red, 0.12),
                borderColor: up ? withAlpha(IA_COLORS.green, 0.25) : withAlpha(IA_COLORS.red, 0.25),
              }}
              title={delta.label ?? "delta"}
            >
              <DeltaIcon className="h-3.5 w-3.5" />
              <span className="tabular-nums">{formatDelta(delta.pct)}</span>
              {delta.label ? <span className="text-[11px] text-muted-foreground ml-1">{delta.label}</span> : null}
            </div>
          ) : null}
        </div>
      </div>
    </div>
  );
}


