"use client";

import * as React from "react";
import { cn } from "./utils/cn";
import { withAlpha } from "./iaColors";

export type IASparklineProps = {
  values: Array<number | null | undefined>;
  width?: number;
  height?: number;
  stroke?: string;
  strokeWidth?: number;
  /** show subtle fill under the line */
  fill?: boolean;
  /** background + border similar to the screenshots */
  framed?: boolean;
  className?: string;
};

function finiteOrNull(v: unknown): number | null {
  return typeof v === "number" && Number.isFinite(v) ? v : null;
}

export function IASparkline({
  values,
  width = 140,
  height = 36,
  stroke = "#22C55E",
  strokeWidth = 2,
  fill = true,
  framed = true,
  className,
}: IASparklineProps) {
  const pts = React.useMemo(() => values.map(finiteOrNull), [values]);
  const minMax = React.useMemo(() => {
    let min = Number.POSITIVE_INFINITY;
    let max = Number.NEGATIVE_INFINITY;
    for (const v of pts) {
      if (v === null) continue;
      if (v < min) min = v;
      if (v > max) max = v;
    }
    if (!Number.isFinite(min) || !Number.isFinite(max) || min === max) {
      return { min: 0, max: 1 };
    }
    return { min, max };
  }, [pts]);

  const poly = React.useMemo(() => {
    const n = pts.length;
    if (n <= 1) return "";
    const { min, max } = minMax;
    const dx = (width - 2) / (n - 1);
    const scale = max - min;
    const toY = (v: number) => (1 - (v - min) / scale) * (height - 2) + 1;
    const parts: string[] = [];
    for (let i = 0; i < n; i++) {
      const v = pts[i];
      const x = 1 + i * dx;
      const y = v === null ? height - 1 : toY(v);
      parts.push(`${x.toFixed(1)},${y.toFixed(1)}`);
    }
    return parts.join(" ");
  }, [height, minMax, pts, width]);

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
        <linearGradient id={`spark_${gradId}`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor={withAlpha(stroke, 0.35)} />
          <stop offset="100%" stopColor={withAlpha(stroke, 0)} />
        </linearGradient>
      </defs>

      {fillPath ? <path d={fillPath} fill={`url(#spark_${gradId})`} /> : null}
      {poly ? <polyline fill="none" stroke={stroke} strokeWidth={strokeWidth} points={poly} opacity="0.95" /> : null}
    </svg>
  );

  if (!framed) return <span className={className}>{svg}</span>;

  return (
    <div className={cn("inline-block rounded-lg border border-border bg-card p-0", className)}>
      {svg}
    </div>
  );
}


