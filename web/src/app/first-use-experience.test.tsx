import { cleanup, render, screen, waitFor } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { Providers } from "./Providers"
import type { Secret, Session, User } from "./types"

const mocks = vi.hoisted(() => ({
  groups: vi.fn(),
  overview: vi.fn(),
  projects: vi.fn(),
  secrets: vi.fn(),
  session: vi.fn(),
  users: vi.fn(),
}))

vi.mock("./api", () => ({
  ApiError: class ApiError extends Error {},
  api: mocks,
}))

import { App } from "./App"

const admin: User = {
  id: "user-1",
  username: "admin",
  displayName: "Administrator",
  role: "admin",
  disabled: false,
  createdAt: 1,
  updatedAt: 1,
  lastLoginAt: 1,
}
const session: Session = { csrfToken: "csrf", user: admin }
const secret: Secret = {
  id: "secret-1",
  projectId: null,
  key: "deepseek-api-key",
  value: "secret-value",
  note: "",
  sdkEncrypted: false,
  deletedAt: null,
  permissions: { read: true, write: true },
  createdAt: 1,
  updatedAt: 1,
}

describe("first-use experience", () => {
  afterEach(cleanup)

  beforeEach(() => {
    vi.clearAllMocks()
    const stored = new Map<string, string>()
    Object.defineProperty(globalThis, "localStorage", { configurable: true, value: { clear: () => stored.clear(), getItem: (key: string) => stored.get(key) ?? null, removeItem: (key: string) => stored.delete(key), setItem: (key: string, value: string) => stored.set(key, value) } })
    localStorage.clear()
    localStorage.setItem("lightbws-onboarding-v1", "done")
    mocks.session.mockResolvedValue(session)
    mocks.overview.mockResolvedValue({ projects: 0, secrets: 0, trash: 0 })
    mocks.projects.mockResolvedValue([])
    mocks.secrets.mockResolvedValue([secret])
    mocks.groups.mockResolvedValue([])
    mocks.users.mockResolvedValue([admin])
    Object.assign(navigator, { clipboard: { writeText: vi.fn().mockResolvedValue(undefined) } })
  })

  it("shows the minimum first-use path and current service endpoint", async () => {
    history.replaceState(null, "", "/#dashboard")
    render(<Providers><App /></Providers>)

    expect(await screen.findByRole("heading", { name: "Start with one secret" })).toBeInTheDocument()
    expect(screen.getByText(location.origin)).toBeInTheDocument()
    expect(screen.getByText("You only need the service endpoint and one secret to begin. Projects are optional.")).toBeInTheDocument()
  })

  it("keeps the tour active while navigating between workspace pages", async () => {
    history.replaceState(null, "", "/#dashboard")
    render(<Providers><App /></Providers>)

    await userEvent.click(await screen.findByRole("button", { name: "Show me around" }))
    expect(await screen.findByRole("dialog", { name: "The shortest path" })).toBeInTheDocument()
    await userEvent.click(screen.getByRole("button", { name: "Next" }))
    expect(await screen.findByRole("dialog", { name: "Your service endpoint" })).toBeInTheDocument()
    await userEvent.click(screen.getByRole("button", { name: "Next" }))
    expect(await screen.findByRole("dialog", { name: "Create your first secret" })).toBeInTheDocument()
    expect(location.hash).toBe("#secrets")
  })

  it("copies the secret key and value independently", async () => {
    history.replaceState(null, "", "/#secrets")
    render(<Providers><App /></Providers>)

    await screen.findByText("deepseek-api-key")
    await userEvent.click(screen.getByRole("button", { name: "Copy key" }))
    expect(navigator.clipboard.writeText).toHaveBeenLastCalledWith("deepseek-api-key")
    await userEvent.click(screen.getByRole("button", { name: "Copy value" }))
    expect(navigator.clipboard.writeText).toHaveBeenLastCalledWith("secret-value")
  })

  it("guides a single-admin workspace to add users before groups", async () => {
    history.replaceState(null, "", "/#groups")
    render(<Providers><App /></Providers>)

    expect(await screen.findByRole("heading", { name: "Add users before groups" })).toBeInTheDocument()
    await userEvent.click(screen.getByRole("button", { name: "Add a user first" }))
    await waitFor(() => expect(location.hash).toBe("#users"))
  })

  it("provides an in-app help entry with scenario-based guidance", async () => {
    history.replaceState(null, "", "/#help")
    render(<Providers><App /></Providers>)

    expect(await screen.findByRole("heading", { name: "Start with the shortest path" })).toBeInTheDocument()
    expect(screen.getByRole("heading", { name: "Connect an app, CI job, BWS, or fnox" })).toBeInTheDocument()
    expect(screen.getByText(location.origin)).toBeInTheDocument()
    expect(screen.getByRole("button", { name: "Help" })).toBeInTheDocument()
  })
})
