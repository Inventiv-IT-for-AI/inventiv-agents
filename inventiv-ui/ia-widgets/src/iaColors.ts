"use client";

/**
 * Flashy palette for dashboards / mini charts.
 * Keep it deterministic & reusable (no Tailwind class explosion).
 */
export const IA_COLORS = {
  amber: "#F59E0B",
  orange: "#F97316",
  red: "#EF4444",
  pink: "#EC4899",
  purple: "#8B5CF6",
  violet: "#A855F7",
  indigo: "#6366F1",
  blue: "#3B82F6",
  cyan: "#06B6D4",
  teal: "#14B8A6",
  green: "#22C55E",
  lime: "#84CC16",
} as const;

export type IAColorName = keyof typeof IA_COLORS;

export function withAlpha(hex: string, alpha: number) {
  const a = Math.max(0, Math.min(1, alpha));
  const h = hex.replace("#", "").trim();
  const full = h.length === 3 ? h.split("").map((c) => c + c).join("") : h;
  const r = parseInt(full.slice(0, 2), 16);
  const g = parseInt(full.slice(2, 4), 16);
  const b = parseInt(full.slice(4, 6), 16);
  return `rgba(${r}, ${g}, ${b}, ${a})`;
}

export function linearGradientBg(accentHex: string) {
  // Subtle dark gradient + colored glow on the left
  return {
    backgroundImage: `linear-gradient(135deg, ${withAlpha("#0B1220", 0.92)} 0%, ${withAlpha("#0B1220", 0.72)} 60%, ${withAlpha(accentHex, 0.14)} 100%)`,
  } as const;
}


