"use client";

import React, { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";
import { isRtl, LocaleCode, normalizeLocale, t as translate } from "./i18n";

type I18nContextValue = {
  locale: LocaleCode;
  setLocale: (next: LocaleCode) => void;
  t: (key: string) => string;
};

const I18nContext = createContext<I18nContextValue | null>(null);

function applyDocumentLocale(locale: LocaleCode) {
  if (typeof document === "undefined") return;
  document.documentElement.lang = locale === "ar" ? "ar" : locale;
  document.documentElement.dir = isRtl(locale) ? "rtl" : "ltr";
  document.documentElement.dataset.locale = locale;
}

export function I18nProvider({
  children,
  initialLocale,
}: {
  children: React.ReactNode;
  initialLocale?: string | null;
}) {
  const [locale, _setLocale] = useState<LocaleCode>(() => normalizeLocale(initialLocale));

  const setLocale = useCallback((next: LocaleCode) => {
    const normalized = normalizeLocale(next);
    _setLocale(normalized);
    try {
      localStorage.setItem("inventiv_locale", normalized);
    } catch {
      // ignore
    }
    applyDocumentLocale(normalized);
  }, []);

  useEffect(() => {
    // Initialize from localStorage if present (useful on login screen), else keep initialLocale.
    try {
      const saved = localStorage.getItem("inventiv_locale");
      if (saved) {
        const normalized = normalizeLocale(saved);
        _setLocale(normalized);
        applyDocumentLocale(normalized);
        return;
      }
    } catch {
      // ignore
    }
    applyDocumentLocale(locale);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const t = useCallback(
    (key: string) => translate(locale, key),
    [locale]
  );

  const value = useMemo<I18nContextValue>(() => ({ locale, setLocale, t }), [locale, setLocale, t]);

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n() {
  const ctx = useContext(I18nContext);
  if (!ctx) throw new Error("useI18n must be used within I18nProvider");
  return ctx;
}


