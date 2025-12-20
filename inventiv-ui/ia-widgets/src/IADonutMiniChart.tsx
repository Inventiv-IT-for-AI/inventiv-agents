"use client";

import * as React from "react";
import { cn } from "./utils/cn";
import { withAlpha } from "./iaColors";

export type IADonutSegment = {
  label?: string;
  value: number;
  color: string;
};

export type IADonutMiniChartProps = {
  segments: IADonutSegment[];
  size?: number; // square
  strokeWidth?: number;
  framed?: boolean;
  centerLabel?: string;
  subLabel?: string;
  /** Render segment labels around the donut (useful for "top providers" donuts). Default: false */
  showSegmentLabels?: boolean;
  /** Optional label formatter (defaults to segment.label). */
  formatSegmentLabel?: (seg: IADonutSegment, idx: number) => string;
  className?: string;
};

export function IADonutMiniChart({
  segments,
  size = 120,
  strokeWidth = 16,
  framed = true,
  centerLabel,
  subLabel,
  showSegmentLabels = false,
  formatSegmentLabel,
  className,
}: IADonutMiniChartProps) {
  const total = segments.reduce((acc, s) => acc + (Number.isFinite(s.value) ? Math.max(0, s.value) : 0), 0) || 1;
  const r = (size - strokeWidth) / 2;
  const c = size / 2;
  const circumference = 2 * Math.PI * r;
  const startAngle = -Math.PI / 2; // 12 o'clock

  // Start at top (12 o'clock)
  let offset = circumference * 0.25;
  let angleCursor = startAngle;

  const svg = (
    <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} className="block">

      {/* track */}
      <circle cx={c} cy={c} r={r} fill="none" stroke="currentColor" opacity="0.18" strokeWidth={strokeWidth} />

      {/* arcs */}
      {segments.map((s, idx) => {
        const v = Number.isFinite(s.value) ? Math.max(0, s.value) : 0;
        const dash = (v / total) * circumference;
        const segAngle = (v / total) * Math.PI * 2;
        const dasharray = `${dash} ${circumference - dash}`;
        const dashoffset = offset;
        offset -= dash;
        const midAngle = angleCursor + segAngle / 2;
        angleCursor += segAngle;

        const labelText = formatSegmentLabel ? formatSegmentLabel(s, idx) : (s.label ?? "");
        const labelRadius = r + strokeWidth / 2 + 14;
        const lx = c + Math.cos(midAngle) * labelRadius;
        const ly = c + Math.sin(midAngle) * labelRadius;

        return (
          <g key={idx}>
            <circle
              cx={c}
              cy={c}
              r={r}
              fill="none"
              stroke={s.color}
              strokeWidth={strokeWidth}
              strokeLinecap="round"
              strokeDasharray={dasharray}
              strokeDashoffset={dashoffset}
              transform={`rotate(-90 ${c} ${c})`}
            />

            {showSegmentLabels && labelText ? (
              <text
                x={lx}
                y={ly}
                textAnchor={lx >= c ? "start" : "end"}
                dominantBaseline="central"
                fontSize="10"
                fontWeight="600"
                style={{
                  fill: "hsl(var(--foreground))",
                  paintOrder: "stroke",
                  stroke: "hsl(var(--card))",
                  strokeWidth: 4,
                }}
              >
                {labelText}
              </text>
            ) : null}
          </g>
        );
      })}

      {/* center text */}
      {centerLabel ? (
        <text
          x={c}
          y={c - 2}
          textAnchor="middle"
          dominantBaseline="central"
          fill="hsl(var(--foreground))"
          fontSize="18"
          fontWeight="700"
        >
          {centerLabel}
        </text>
      ) : null}
      {subLabel ? (
        <text
          x={c}
          y={c + 18}
          textAnchor="middle"
          dominantBaseline="central"
          fill="hsl(var(--muted-foreground))"
          fontSize="11"
        >
          {subLabel}
        </text>
      ) : null}
    </svg>
  );

  if (!framed) return <span className={className}>{svg}</span>;

  return (
    <div
      className={cn("inline-block rounded-xl border border-border bg-card p-0", className)}
      style={{ color: "hsl(var(--muted-foreground))" }}
    >
      {svg}
    </div>
  );
}


