;(() => {
  const availableThemes = ["neutral", "stone", "butter", "matcha", "chocolate", "gothic", "y2k"]
  let astryxTheme = "neutral"
  try {
    const storedTheme = localStorage.getItem("lightbws-astryx-theme")
    astryxTheme = availableThemes.includes(storedTheme) ? storedTheme : "neutral"
  } catch {
    astryxTheme = "neutral"
  }
  document.documentElement.dataset.astryxTheme = astryxTheme

  let preference = "system"
  let locale = null
  try {
    preference = localStorage.getItem("lightbws-theme") || "system"
    locale = localStorage.getItem("lightbws-locale")
  } catch {
    preference = "system"
  }
  const resolvedLocale = locale === "zh-CN" || locale === "en"
    ? locale
    : navigator.language.toLowerCase().startsWith("zh") ? "zh-CN" : "en"
  try {
    const dark = preference === "dark" || (preference === "system" && matchMedia("(prefers-color-scheme: dark)").matches)
    document.documentElement.dataset.theme = dark ? "dark" : "light"
    document.documentElement.dataset.themePreference = preference
  } catch {
    document.documentElement.dataset.theme = "light"
    document.documentElement.dataset.themePreference = "system"
  }
  document.documentElement.lang = resolvedLocale
})()
