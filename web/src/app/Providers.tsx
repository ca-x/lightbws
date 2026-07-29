import { InternationalizationProvider } from "@astryxdesign/core/i18n"
import { LayerProvider } from "@astryxdesign/core/Layer"
import { Theme } from "@astryxdesign/core/theme"
import { SpotlightProvider } from "react-tourlight"
import type { ReactNode } from "react"

import { I18nProvider, useI18n } from "../i18n/I18nProvider"
import { astryxThemes } from "../theme/astryxThemes"
import { ThemeProvider, useTheme } from "../theme/ThemeProvider"

export function Providers({ children }: { children: ReactNode }) {
  return <ThemeProvider><I18nProvider><AstryxRuntime>{children}</AstryxRuntime></I18nProvider></ThemeProvider>
}
function AstryxRuntime({ children }: { children: ReactNode }) {
  const { locale, t } = useI18n()
  const { resolved, themeName } = useTheme()
  return <Theme theme={astryxThemes[themeName]} mode={resolved}><InternationalizationProvider locale={locale}><LayerProvider toast={{ position: "topEnd", maxVisible: 3 }}><SpotlightProvider persist theme={resolved} transitionDuration={180} labels={{ next: t("tourNext"), previous: t("tourPrevious"), skip: t("tourSkip"), done: t("tourDone"), close: t("close"), stepOf: (current, total) => t("tourStepOf").replace("{current}", String(current)).replace("{total}", String(total)) }} navigate={(route) => { location.hash = route.replace(/^#/, "") }} isRouteActive={(route) => location.hash === route}>{children}</SpotlightProvider></LayerProvider></InternationalizationProvider></Theme>
}
