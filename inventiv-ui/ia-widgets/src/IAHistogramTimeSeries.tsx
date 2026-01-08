"use client";

import * as React from "react";
import { cn } from "./utils/cn";
import { withAlpha } from "./iaColors";

export type IAHistogramPoint = {
  t: string; // ISO timestamp
  v: number;
};

export type IAHistogramTick = {
  index: number; // index in points
  label: string;
};

export type IAHistogramTimeSeriesProps = {
  points: IAHistogramPoint[];
  width?: number;
  height?: number;
  color?: string;
  framed?: boolean;
  /** Optional x-axis tick labels (keep very sparse: 2-4). */
  ticks?: IAHistogramTick[];
  className?: string;
};

function finiteOrZero(v: unknown): number {
  return typeof v === "number" && Number.isFinite(v) ? v : 0;
}

export function IAHistogramTimeSeries({
  points,
  width = 420,
  height = 90,
  color = "#3B82F6",
  framed = true,
  ticks,
  className,
}: IAHistogramTimeSeriesProps) {
  const vals = React.useMemo(() => points.map((p) => finiteOrZero(p.v)), [points]);
  const n = Math.max(1, vals.length);
  const max = React.useMemo(() => Math.max(1e-9, ...vals), [vals]);

  const paddingTop = 8;
  const paddingBottom = ticks?.length ? 16 : 8;
  const paddingX = 10;
  const usableH = Math.max(8, height - paddingTop - paddingBottom);
  const usableW = Math.max(8, width - 2 * paddingX);

  const gap = n <= 40 ? 4 : n <= 120 ? 2 : 1;
  const barW = Math.max(1, (usableW - gap * (n - 1)) / n);
  const rx = n <= 40 ? Math.min(8, barW / 2) : 2;

  const svg = (
    <svg width={width} height={height} viewBox={`0 0 ${width} ${height}`} className="block">
      {/* grid */}
      <g opacity="0.35" strokeWidth="1" strokeDasharray="4 4" style={{ stroke: "hsl(var(--border))" }}>
        <line x1={paddingX} y1={paddingTop + usableH * 0.25} x2={width - paddingX} y2={paddingTop + usableH * 0.25} />
        <line x1={paddingX} y1={paddingTop + usableH * 0.5} x2={width - paddingX} y2={paddingTop + usableH * 0.5} />
        <line x1={paddingX} y1={paddingTop + usableH * 0.75} x2={width - paddingX} y2={paddingTop + usableH * 0.75} />
      </g>

      {/* bars */}
      {vals.map((v, i) => {
        const h = (v / max) * usableH;
        const x = paddingX + i * (barW + gap);
        const y = paddingTop + (usableH - h);
        return <rect key={i} x={x} y={y} width={barW} height={h} rx={rx} fill={withAlpha(color, 0.95)} />;
      })}

      {/* sparse x ticks */}
      {ticks?.length ? (
        <g>
          {ticks.map((t, idx) => {
            const clamped = Math.max(0, Math.min(n - 1, t.index));
            const x = paddingX + clamped * (barW + gap) + barW / 2;
            const y = height - 3;
            return (
              <text
                key={`${clamped}:${idx}`}
                x={x}
                y={y}
                textAnchor={idx === 0 ? "start" : idx === ticks.length - 1 ? "end" : "middle"}
                fontSize="10"
                style={{ fill: "hsl(var(--muted-foreground))" }}
              >
                {t.label}
              </text>
            );
          })}
        </g>
      ) : null}
    </svg>
  );

  if (!framed) return <span className={className}>{svg}</span>;
  return <div className={cn("inline-block rounded-lg border border-border bg-card p-0", className)}>{svg}</div>;
}


