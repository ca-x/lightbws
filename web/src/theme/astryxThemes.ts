import type { DefinedTheme } from "@astryxdesign/core/theme"
import { butterTheme } from "@astryxdesign/theme-butter/built"
import { chocolateTheme } from "@astryxdesign/theme-chocolate/built"
import { gothicTheme } from "@astryxdesign/theme-gothic/built"
import { matchaTheme } from "@astryxdesign/theme-matcha/built"
import { neutralTheme } from "@astryxdesign/theme-neutral/built"
import { stoneTheme } from "@astryxdesign/theme-stone/built"
import { y2kTheme } from "@astryxdesign/theme-y2k/built"

export const astryxThemeNames = [
  "neutral",
  "stone",
  "butter",
  "matcha",
  "chocolate",
  "gothic",
  "y2k",
] as const

export type AstryxThemeName = typeof astryxThemeNames[number]

export const astryxThemes: Record<AstryxThemeName, DefinedTheme> = {
  neutral: neutralTheme,
  stone: stoneTheme,
  butter: butterTheme,
  matcha: matchaTheme,
  chocolate: chocolateTheme,
  gothic: gothicTheme,
  y2k: y2kTheme,
}

export function isAstryxThemeName(value: string | null): value is AstryxThemeName {
  return astryxThemeNames.some((name) => name === value)
}
