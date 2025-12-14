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
