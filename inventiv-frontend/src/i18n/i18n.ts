import enUS from "./messages/en-US.json";
import frFR from "./messages/fr-FR.json";
import ar from "./messages/ar.json";

export const SUPPORTED_LOCALES = ["en-US", "fr-FR", "ar"] as const;
export type LocaleCode = (typeof SUPPORTED_LOCALES)[number];

const messagesByLocale: Record<LocaleCode, any> = {
  "en-US": enUS,
  "fr-FR": frFR,
  ar,
};

export function normalizeLocale(input?: string | null): LocaleCode {
  const s = (input ?? "").trim();
  if (s === "en-US" || s === "fr-FR" || s === "ar") return s;
  return "en-US";
}

export function isRtl(locale: LocaleCode): boolean {
  return locale === "ar";
}

function getPath(obj: any, path: string): any {
  const parts = path.split(".").filter(Boolean);
  let cur = obj;
  for (const p of parts) {
    if (cur && typeof cur === "object" && p in cur) cur = cur[p];
    else return undefined;
  }
  return cur;
}

export function t(locale: LocaleCode, key: string): string {
  const loc = normalizeLocale(locale);
  const dict = messagesByLocale[loc] ?? messagesByLocale["en-US"];
  const val = getPath(dict, key);
  if (typeof val === "string") return val;
  // fallback to en-US
  const fallback = getPath(messagesByLocale["en-US"], key);
  if (typeof fallback === "string") return fallback;
  return key;
}

export const LOCALE_LABELS: Record<LocaleCode, string> = {
  "en-US": "English (US)",
  "fr-FR": "Français (FR)",
  ar: "العربية",
};


