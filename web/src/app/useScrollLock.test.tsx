import { cleanup, renderHook } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it } from "vitest"

import { useScrollLock } from "./useScrollLock"

beforeEach(() => {
  document.documentElement.style.overflow = "clip"
  document.body.style.overflow = "auto"
})

afterEach(() => {
  cleanup()
  document.documentElement.style.overflow = ""
  document.body.style.overflow = ""
})

describe("viewport scroll locking", () => {
  it("locks both the root viewport and body and restores their original values", () => {
    const lock = renderHook(() => useScrollLock(true))

    expect(document.documentElement.style.overflow).toBe("hidden")
    expect(document.body.style.overflow).toBe("hidden")

    lock.unmount()

    expect(document.documentElement.style.overflow).toBe("clip")
    expect(document.body.style.overflow).toBe("auto")
  })

  it("retains the viewport lock until every overlay closes, regardless of cleanup order", () => {
    const parent = renderHook(() => useScrollLock(true))
    const child = renderHook(() => useScrollLock(true))

    parent.unmount()

    expect(document.documentElement.style.overflow).toBe("hidden")
    expect(document.body.style.overflow).toBe("hidden")

    child.unmount()

    expect(document.documentElement.style.overflow).toBe("clip")
    expect(document.body.style.overflow).toBe("auto")
  })

  it("acquires and releases a lock as a drawer opens and closes", () => {
    const lock = renderHook(({ locked }) => useScrollLock(locked), { initialProps: { locked: false } })

    expect(document.documentElement.style.overflow).toBe("clip")

    lock.rerender({ locked: true })

    expect(document.documentElement.style.overflow).toBe("hidden")
    expect(document.body.style.overflow).toBe("hidden")

    lock.rerender({ locked: false })

    expect(document.documentElement.style.overflow).toBe("clip")
    expect(document.body.style.overflow).toBe("auto")
  })
})
