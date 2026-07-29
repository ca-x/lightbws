import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from "react"

import type { ThemeMode } from "../app/types"
import { isAstryxThemeName, type AstryxThemeName } from "./astryxThemes"

interface ThemeValue {
  mode: ThemeMode
  resolved: "light" | "dark"
  themeName: AstryxThemeName
  setMode: (mode: ThemeMode) => void
  setThemeName: (themeName: AstryxThemeName) => void
}

const ThemeContext = createContext<ThemeValue | null>(null)

function storedMode(): ThemeMode {
  try {
    const value = localStorage.getItem("lightbws-theme")
    return value === "light" || value === "dark" ? value : "system"
  } catch {
    return "system"
  }
}

function storedThemeName(): AstryxThemeName {
  try {
    const value = localStorage.getItem("lightbws-astryx-theme")
    return isAstryxThemeName(value) ? value : "neutral"
  } catch {
    return "neutral"
  }
}

function resolve(mode: ThemeMode) {
  return mode === "system"
    ? matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light"
    : mode
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [mode, setModeState] = useState<ThemeMode>(storedMode)
  const [themeName, setThemeNameState] = useState<AstryxThemeName>(storedThemeName)
  const [resolved, setResolved] = useState<"light" | "dark">(() => resolve(mode))

  useEffect(() => {
    const media = matchMedia("(prefers-color-scheme: dark)")
    const apply = () => {
      const next = resolve(mode)
      document.documentElement.dataset.theme = next
      document.documentElement.dataset.themePreference = mode
      setResolved(next)
    }
    apply()
    media.addEventListener("change", apply)
    return () => media.removeEventListener("change", apply)
  }, [mode])

  const value = useMemo<ThemeValue>(() => ({
    mode,
    resolved,
    themeName,
    setMode(next) {
      try { localStorage.setItem("lightbws-theme", next) } catch { /* storage may be blocked */ }
      setModeState(next)
    },
    setThemeName(next) {
      try { localStorage.setItem("lightbws-astryx-theme", next) } catch { /* storage may be blocked */ }
      document.documentElement.dataset.astryxTheme = next
      setThemeNameState(next)
    },
  }), [mode, resolved, themeName])

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>
}

export function useTheme() {
  const value = useContext(ThemeContext)
  if (!value) throw new Error("useTheme must be used inside ThemeProvider")
  return value
}
