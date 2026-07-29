import "@astryxdesign/core/reset.css"
import "@astryxdesign/core/astryx.css"
import "@astryxdesign/theme-butter/theme.css"
import "@astryxdesign/theme-chocolate/theme.css"
import "@astryxdesign/theme-gothic/theme.css"
import "@astryxdesign/theme-matcha/theme.css"
import "@astryxdesign/theme-neutral/theme.css"
import "@astryxdesign/theme-stone/theme.css"
import "@astryxdesign/theme-y2k/theme.css"
import "react-tourlight/styles.css"
import "../tokens.css"
import "./styles/app.css"

import { StrictMode } from "react"
import { createRoot } from "react-dom/client"

import { App } from "./app/App"
import { Providers } from "./app/Providers"

const root = document.getElementById("root")
if (!root) throw new Error("LightBWS root element is missing")

createRoot(root).render(
  <StrictMode>
    <Providers><App /></Providers>
  </StrictMode>,
)
