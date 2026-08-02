import { render, screen, waitFor } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { beforeEach, describe, expect, it, vi } from "vitest"

import { Providers } from "./Providers"
import type { Session } from "./types"

const mocks = vi.hoisted(() => ({
  login: vi.fn(),
  overview: vi.fn(),
  session: vi.fn(),
}))

vi.mock("./api", () => ({
  ApiError: class ApiError extends Error {},
  api: mocks,
}))

import { App } from "./App"

const session: Session = {
  csrfToken: "csrf",
  user: {
    id: "user-1",
    username: "admin",
    displayName: "Administrator",
    role: "admin",
    disabled: false,
    createdAt: 1,
    updatedAt: 1,
    lastLoginAt: 1,
  },
}

describe("authentication routes", () => {
  beforeEach(() => {
    vi.clearAllMocks()
    history.replaceState(null, "", "/")
    mocks.overview.mockResolvedValue({ projects: 0, secrets: 0, trash: 0 })
  })

  it("redirects signed-out users to /login and returns after login", async () => {
    mocks.session.mockRejectedValue(new Error("signed out"))
    mocks.login.mockResolvedValue(session)

    render(<Providers><App /></Providers>)

    await waitFor(() => expect(location.pathname).toBe("/login"))
    expect(screen.getByRole("heading", { level: 1, name: "Open your secure workspace" })).toBeInTheDocument()
    expect(screen.getAllByRole("heading", { level: 1 })).toHaveLength(1)
    await userEvent.type(screen.getByLabelText(/^Username/), "admin")
    await userEvent.type(screen.getByLabelText(/^Password/), "password")
    await userEvent.click(screen.getByRole("button", { name: "Sign in" }))

    await waitFor(() => expect(location.pathname).toBe("/"))
  })

  it("moves an authenticated user away from /login", async () => {
    history.replaceState(null, "", "/login")
    mocks.session.mockResolvedValue(session)

    render(<Providers><App /></Providers>)

    await waitFor(() => expect(location.pathname).toBe("/"))
  })
})
