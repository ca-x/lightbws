import { useEffect } from "react"

let activeLocks = 0
let previousOverflow = { root: "", body: "" }

export function useScrollLock(locked: boolean) {
  useEffect(() => {
    if (!locked) return
    if (activeLocks === 0) {
      previousOverflow = {
        root: document.documentElement.style.overflow,
        body: document.body.style.overflow,
      }
      document.documentElement.style.overflow = "hidden"
      document.body.style.overflow = "hidden"
    }
    activeLocks += 1
    return () => {
      activeLocks -= 1
      if (activeLocks !== 0) return
      document.documentElement.style.overflow = previousOverflow.root
      document.body.style.overflow = previousOverflow.body
    }
  }, [locked])
}
