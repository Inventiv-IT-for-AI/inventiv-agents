"use client";

import * as React from "react";
import { cn } from "./utils/cn";
import { withAlpha } from "./iaColors";

export type IASparklineDualPoint = { a: number | null | undefined; b: number | null | undefined };

export type IASparklineDualProps = {
  points: IASparklineDualPoint[];
  width?: number;
  height?: number;
  strokeA?: string;
  strokeB?: string;
  framed?: boolean;
  className?: string;
};

function clamp01(v: number) {
  if (v < 0) return 0;
  if (v > 1) return 1;
  return v;
}

function finiteOrNull(v: unknown): number | null {
  return typeof v === "number" && Number.isFinite(v) ? v : null;
}

export function IASparklineDual({
  points,
  width = 160,
  height = 36,
  strokeA = "#22C55E",
  strokeB = "#F97316",
  framed = true,
  className,
}: IASparklineDualProps) {
  const aVals = React.useMemo(() => points.map((p) => finiteOrNull(p.a)), [points]);
  const bVals = React.useMemo(() => points.map((p) => finiteOrNull(p.b)), [points]);

  const toPoly = React.useCallback(
    (vals: Array<number | null>) => {
      const n = vals.length;
      if (n <= 1) return "";
      // Normalize to [0..1] using min/max of this series (avoids flattening when scales differ)
      let min = Number.POSITIVE_INFINITY;
      let max = Number.NEGATIVE_INFINITY;
      for (const v of vals) {
        if (v === null) continue;
        if (v < min) min = v;
        if (v > max) max = v;
      }
      if (!Number.isFinite(min) || !Number.isFinite(max) || min === max) {
        min = 0;
        max = 1;
      }
      const dx = (width - 2) / (n - 1);
      const scale = max - min;
      const parts: string[] = [];
      for (let i = 0; i < n; i++) {
        const v = vals[i];
        const x = 1 + i * dx;
        const norm = v === null ? 0 : clamp01((v - min) / scale);
        const y = (1 - norm) * (height - 2) + 1;
        parts.push(`${x.toFixed(1)},${y.toFixed(1)}`);
      }
      return parts.join(" ");
    },
    [height, width]
  );

  const polyA = React.useMemo(() => toPoly(aVals), [aVals, toPoly]);
  const polyB = React.useMemo(() => toPoly(bVals), [bVals, toPoly]);

  const svg = (
    <svg width={width} height={height} viewBox={`0 0 ${width} ${height}`} className="block">
      {polyB ? <polyline fill="none" stroke={strokeB} strokeWidth="2" points={polyB} opacity="0.95" /> : null}
      {polyA ? <polyline fill="none" stroke={strokeA} strokeWidth="2" points={polyA} opacity="0.95" /> : null}

      {/* subtle legend dots (tiny) */}
      <circle cx={width - 26} cy={height - 10} r="2.5" fill={withAlpha(strokeA, 0.95)} />
      <circle cx={width - 14} cy={height - 10} r="2.5" fill={withAlpha(strokeB, 0.95)} />
    </svg>
  );

  if (!framed) return <span className={className}>{svg}</span>;

  return (
    <div className={cn("inline-block rounded-lg border border-border bg-card p-0", className)}>
      {svg}
    </div>
  );
}


