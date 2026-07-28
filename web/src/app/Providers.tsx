import { InternationalizationProvider } from "@astryxdesign/core/i18n"
import { LayerProvider } from "@astryxdesign/core/Layer"
import { Theme } from "@astryxdesign/core/theme"
import { neutralTheme } from "@astryxdesign/theme-neutral/built"
import type { ReactNode } from "react"

import { I18nProvider, useI18n } from "../i18n/I18nProvider"
import { ThemeProvider, useTheme } from "../theme/ThemeProvider"

export function Providers({ children }: { children: ReactNode }) {
  return <ThemeProvider><I18nProvider><AstryxRuntime>{children}</AstryxRuntime></I18nProvider></ThemeProvider>
}
function AstryxRuntime({ children }: { children: ReactNode }) {
  const { locale } = useI18n()
  const { resolved } = useTheme()
  return <Theme theme={neutralTheme} mode={resolved}><InternationalizationProvider locale={locale}><LayerProvider toast={{ position: "topEnd", maxVisible: 3 }}>{children}</LayerProvider></InternationalizationProvider></Theme>
}
