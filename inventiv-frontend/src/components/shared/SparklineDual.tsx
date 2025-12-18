"use client";

import { useMemo } from "react";

type Point = { a: number | null | undefined; b: number | null | undefined };

type SparklineDualProps = {
  points: Point[];
  width?: number;
  height?: number;
  strokeA?: string; // green GPU
  strokeB?: string; // orange VRAM
};

function clamp01(v: number) {
  if (v < 0) return 0;
  if (v > 100) return 100;
  return v;
}

export function SparklineDual({
  points,
  width = 220,
  height = 48,
  strokeA = "#22c55e",
  strokeB = "#f97316",
}: SparklineDualProps) {
  const { polyA, polyB } = useMemo(() => {
    const n = points.length;
    if (n <= 1) return { polyA: "", polyB: "" };

    const toXY = (idx: number, v: number | null | undefined) => {
      const x = (idx / (n - 1)) * (width - 2) + 1;
      const y = (1 - clamp01(typeof v === "number" ? v : 0) / 100) * (height - 2) + 1;
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    };

    const a = points.map((p, i) => toXY(i, p.a)).join(" ");
    const b = points.map((p, i) => toXY(i, p.b)).join(" ");
    return { polyA: a, polyB: b };
  }, [points, width, height]);

  return (
    <svg width={width} height={height} viewBox={`0 0 ${width} ${height}`} className="block">
      <rect x="0" y="0" width={width} height={height} rx="6" fill="#050505" stroke="#222" />
      <polyline fill="none" stroke={strokeB} strokeWidth="2" points={polyB} opacity="0.9" />
      <polyline fill="none" stroke={strokeA} strokeWidth="2" points={polyA} opacity="0.9" />
    </svg>
  );
}


