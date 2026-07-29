import { Check } from "lucide-react"

import { useI18n } from "../i18n/I18nProvider"
import { astryxThemeNames, type AstryxThemeName } from "./astryxThemes"
import { useTheme } from "./ThemeProvider"

export function ThemePicker({ labels }: { labels: Record<AstryxThemeName, string> }) {
  const { t } = useI18n()
  const { resolved, themeName, setThemeName } = useTheme()

  return (
    <fieldset className="theme-grid" aria-label={t("themeStyle")}>
      <legend className="visually-hidden">{t("themeStyle")}</legend>
      {astryxThemeNames.map((name) => {
        const selected = themeName === name
        return (
          <label className="theme-option" data-selected={selected || undefined} key={name}>
            <input
              className="theme-radio"
              type="radio"
              name="astryx-theme"
              value={name}
              checked={selected}
              onChange={() => setThemeName(name)}
            />
            <span
              className="theme-preview"
              data-astryx-theme={name}
              data-theme={resolved}
              style={{ colorScheme: resolved }}
              aria-hidden="true"
            >
              <span className="theme-preview-sidebar" />
              <span className="theme-preview-content">
                <span className="theme-preview-accent" />
                <span className="theme-preview-line" />
                <span className="theme-preview-line short" />
              </span>
            </span>
            <span className="theme-option-label"><span>{labels[name]}</span>{selected && <Check aria-hidden="true" />}</span>
          </label>
        )
      })}
    </fieldset>
  )
}
