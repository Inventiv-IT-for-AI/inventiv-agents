"use client";

import * as React from "react";
import { cn } from "./utils/cn";
import { withAlpha } from "./iaColors";

export type IATimeSeriesPoint = {
  /** ISO timestamp or any label (only used for tooltips/labels in future) */
  t: string;
  v: number;
};

export type IALineTimeSeriesProps = {
  points: IATimeSeriesPoint[];
  width?: number;
  height?: number;
  stroke?: string;
  fill?: boolean;
  framed?: boolean;
  className?: string;
};

function finiteOrZero(v: unknown): number {
  return typeof v === "number" && Number.isFinite(v) ? v : 0;
}

export function IALineTimeSeries({
  points,
  width = 360,
  height = 84,
  stroke = "#3B82F6",
  fill = true,
  framed = true,
  className,
}: IALineTimeSeriesProps) {
  const values = React.useMemo(() => points.map((p) => finiteOrZero(p.v)), [points]);

  const minMax = React.useMemo(() => {
    let min = Number.POSITIVE_INFINITY;
    let max = Number.NEGATIVE_INFINITY;
    for (const v of values) {
      if (!Number.isFinite(v)) continue;
      if (v < min) min = v;
      if (v > max) max = v;
    }
    if (!Number.isFinite(min) || !Number.isFinite(max) || min === max) return { min: 0, max: 1 };
    return { min, max };
  }, [values]);

  const poly = React.useMemo(() => {
    const n = values.length;
    if (n <= 1) return "";
    const { min, max } = minMax;
    const dx = (width - 2) / (n - 1);
    const scale = max - min;
    const toY = (v: number) => (1 - (v - min) / scale) * (height - 2) + 1;
    const parts: string[] = [];
    for (let i = 0; i < n; i++) {
      const x = 1 + i * dx;
      const y = toY(values[i]);
      parts.push(`${x.toFixed(1)},${y.toFixed(1)}`);
    }
    return parts.join(" ");
  }, [height, minMax, values, width]);

  const fillPath = React.useMemo(() => {
    if (!fill || !poly) return "";
    const first = poly.split(" ")[0];
    const last = poly.split(" ").at(-1);
    if (!first || !last) return "";
    const [x0] = first.split(",");
    const [x1] = last.split(",");
    return `M ${poly.replaceAll(" ", " L ")} L ${x1} ${height - 1} L ${x0} ${height - 1} Z`;
  }, [fill, height, poly]);

  const gradId = React.useId().replace(/:/g, "_");

  const svg = (
    <svg width={width} height={height} viewBox={`0 0 ${width} ${height}`} className="block">
      <defs>
        <linearGradient id={`line_${gradId}`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor={withAlpha(stroke, 0.35)} />
          <stop offset="100%" stopColor={withAlpha(stroke, 0)} />
        </linearGradient>
      </defs>

      {/* grid */}
      <g opacity="0.35" strokeWidth="1" strokeDasharray="4 4" style={{ stroke: "hsl(var(--border))" }}>
        <line x1="10" y1={height * 0.25} x2={width - 10} y2={height * 0.25} />
        <line x1="10" y1={height * 0.5} x2={width - 10} y2={height * 0.5} />
        <line x1="10" y1={height * 0.75} x2={width - 10} y2={height * 0.75} />
      </g>

      {fillPath ? <path d={fillPath} fill={`url(#line_${gradId})`} /> : null}
      {poly ? <polyline fill="none" stroke={stroke} strokeWidth="2.5" points={poly} opacity="0.95" /> : null}
    </svg>
  );

  if (!framed) return <span className={className}>{svg}</span>;
  return <div className={cn("inline-block rounded-lg border border-border bg-card p-0", className)}>{svg}</div>;
}


