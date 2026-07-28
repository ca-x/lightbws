;(() => {
  let preference = "system"
  let locale = null
  try {
    preference = localStorage.getItem("lightbws-theme") || "system"
    locale = localStorage.getItem("lightbws-locale")
  } catch {
    preference = "system"
  }
  const dark = preference === "dark" || (preference === "system" && matchMedia("(prefers-color-scheme: dark)").matches)
  const resolvedLocale = locale === "zh-CN" || locale === "en"
    ? locale
    : navigator.language.toLowerCase().startsWith("zh") ? "zh-CN" : "en"
  document.documentElement.dataset.theme = dark ? "dark" : "light"
  document.documentElement.dataset.themePreference = preference
  document.documentElement.dataset.astryxTheme = "neutral"
  document.documentElement.lang = resolvedLocale
})()
