import { createContext, useContext, useMemo, useState, type ReactNode } from "react"

import type { Locale } from "../app/types"
import { messages, type MessageKey } from "./messages"

interface I18nValue { locale: Locale; setLocale: (locale: Locale) => void; t: (key: MessageKey) => string }
const I18nContext = createContext<I18nValue | null>(null)

function initialLocale(): Locale {
  try {
    const stored = localStorage.getItem("lightbws-locale")
    if (stored === "en" || stored === "zh-CN") return stored
  } catch { /* storage may be blocked */ }
  return navigator.language.toLowerCase().startsWith("zh") ? "zh-CN" : "en"
}

export function I18nProvider({ children }: { children: ReactNode }) {
  const [locale, setLocaleState] = useState<Locale>(initialLocale)
  const value = useMemo<I18nValue>(() => ({
    locale,
    setLocale(next) {
      try { localStorage.setItem("lightbws-locale", next) } catch { /* storage may be blocked */ }
      document.documentElement.lang = next
      setLocaleState(next)
    },
    t: (key) => messages[locale][key],
  }), [locale])
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>
}

export function useI18n() {
  const value = useContext(I18nContext)
  if (!value) throw new Error("useI18n must be used inside I18nProvider")
  return value
}
