import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from "react"

import type { ThemeMode } from "../app/types"

interface ThemeValue { mode: ThemeMode; resolved: "light" | "dark"; setMode: (mode: ThemeMode) => void }
const ThemeContext = createContext<ThemeValue | null>(null)

function initialMode(): ThemeMode {
  try {
    const value = localStorage.getItem("lightbws-theme")
    return value === "light" || value === "dark" ? value : "system"
  } catch { return "system" }
}
function resolve(mode: ThemeMode) { return mode === "system" ? matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light" : mode }

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [mode, setModeState] = useState<ThemeMode>(initialMode)
  const [resolved, setResolved] = useState<"light" | "dark">(() => resolve(mode))
  useEffect(() => {
    const media = matchMedia("(prefers-color-scheme: dark)")
    const apply = () => { const next = resolve(mode); document.documentElement.dataset.theme = next; document.documentElement.dataset.themePreference = mode; setResolved(next) }
    apply(); media.addEventListener("change", apply); return () => media.removeEventListener("change", apply)
  }, [mode])
  const value = useMemo(() => ({ mode, resolved, setMode(next: ThemeMode) { try { localStorage.setItem("lightbws-theme", next) } catch { /* storage may be blocked */ }; setModeState(next) } }), [mode, resolved])
  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>
}
export function useTheme() { const value = useContext(ThemeContext); if (!value) throw new Error("useTheme must be used inside ThemeProvider"); return value }
