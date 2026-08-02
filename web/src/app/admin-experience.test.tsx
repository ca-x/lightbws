import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { Providers } from "./Providers"
import type { MachineAccessToken, MachineAccount, Session, User } from "./types"

const mocks = vi.hoisted(() => ({
  createUser: vi.fn(),
  deleteMachine: vi.fn(),
  machineEvents: vi.fn(),
  machineTokens: vi.fn(),
  machines: vi.fn(),
  session: vi.fn(),
  users: vi.fn(),
}))

vi.mock("./api", () => ({
  ApiError: class ApiError extends Error {
    constructor(
      public status: number,
      public code: string,
      public detail?: string,
    ) {
      super(detail || code)
    }
  },
  api: mocks,
}))

import { ApiError } from "./api"
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
const machine: MachineAccount = {
  id: "machine-1",
  name: "Production deploy",
  clientId: "9d75a659-7297-4a98-adb4-d97dfda014ce",
  lastUsedAt: null,
  revokedAt: null,
  compatibilityAccount: false,
  createdAt: 1,
}
const machineToken: MachineAccessToken = {
  id: "token-1",
  machineAccountId: machine.id,
  name: "Production token",
  expiresAt: null,
  lastUsedAt: null,
  revokedAt: null,
  createdAt: 1,
}

describe("admin experience", () => {
  afterEach(cleanup)

  beforeEach(() => {
    vi.clearAllMocks()
    const stored = new Map<string, string>()
    Object.defineProperty(globalThis, "localStorage", {
      configurable: true,
      value: {
        clear: () => stored.clear(),
        getItem: (key: string) => stored.get(key) ?? null,
        removeItem: (key: string) => stored.delete(key),
        setItem: (key: string, value: string) => stored.set(key, value),
      },
    })
    localStorage.setItem("lightbws-locale", "en")
    localStorage.setItem("lightbws-onboarding-v1", "done")
    mocks.session.mockResolvedValue(session)
    mocks.users.mockResolvedValue([admin])
    mocks.machines.mockResolvedValue([machine])
    mocks.machineTokens.mockResolvedValue([machineToken])
    mocks.machineEvents.mockResolvedValue([])
    mocks.createUser.mockResolvedValue({
      ...admin,
      id: "user-2",
      username: "operator",
      displayName: "Operator",
      role: "user",
    })
    mocks.deleteMachine.mockResolvedValue(undefined)
  })

  it("validates a new user inline and sends only the accepted API fields", async () => {
    history.replaceState(null, "", "/#users")
    render(
      <Providers>
        <App />
      </Providers>,
    )

    await userEvent.click(await screen.findByRole("button", { name: "New user" }))
    const dialog = screen.getByRole("dialog", { name: "New user" })
    const username = within(dialog).getByRole("textbox", { name: /Username/i })
    const displayName = within(dialog).getByRole("textbox", {
      name: /Display name/i,
    })
    const password = within(dialog).getByLabelText(/^Password/i)

    await waitFor(() => expect(username).toHaveFocus())
    expect(within(dialog).queryByText("Enter a value for this field.")).not.toBeInTheDocument()
    await userEvent.click(username)
    await userEvent.tab()
    expect(within(dialog).getByText("Enter a value for this field.")).toBeInTheDocument()

    await userEvent.type(username, "  operator  ")
    await userEvent.type(displayName, "  Operator  ")
    await userEvent.type(password, "short")
    await userEvent.tab()
    expect(within(dialog).getByText("Use 6 to 4,096 characters; no line breaks or control characters.")).toBeInTheDocument()

    await userEvent.type(password, "-enough")
    await userEvent.click(within(dialog).getByRole("button", { name: "Save" }))

    await waitFor(() =>
      expect(mocks.createUser).toHaveBeenCalledWith({
        username: "operator",
        displayName: "Operator",
        role: "user",
        password: "short-enough",
      }),
    )
  })

  it("keeps the user form open with a recoverable conflict message", async () => {
    mocks.createUser.mockRejectedValue(new ApiError(409, "CONFLICT"))
    history.replaceState(null, "", "/#users")
    render(
      <Providers>
        <App />
      </Providers>,
    )

    await userEvent.click(await screen.findByRole("button", { name: "New user" }))
    const dialog = screen.getByRole("dialog", { name: "New user" })
    await userEvent.type(within(dialog).getByRole("textbox", { name: /Username/i }), "admin")
    await userEvent.type(within(dialog).getByRole("textbox", { name: /Display name/i }), "Duplicate")
    await userEvent.type(within(dialog).getByLabelText(/^Password/i), "password-123")
    await userEvent.click(within(dialog).getByRole("button", { name: "Save" }))

    expect(await within(dialog).findByText("This change conflicts with the current workspace state. Refresh and try again.")).toBeInTheDocument()
    expect(screen.getByRole("dialog", { name: "New user" })).toBeInTheDocument()
  })

  it("returns focus to the trigger after closing a modal with Escape", async () => {
    history.replaceState(null, "", "/#users")
    render(
      <Providers>
        <App />
      </Providers>,
    )

    const trigger = await screen.findByRole("button", { name: "New user" })
    await userEvent.click(trigger)
    expect(screen.getByRole("dialog", { name: "New user" })).toBeInTheDocument()

    await userEvent.keyboard("{Escape}")

    expect(screen.queryByRole("dialog", { name: "New user" })).not.toBeInTheDocument()
    expect(trigger).toHaveFocus()
  })

  it("keeps focus on the current field when a modal parent rerenders", async () => {
    history.replaceState(null, "", "/#users")
    render(
      <Providers>
        <App />
      </Providers>,
    )

    await userEvent.click(await screen.findByRole("button", { name: "New user" }))
    const dialog = screen.getByRole("dialog", { name: "New user" })
    await waitFor(() => expect(within(dialog).getByRole("textbox", { name: /Username/i })).toHaveFocus())
    const displayName = within(dialog).getByRole("textbox", {
      name: /Display name/i,
    })
    await userEvent.click(displayName)
    expect(displayName).toHaveFocus()

    fireEvent.click(screen.getByRole("button", { name: "Navigation" }))
    await act(() => new Promise<void>((resolve) => requestAnimationFrame(() => resolve())))

    expect(displayName).toHaveFocus()
  })

  it("returns focus to the mobile navigation trigger when the drawer closes", async () => {
    history.replaceState(null, "", "/#users")
    render(
      <Providers>
        <App />
      </Providers>,
    )

    const trigger = await screen.findByRole("button", { name: "Navigation" })
    await userEvent.click(trigger)
    await userEvent.click(screen.getAllByRole("button", { name: "Close navigation" })[0])

    expect(trigger).toHaveFocus()
  })

  it("exposes data tables as keyboard-scrollable regions", async () => {
    const clientWidth = vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(320)
    const scrollWidth = vi.spyOn(HTMLElement.prototype, "scrollWidth", "get").mockReturnValue(640)
    history.replaceState(null, "", "/#users")
    try {
      render(
        <Providers>
          <App />
        </Providers>,
      )

      expect(
        await screen.findByRole("region", {
          name: "Scroll sideways to see more columns and actions.",
        }),
      ).toHaveAttribute("tabindex", "0")
    } finally {
      clientWidth.mockRestore()
      scrollWidth.mockRestore()
    }
  })

  it("does not add a scroll instruction or tab stop when a table fits", async () => {
    history.replaceState(null, "", "/#users")
    render(
      <Providers>
        <App />
      </Providers>,
    )

    await screen.findByRole("table")
    expect(
      screen.queryByRole("region", {
        name: "Scroll sideways to see more columns and actions.",
      }),
    ).not.toBeInTheDocument()
    expect(screen.queryByText("Scroll sideways to see more columns and actions.")).not.toBeInTheDocument()
  })

  it("keeps the parent modal open when Escape closes a nested confirmation", async () => {
    history.replaceState(null, "", "/#machines")
    render(
      <Providers>
        <App />
      </Providers>,
    )

    await userEvent.click(await screen.findByRole("button", { name: "Credentials & events" }))
    const parent = await screen.findByRole("dialog", {
      name: /Credentials & events · Production deploy/,
    })
    await userEvent.click(await within(parent).findByRole("button", { name: "Revoke" }))
    const confirmation = screen.getByRole("alertdialog", {
      name: "Revoke access token?",
    })
    const cancel = within(confirmation).getByRole("button", { name: "Cancel" })
    cancel.focus()
    expect(cancel).toHaveFocus()

    await userEvent.keyboard("{Escape}")

    await waitFor(() => expect(screen.queryByRole("alertdialog", { name: "Revoke access token?" })).not.toBeInTheDocument())
    expect(parent).toBeInTheDocument()
    await waitFor(() => expect(within(parent).getByRole("button", { name: "Revoke" })).toHaveFocus())
  })

  it("requires explicit confirmation before deleting a machine account", async () => {
    history.replaceState(null, "", "/#machines")
    render(
      <Providers>
        <App />
      </Providers>,
    )

    await screen.findByText("Production deploy")
    await userEvent.click(screen.getByRole("button", { name: "Delete" }))
    expect(mocks.deleteMachine).not.toHaveBeenCalled()

    const confirmation = screen.getByRole("alertdialog", {
      name: "Delete machine account?",
    })
    expect(within(confirmation).getByText(/All credentials and sessions/)).toBeInTheDocument()
    await userEvent.click(within(confirmation).getByRole("button", { name: "Delete" }))

    await waitFor(() => expect(mocks.deleteMachine).toHaveBeenCalledWith("machine-1"))
  })
})
