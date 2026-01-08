"use client";

import * as React from "react";
import { cn } from "./utils/cn";
import { withAlpha } from "./iaColors";

export type IABarMiniChartProps = {
  values: Array<number | null | undefined>;
  width?: number;
  height?: number;
  color?: string;
  colors?: string[];
  labels?: string[];
  /** Show labels under bars (useful when the meaning isn't obvious). Default: false */
  showLabels?: boolean;
  framed?: boolean;
  className?: string;
};

function finiteOrZero(v: unknown): number {
  return typeof v === "number" && Number.isFinite(v) ? v : 0;
}

export function IABarMiniChart({
  values,
  width = 160,
  height = 48,
  color = "#8B5CF6",
  colors,
  labels,
  showLabels = false,
  framed = true,
  className,
}: IABarMiniChartProps) {
  const vals = React.useMemo(() => values.map(finiteOrZero), [values]);
  const max = React.useMemo(() => Math.max(1, ...vals), [vals]);
  const n = vals.length || 1;
  const gap = 6;
  const innerW = width - 2;
  const barW = Math.max(6, (innerW - gap * (n - 1)) / n);
  const labelH = showLabels ? 14 : 0;
  const baseY = height - 1 - labelH;
  const topPad = 6;
  const usableH = Math.max(4, height - topPad - 2 - labelH);
  const rx = Math.min(10, barW / 2);

  const svg = (
    <svg width={width} height={height} viewBox={`0 0 ${width} ${height}`} className="block">
      {/* grid */}
      <g opacity="0.35" strokeWidth="1" strokeDasharray="4 4" stroke="currentColor">
        <line x1="10" y1={topPad + usableH * 0.25} x2={width - 10} y2={topPad + usableH * 0.25} />
        <line x1="10" y1={topPad + usableH * 0.5} x2={width - 10} y2={topPad + usableH * 0.5} />
        <line x1="10" y1={topPad + usableH * 0.75} x2={width - 10} y2={topPad + usableH * 0.75} />
      </g>

      {vals.map((v, i) => {
        const h = (v / max) * usableH;
        const x = 1 + i * (barW + gap);
        const y = baseY - h;
        const c = colors?.[i % colors.length] ?? color;
        const label = labels?.[i];
        return (
          <g key={i}>
            <rect x={x} y={y} width={barW} height={h} rx={rx} fill={withAlpha(c, 0.95)} />
            {showLabels && label ? (
              <text
                x={x + barW / 2}
                y={height - 2}
                textAnchor="middle"
                fontSize="10"
                style={{ fill: "hsl(var(--muted-foreground))" }}
              >
                {label}
              </text>
            ) : null}
          </g>
        );
      })}
    </svg>
  );

  if (!framed) return <span className={className}>{svg}</span>;

  // IMPORTANT: do the "frame" in HTML (theme-aware), keep SVG transparent.
  return (
    <div
      className={cn("inline-block rounded-lg border border-border bg-card p-0", className)}
      style={{ color: "hsl(var(--border))" }}
    >
      {svg}
    </div>
  );
}


