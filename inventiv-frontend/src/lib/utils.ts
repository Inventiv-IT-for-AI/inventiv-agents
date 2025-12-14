import { clsx, type ClassValue } from "clsx"
import { twMerge } from "tailwind-merge"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

export function displayOrDash(value?: string | null) {
  const v = (value ?? "").trim();
  if (!v) return "-";
  if (/^Unknown\s+/i.test(v)) return "-";
  return v;
}

export function asNumber(value: unknown): number | null {
  if (value === null || value === undefined) return null;
  if (typeof value === "number") return Number.isFinite(value) ? value : null;
  if (typeof value === "string") {
    const n = Number(value);
    return Number.isFinite(n) ? n : null;
  }
  return null;
}

export function formatEur(value: unknown, opts?: { minFrac?: number; maxFrac?: number }) {
  const n = asNumber(value);
  if (n === null) return "-";
  const minFrac = opts?.minFrac ?? 2;
  const maxFrac = opts?.maxFrac ?? 4;
  return new Intl.NumberFormat("fr-FR", {
    style: "currency",
    currency: "EUR",
    minimumFractionDigits: minFrac,
    maximumFractionDigits: maxFrac,
  }).format(n);
}

// Tailwind safelist for runtime-provided classes (e.g. DB-driven `action_types.color_class`).
// Tailwind v4 only generates utilities it can see statically in the codebase.
// Keep this list in sync with seeded action_types badge colors.
export const __tailwindSafelistActionTypeBadges = [
  "text-white",
  "bg-blue-500",
  "hover:bg-blue-600",
  "bg-blue-600",
  "hover:bg-blue-700",
  "bg-purple-500",
  "hover:bg-purple-600",
  "bg-purple-600",
  "hover:bg-purple-700",
  "bg-orange-500",
  "hover:bg-orange-600",
  "bg-orange-600",
  "hover:bg-orange-700",
  "bg-indigo-500",
  "hover:bg-indigo-600",
  "bg-green-500",
  "hover:bg-green-600",
  "bg-green-600",
  "hover:bg-green-700",
  "bg-teal-600",
  "hover:bg-teal-700",
  "bg-gray-600",
  "hover:bg-gray-700",
  "bg-yellow-500",
  "hover:bg-yellow-600",
  "bg-yellow-600",
  "hover:bg-yellow-700",
  "bg-red-500",
  "hover:bg-red-600",
].join(" ");
