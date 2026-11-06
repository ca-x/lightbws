import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { Providers } from "./Providers"
import type { IssuedMachineAccount, Project, Secret, Session, User } from "./types"

const mocks = vi.hoisted(() => ({
  createGroup: vi.fn(),
  createMachine: vi.fn(),
  createMachineToken: vi.fn(),
  createProject: vi.fn(),
  createSecret: vi.fn(),
  createUser: vi.fn(),
  groups: vi.fn(),
  machineAccess: vi.fn(),
  machines: vi.fn(),
  overview: vi.fn(),
  projects: vi.fn(),
  secrets: vi.fn(),
  session: vi.fn(),
  updateMachineAccess: vi.fn(),
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
      this.name = "ApiError"
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
const member: User = {
  ...admin,
  id: "user-2",
  username: "operator",
  displayName: "Operator",
  role: "user",
}
const session: Session = { csrfToken: "csrf", user: admin }
const project: Project = {
  id: "project-1",
  name: "Production",
  sdkEncrypted: true,
  deletedAt: null,
  permissions: { read: true, write: true },
  createdAt: 1,
  updatedAt: 1,
}
const readOnlyProject: Project = {
  ...project,
  id: "project-read-only",
  name: "Read-only staging",
  permissions: { read: true, write: false },
}
const createdProject: Project = {
  ...project,
  id: "project-created",
  name: "Release automation",
}
const draft = {
  key: "DEPLOY_TOKEN",
  value: "  release-secret-value  ",
  note: "CI deployment\nDo not discard this note",
}
const secret: Secret = {
  ...draft,
  id: "secret-1",
  projectId: null,
  sdkEncrypted: false,
  deletedAt: null,
  permissions: { read: true, write: true },
  createdAt: 1,
  updatedAt: 1,
}
const issuedMachine: IssuedMachineAccount = {
  id: "machine-created",
  name: "Release worker",
  clientId: "9d75a659-7297-4a98-adb4-d97dfda014ce",
  accessToken: "machine-creation-one-time-token",
  lastUsedAt: null,
  revokedAt: null,
  compatibilityAccount: false,
  createdAt: 1,
}
const resourceChoices = [
  { name: "New secret", field: "Key" },
  { name: "New project", field: "Project name" },
  { name: "New machine account", field: "Machine account name" },
  { name: "New user", field: "Username" },
  { name: "New group", field: "Group name" },
]

function createStorage(entries: Array<[string, string]> = []) {
  const stored = new Map(entries)
  return {
    clear: () => stored.clear(),
    getItem: (key: string) => stored.get(key) ?? null,
    removeItem: (key: string) => stored.delete(key),
    setItem: (key: string, value: string) => { stored.set(key, value) },
  }
}

function renderWorkspace(route = "#secrets") {
  history.replaceState(null, "", `/${route}`)
  return render(<Providers><App /></Providers>)
}

async function openCreation() {
  await userEvent.click(await screen.findByRole("button", { name: "Navigation" }))
  const navigation = screen.getByRole("dialog", { name: "Navigation" })
  await userEvent.click(within(navigation).getByRole("button", { name: "Create new" }))
  return screen.findByRole("dialog", { name: "Create new" })
}

async function chooseResource(name: string) {
  const chooser = await openCreation()
  await userEvent.click(within(chooser).getByRole("button", { name: new RegExp(`^${name}\\b`) }))
  return screen.findByRole("dialog", { name })
}

async function findSecretDialog() {
  await screen.findByRole("textbox", { name: /^Key/ })
  return screen.getByRole("dialog", { name: "New secret" })
}

async function fillSecretDraft(dialog: HTMLElement) {
  await userEvent.type(within(dialog).getByRole("textbox", { name: /^Key/ }), draft.key)
  await userEvent.type(within(dialog).getByRole("textbox", { name: "Value" }), draft.value)
  await userEvent.type(within(dialog).getByRole("textbox", { name: "Note" }), draft.note)
}

function expectSecretDraft(dialog: HTMLElement) {
  expect(within(dialog).getByRole("textbox", { name: /^Key/ })).toHaveValue(draft.key)
  expect(within(dialog).getByRole("textbox", { name: "Value" })).toHaveValue(draft.value)
  expect(within(dialog).getByRole("textbox", { name: "Note" })).toHaveValue(draft.note)
}

function expectNoAdministrativeCreation() {
  expect(mocks.createProject).not.toHaveBeenCalled()
  expect(mocks.createMachine).not.toHaveBeenCalled()
  expect(mocks.createUser).not.toHaveBeenCalled()
  expect(mocks.createGroup).not.toHaveBeenCalled()
}

describe("unified creation workflow", () => {
  let previousOverflow = ""

  beforeEach(() => {
    vi.resetAllMocks()
    previousOverflow = document.body.style.overflow
    vi.stubGlobal("localStorage", createStorage([
      ["lightbws-locale", "en"],
      ["lightbws-onboarding-v1", "done"],
    ]))
    vi.stubGlobal("sessionStorage", createStorage())
    Object.assign(navigator, { clipboard: { writeText: vi.fn().mockResolvedValue(undefined) } })
    mocks.session.mockResolvedValue(session)
    mocks.overview.mockResolvedValue({ projects: 1, secrets: 0, trash: 0 })
    mocks.projects.mockResolvedValue([project])
    mocks.secrets.mockResolvedValue([])
    mocks.users.mockResolvedValue([admin, member])
    mocks.groups.mockResolvedValue([])
    mocks.machines.mockResolvedValue([])
    mocks.createProject.mockResolvedValue(createdProject)
    mocks.createSecret.mockResolvedValue(secret)
    mocks.createMachine.mockResolvedValue(issuedMachine)
    mocks.machineAccess.mockResolvedValue({ users: [], groups: [], projects: [] })
    mocks.updateMachineAccess.mockResolvedValue({ users: [], groups: [], projects: [] })
  })

  afterEach(() => {
    cleanup()
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
    document.body.style.overflow = previousOverflow
    history.replaceState(null, "", "/")
  })

  it.each(resourceChoices)("selects $name from one navigation creation entry", async ({ name, field }) => {
    renderWorkspace()

    const chooser = await openCreation()
    expect(screen.getAllByRole("dialog")).toHaveLength(1)
    for (const choice of resourceChoices) {
      expect(within(chooser).getByRole("button", { name: new RegExp(`^${choice.name}\\b`) })).toBeInTheDocument()
    }

    await userEvent.click(within(chooser).getByRole("button", { name: new RegExp(`^${name}\\b`) }))

    await screen.findByRole("textbox", { name: new RegExp(`^${field}\\b`) })
    expect(screen.getByRole("dialog", { name })).toBeInTheDocument()
    expect(screen.getAllByRole("dialog")).toHaveLength(1)
    expect(screen.queryByRole("dialog", { name: "Create new" })).not.toBeInTheDocument()
    expect(location.hash).toBe("#secrets")
    expect(mocks.createSecret).not.toHaveBeenCalled()
    expectNoAdministrativeCreation()
  })

  it("opens the dashboard's first secret action without navigating away", async () => {
    renderWorkspace("#dashboard")

    await userEvent.click(await screen.findByRole("button", { name: "Create first secret" }))

    const dialog = await findSecretDialog()
    expect(within(dialog).getByRole("combobox", { name: "Project" })).toHaveValue("")
    expect(screen.getAllByRole("dialog")).toHaveLength(1)
    expect(location.hash).toBe("#dashboard")
    expect(mocks.createSecret).not.toHaveBeenCalled()
  })

  it("opens a new user from the groups prerequisite without navigating away", async () => {
    mocks.users.mockResolvedValue([admin])
    renderWorkspace("#groups")

    await userEvent.click(await screen.findByRole("button", { name: "Add a user first" }))

    const dialog = await screen.findByRole("dialog", { name: "New user" })
    expect(within(dialog).getByRole("textbox", { name: /^Username/ })).toBeInTheDocument()
    expect(screen.getAllByRole("dialog")).toHaveLength(1)
    expect(location.hash).toBe("#groups")
    expect(mocks.createUser).not.toHaveBeenCalled()
  })

  it("preserves the projects filter and trigger focus after a local creation refresh", async () => {
    mocks.projects.mockResolvedValueOnce([project]).mockResolvedValue([project, createdProject])
    renderWorkspace("#projects")

    const search = await screen.findByRole("textbox", { name: "Search" })
    await userEvent.type(search, "Production")
    const trigger = within(screen.getByRole("main")).getByRole("button", { name: "New project" })
    const visibleRects = [new DOMRect(0, 0, 160, 40)]
    vi.spyOn(trigger, "getClientRects").mockReturnValue(Object.assign(visibleRects, {
      item: (index: number) => visibleRects[index] ?? null,
    }))
    await userEvent.click(trigger)
    const dialog = await screen.findByRole("dialog", { name: "New project" })
    await userEvent.type(within(dialog).getByRole("textbox", { name: /^Project name/ }), createdProject.name)
    await userEvent.click(within(dialog).getByRole("button", { name: "Save" }))

    const completed = await screen.findByRole("dialog", { name: "Created successfully" })
    await waitFor(() => expect(mocks.projects).toHaveBeenCalledTimes(2))
    await userEvent.click(within(completed).getByRole("button", { name: "Done" }))

    expect(screen.getByRole("textbox", { name: "Search" })).toBe(search)
    expect(search).toHaveValue("Production")
    expect(within(screen.getByRole("main")).getByRole("button", { name: "New project" })).toBe(trigger)
    expect(within(screen.getByRole("table")).getByRole("button", { name: project.name })).toBeInTheDocument()
    expect(within(screen.getByRole("table")).queryByRole("button", { name: createdProject.name })).not.toBeInTheDocument()
    await waitFor(() => expect(trigger).toHaveFocus())
    expect(location.hash).toBe("#projects")
    expect(mocks.createProject).toHaveBeenCalledTimes(1)
  })

  it("keeps a created project selected and retains the secret draft when saving fails", async () => {
    mocks.createSecret.mockRejectedValueOnce(new ApiError(500, "INTERNAL_ERROR"))
    renderWorkspace()

    const projectDialog = await chooseResource("New project")
    await userEvent.type(within(projectDialog).getByRole("textbox", { name: /^Project name/ }), createdProject.name)
    await userEvent.click(within(projectDialog).getByRole("button", { name: "Save" }))

    const completed = await screen.findByRole("dialog", { name: "Created successfully" })
    expect(within(completed).getByText(createdProject.name)).toBeInTheDocument()
    expect(mocks.createProject).toHaveBeenCalledTimes(1)
    expect(mocks.createProject).toHaveBeenCalledWith(createdProject.name)
    expect(mocks.createSecret).not.toHaveBeenCalled()
    await userEvent.click(within(completed).getByRole("button", { name: "Add a secret" }))

    const dialog = await findSecretDialog()
    const projectPicker = within(dialog).getByRole("combobox", { name: "Project" })
    expect(projectPicker).toHaveValue(createdProject.id)
    expect(within(projectPicker).getByRole("option", { name: createdProject.name })).toBeInTheDocument()
    await fillSecretDraft(dialog)
    await userEvent.click(within(dialog).getByRole("button", { name: "Save" }))

    expect(await within(dialog).findByText("The request could not be completed.")).toBeInTheDocument()
    expect(screen.getByRole("dialog", { name: "New secret" })).toBe(dialog)
    expectSecretDraft(dialog)
    expect(projectPicker).toHaveValue(createdProject.id)
    expect(mocks.createProject).toHaveBeenCalledTimes(1)
    expect(mocks.createSecret).toHaveBeenCalledTimes(1)
    expect(mocks.createSecret).toHaveBeenNthCalledWith(1, { ...draft, projectId: createdProject.id })

    await userEvent.click(within(dialog).getByRole("button", { name: "Save" }))

    const saved = await screen.findByRole("dialog", { name: "Created successfully" })
    expect(within(saved).getByText(draft.key)).toBeInTheDocument()
    expect(mocks.createProject).toHaveBeenCalledTimes(1)
    expect(mocks.createSecret).toHaveBeenCalledTimes(2)
    expect(mocks.createSecret).toHaveBeenNthCalledWith(2, { ...draft, projectId: createdProject.id })
    await userEvent.click(within(saved).getByRole("button", { name: "Done" }))
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument()
  })

  it("creates and selects an inline project without losing or submitting the secret draft", async () => {
    renderWorkspace()
    await chooseResource("New secret")
    const dialog = await findSecretDialog()
    await fillSecretDraft(dialog)
    await userEvent.click(within(dialog).getByRole("button", { name: "New project" }))

    const projectName = within(dialog).getByRole("textbox", { name: /^Project name/ })
    await waitFor(() => expect(projectName).toHaveFocus())
    expectSecretDraft(dialog)
    expect(within(dialog).getByRole("button", { name: "Save" })).toBeDisabled()
    await userEvent.type(projectName, createdProject.name)
    await userEvent.click(within(dialog).getByRole("button", { name: "Create and select" }))

    const projectPicker = within(dialog).getByRole("combobox", { name: "Project" })
    await waitFor(() => expect(projectPicker).toHaveValue(createdProject.id))
    await waitFor(() => expect(projectPicker).toHaveFocus())
    expect(screen.getByRole("dialog", { name: "New secret" })).toBe(dialog)
    expect(screen.getAllByRole("dialog")).toHaveLength(1)
    expectSecretDraft(dialog)
    expect(within(dialog).queryByRole("textbox", { name: /^Project name/ })).not.toBeInTheDocument()
    expect(mocks.createProject).toHaveBeenCalledTimes(1)
    expect(mocks.createProject).toHaveBeenCalledWith(createdProject.name)
    expect(mocks.createSecret).not.toHaveBeenCalled()

    await userEvent.click(within(dialog).getByRole("button", { name: "Save" }))

    expect(await screen.findByRole("dialog", { name: "Created successfully" })).toBeInTheDocument()
    expect(mocks.createSecret).toHaveBeenCalledTimes(1)
    expect(mocks.createSecret).toHaveBeenCalledWith({ ...draft, projectId: createdProject.id })
    expect(mocks.createProject).toHaveBeenCalledTimes(1)
  })

  it("returns focus to New project when inline project creation is cancelled", async () => {
    renderWorkspace()
    await chooseResource("New secret")
    const dialog = await findSecretDialog()
    await fillSecretDraft(dialog)
    await userEvent.click(within(dialog).getByRole("button", { name: "New project" }))

    const projectName = within(dialog).getByRole("textbox", { name: /^Project name/ })
    await waitFor(() => expect(projectName).toHaveFocus())
    await userEvent.type(projectName, "Unsaved project")
    await userEvent.tab()
    expect(document.activeElement).toHaveAccessibleName("Cancel")
    await userEvent.keyboard("{Enter}")

    await waitFor(() => expect(within(dialog).getByRole("button", { name: "New project" })).toHaveFocus())
    expect(within(dialog).queryByRole("textbox", { name: /^Project name/ })).not.toBeInTheDocument()
    expect(within(dialog).getByRole("combobox", { name: "Project" })).toHaveValue("")
    expect(within(dialog).getByRole("button", { name: "Save" })).toBeEnabled()
    expectSecretDraft(dialog)
    expect(mocks.createProject).not.toHaveBeenCalled()
    expect(mocks.createSecret).not.toHaveBeenCalled()
  })

  it("recovers from an inline project conflict without creating a secret", async () => {
    mocks.createProject.mockRejectedValueOnce(new ApiError(409, "CONFLICT"))
    renderWorkspace()
    await chooseResource("New secret")
    const dialog = await findSecretDialog()
    await fillSecretDraft(dialog)
    await userEvent.click(within(dialog).getByRole("button", { name: "New project" }))
    const projectName = within(dialog).getByRole("textbox", { name: /^Project name/ })
    await userEvent.type(projectName, "Existing project")
    await userEvent.click(within(dialog).getByRole("button", { name: "Create and select" }))

    expect(await within(dialog).findByText("This change conflicts with the current workspace state. Refresh and try again.")).toBeInTheDocument()
    expect(projectName).toHaveValue("Existing project")
    expect(projectName).toHaveAttribute("aria-invalid", "true")
    expect(within(dialog).getByRole("combobox", { name: "Project" })).toHaveValue("")
    expect(within(dialog).getByRole("button", { name: "Create and select" })).toBeEnabled()
    expect(within(dialog).getByRole("button", { name: "Save" })).toBeDisabled()
    expectSecretDraft(dialog)
    expect(mocks.createSecret).not.toHaveBeenCalled()

    await userEvent.clear(projectName)
    await userEvent.type(projectName, createdProject.name)
    await userEvent.click(within(dialog).getByRole("button", { name: "Create and select" }))

    await waitFor(() => expect(within(dialog).getByRole("combobox", { name: "Project" })).toHaveValue(createdProject.id))
    expect(within(dialog).queryByText("This change conflicts with the current workspace state. Refresh and try again.")).not.toBeInTheDocument()
    expect(within(dialog).getByRole("button", { name: "Save" })).toBeEnabled()
    expectSecretDraft(dialog)
    expect(mocks.createProject).toHaveBeenCalledTimes(2)
    expect(mocks.createProject).toHaveBeenNthCalledWith(1, "Existing project")
    expect(mocks.createProject).toHaveBeenNthCalledWith(2, createdProject.name)
    expect(mocks.createSecret).not.toHaveBeenCalled()
  })

  it("limits a project deep link to its secrets and clears the scope with All secrets", async () => {
    mocks.projects.mockResolvedValue([project, readOnlyProject])
    mocks.secrets.mockResolvedValue([
      { ...secret, id: "secret-production", projectId: project.id, key: "PRODUCTION_KEY" },
      { ...secret, id: "secret-staging", projectId: readOnlyProject.id, key: "STAGING_KEY" },
      { ...secret, id: "secret-unassigned", projectId: null, key: "UNASSIGNED_KEY" },
    ])
    renderWorkspace(`#secrets?project=${project.id}`)

    expect(await screen.findByText("PRODUCTION_KEY")).toBeInTheDocument()
    const table = screen.getByRole("table")
    expect(within(table).getAllByRole("row")).toHaveLength(2)
    expect(within(table).queryByText("STAGING_KEY")).not.toBeInTheDocument()
    expect(within(table).queryByText("UNASSIGNED_KEY")).not.toBeInTheDocument()
    expect(screen.getByRole("heading", { level: 1, name: project.name })).toBeInTheDocument()
    expect(screen.getByRole("combobox", { name: "Project" })).toHaveValue(project.id)

    await userEvent.click(screen.getByRole("button", { name: "All secrets" }))

    await waitFor(() => expect(location.hash).toBe("#secrets"))
    const allSecrets = screen.getByRole("table")
    expect(within(allSecrets).getByText("PRODUCTION_KEY")).toBeInTheDocument()
    expect(within(allSecrets).getByText("STAGING_KEY")).toBeInTheDocument()
    expect(within(allSecrets).getByText("UNASSIGNED_KEY")).toBeInTheDocument()
    expect(screen.getByRole("combobox", { name: "Project" })).toHaveValue("")
    expect(screen.getByRole("combobox", { name: "Project" })).toHaveDisplayValue("All projects")
    expect(screen.getByRole("heading", { level: 1, name: "Secrets" })).toBeInTheDocument()
    expect(screen.queryByRole("button", { name: "All secrets" })).not.toBeInTheDocument()
  })

  it("offers members only secret creation and saves only to a writable project", async () => {
    mocks.session.mockResolvedValue({ ...session, user: member })
    mocks.projects.mockResolvedValue([readOnlyProject, project])
    renderWorkspace()

    const chooser = await openCreation()
    expect(within(chooser).getByRole("button", { name: /^New secret\b/ })).toBeInTheDocument()
    for (const choice of resourceChoices.filter((choice) => choice.name !== "New secret")) {
      expect(within(chooser).queryByRole("button", { name: new RegExp(`^${choice.name}\\b`) })).not.toBeInTheDocument()
    }
    await userEvent.click(within(chooser).getByRole("button", { name: /^New secret\b/ }))

    const dialog = await findSecretDialog()
    const projectPicker = within(dialog).getByRole("combobox", { name: "Project" })
    expect(projectPicker).toHaveValue(project.id)
    expect(within(projectPicker).getAllByRole("option")).toHaveLength(1)
    expect(within(projectPicker).getByRole("option", { name: project.name })).toBeInTheDocument()
    expect(within(projectPicker).queryByRole("option", { name: readOnlyProject.name })).not.toBeInTheDocument()
    expect(within(projectPicker).queryByRole("option", { name: "No project" })).not.toBeInTheDocument()
    expect(within(dialog).queryByRole("button", { name: "New project" })).not.toBeInTheDocument()
    await fillSecretDraft(dialog)
    await userEvent.click(within(dialog).getByRole("button", { name: "Save" }))

    expect(await screen.findByRole("dialog", { name: "Created successfully" })).toBeInTheDocument()
    expect(mocks.createSecret).toHaveBeenCalledTimes(1)
    expect(mocks.createSecret).toHaveBeenCalledWith({ ...draft, projectId: project.id })
    expectNoAdministrativeCreation()
  })

  it.each([
    { label: "no projects", projects: [] },
    { label: "read-only projects", projects: [readOnlyProject] },
  ])("prevents a member with $label from saving a secret", async ({ projects }) => {
    mocks.session.mockResolvedValue({ ...session, user: member })
    mocks.projects.mockResolvedValue(projects)
    renderWorkspace()

    const chooser = await openCreation()
    await userEvent.click(within(chooser).getByRole("button", { name: /^New secret\b/ }))

    expect(await screen.findByText("You need write access to a project before creating a secret. Ask an administrator to grant access.")).toBeInTheDocument()
    const dialog = screen.getByRole("dialog", { name: "New secret" })
    expect(within(dialog).queryByRole("textbox", { name: /^Key/ })).not.toBeInTheDocument()
    expect(within(dialog).queryByRole("button", { name: "Save" })).not.toBeInTheDocument()
    expect(within(dialog).queryByRole("button", { name: "New project" })).not.toBeInTheDocument()
    expect(mocks.createSecret).not.toHaveBeenCalled()
    expectNoAdministrativeCreation()
  })

  it("keeps project creation and read-only project actions unavailable to members", async () => {
    mocks.session.mockResolvedValue({ ...session, user: member })
    mocks.projects.mockResolvedValue([project, readOnlyProject])
    renderWorkspace("#projects")

    await screen.findByRole("button", { name: readOnlyProject.name })
    const main = screen.getByRole("main")
    expect(within(main).queryByRole("button", { name: "New project" })).not.toBeInTheDocument()
    const readOnlyRow = screen.getByRole("row", { name: new RegExp(readOnlyProject.name) })
    expect(within(readOnlyRow).queryByRole("button", { name: "Add a secret" })).not.toBeInTheDocument()
    expect(within(readOnlyRow).queryByRole("button", { name: "Manage access" })).not.toBeInTheDocument()
    expect(within(readOnlyRow).queryByRole("button", { name: "Edit" })).not.toBeInTheDocument()
    expect(within(readOnlyRow).queryByRole("button", { name: "Delete" })).not.toBeInTheDocument()
    const writableRow = screen.getByRole("row", { name: new RegExp(project.name) })
    await userEvent.click(within(writableRow).getByRole("button", { name: "Add a secret" }))

    const dialog = await findSecretDialog()
    expect(within(dialog).getByRole("combobox", { name: "Project" })).toHaveValue(project.id)
    expect(location.hash).toBe("#projects")
    expect(mocks.createSecret).not.toHaveBeenCalled()
    expectNoAdministrativeCreation()
  })

  it("cycles mobile navigation focus and restores the trigger and scroll style on Escape", async () => {
    document.body.style.overflow = "scroll"
    renderWorkspace()
    const trigger = await screen.findByRole("button", { name: "Navigation" })
    expect(trigger).toHaveAttribute("aria-expanded", "false")
    await userEvent.click(trigger)

    const navigation = screen.getByRole("dialog", { name: "Navigation" })
    const first = within(navigation).getByRole("button", { name: "Close navigation" })
    const last = within(navigation).getByRole("button", { name: "Sign out" })
    await waitFor(() => expect(first).toHaveFocus())
    expect(trigger).toHaveAttribute("aria-expanded", "true")
    expect(document.body.style.overflow).toBe("hidden")
    await userEvent.tab({ shift: true })
    expect(last).toHaveFocus()
    await userEvent.tab()
    expect(first).toHaveFocus()

    await userEvent.keyboard("{Escape}")

    expect(screen.queryByRole("dialog", { name: "Navigation" })).not.toBeInTheDocument()
    expect(trigger).toHaveAttribute("aria-expanded", "false")
    expect(trigger).toHaveFocus()
    expect(document.body.style.overflow).toBe("scroll")
  })

  it("hands mobile navigation to creation and restores scrolling when creation closes", async () => {
    document.body.style.overflow = "scroll"
    renderWorkspace()

    const chooser = await openCreation()
    expect(screen.queryByRole("dialog", { name: "Navigation" })).not.toBeInTheDocument()
    expect(screen.getAllByRole("dialog")).toHaveLength(1)
    expect(document.body.style.overflow).toBe("hidden")
    await waitFor(() => expect(within(chooser).getByRole("button", { name: "Close" })).toHaveFocus())
    await userEvent.keyboard("{Escape}")

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument()
    await waitFor(() => expect(screen.getByRole("button", { name: "Navigation" })).toHaveFocus())
    expect(document.body.style.overflow).toBe("scroll")
  })

  it("restores the previous scroll style when an open mobile drawer unmounts", async () => {
    document.body.style.overflow = "auto"
    const workspace = renderWorkspace()
    await userEvent.click(await screen.findByRole("button", { name: "Navigation" }))
    expect(document.body.style.overflow).toBe("hidden")

    workspace.unmount()

    expect(document.body.style.overflow).toBe("auto")
  })

  it.each(["Escape", "Close", "backdrop"])("keeps pending machine creation open after %s until its one-time token arrives", async (dismissal) => {
    let resolveMachine!: (machine: IssuedMachineAccount) => void
    const response = new Promise<IssuedMachineAccount>((resolve) => { resolveMachine = resolve })
    mocks.createMachine.mockReturnValueOnce(response)
    renderWorkspace()

    const dialog = await chooseResource("New machine account")
    const name = within(dialog).getByRole("textbox", { name: /^Machine account name/ })
    await userEvent.type(name, issuedMachine.name)
    await userEvent.click(within(dialog).getByRole("button", { name: "Save" }))

    await waitFor(() => expect(mocks.createMachine).toHaveBeenCalledTimes(1))
    expect(dialog).toHaveAttribute("aria-busy", "true")
    const close = within(dialog).getByRole("button", { name: "Close" })
    expect(close).toHaveAttribute("aria-disabled", "true")
    expect(within(dialog).getByRole("button", { name: "Cancel" })).toBeDisabled()
    expect(screen.queryByRole("dialog", { name: "Access token" })).not.toBeInTheDocument()

    if (dismissal === "Escape") {
      await userEvent.keyboard("{Escape}")
    } else if (dismissal === "Close") {
      await userEvent.click(close)
    } else {
      const backdrop = dialog.closest<HTMLElement>('[role="presentation"]')
      if (!backdrop) throw new Error("The creation modal backdrop is missing.")
      await userEvent.click(backdrop)
    }

    expect(screen.getByRole("dialog", { name: "New machine account" })).toBe(dialog)
    expect(screen.getAllByRole("dialog")).toHaveLength(1)
    expect(name).toHaveValue(issuedMachine.name)
    expect(document.body.style.overflow).toBe("hidden")
    expect(mocks.createMachine).toHaveBeenCalledTimes(1)

    await act(async () => {
      resolveMachine(issuedMachine)
      await response
    })

    const token = await screen.findByRole("dialog", { name: "Access token" })
    expect(within(token).getByText(issuedMachine.accessToken)).toBeInTheDocument()
    expect(screen.queryByRole("dialog", { name: "New machine account" })).not.toBeInTheDocument()
    await userEvent.click(within(token).getByRole("button", { name: "Copy" }))
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith(issuedMachine.accessToken)
    expect(mocks.createMachine).toHaveBeenCalledTimes(1)
    expect(mocks.createMachine).toHaveBeenCalledWith(issuedMachine.name)
    expect(mocks.createMachineToken).not.toHaveBeenCalled()
  })

  it("releases the machine creation close guard after a failed response", async () => {
    let rejectMachine!: (error: Error) => void
    mocks.createMachine.mockReturnValueOnce(new Promise<IssuedMachineAccount>((_resolve, reject) => {
      rejectMachine = reject
    }))
    renderWorkspace()

    const dialog = await chooseResource("New machine account")
    const name = within(dialog).getByRole("textbox", { name: /^Machine account name/ })
    await userEvent.type(name, issuedMachine.name)
    await userEvent.click(within(dialog).getByRole("button", { name: "Save" }))
    expect(dialog).toHaveAttribute("aria-busy", "true")
    expect(within(dialog).getByRole("button", { name: "Close" })).toHaveAttribute("aria-disabled", "true")

    await act(async () => { rejectMachine(new ApiError(500, "INTERNAL_ERROR")) })

    expect(await within(dialog).findByText("The request could not be completed.")).toBeInTheDocument()
    expect(dialog).not.toHaveAttribute("aria-busy", "true")
    expect(within(dialog).getByRole("button", { name: "Close" })).not.toHaveAttribute("aria-disabled", "true")
    expect(within(dialog).getByRole("button", { name: "Cancel" })).toBeEnabled()
    expect(within(dialog).getByRole("button", { name: "Save" })).toBeEnabled()
    expect(name).toHaveValue(issuedMachine.name)
    expect(mocks.createMachine).toHaveBeenCalledTimes(1)
    expect(screen.queryByRole("dialog", { name: "Access token" })).not.toBeInTheDocument()

    await userEvent.keyboard("{Escape}")

    expect(screen.queryByRole("dialog", { name: "New machine account" })).not.toBeInTheDocument()
    expect(mocks.createMachine).toHaveBeenCalledTimes(1)
  })

  it.each(["Cancel", "Save"])("keeps the one-time machine token copyable after access %s without storage writes", async (action) => {
    renderWorkspace()
    await screen.findByRole("button", { name: "Navigation" })
    const localWrites = vi.spyOn(localStorage, "setItem")
    const sessionWrites = vi.spyOn(sessionStorage, "setItem")
    const dialog = await chooseResource("New machine account")
    await userEvent.type(within(dialog).getByRole("textbox", { name: /^Machine account name/ }), issuedMachine.name)
    await userEvent.click(within(dialog).getByRole("button", { name: "Save" }))

    const token = await screen.findByRole("dialog", { name: "Access token" })
    expect(within(token).getByText("This token is shown once. Copy it before closing.")).toBeInTheDocument()
    expect(within(token).getByText(issuedMachine.accessToken)).toBeInTheDocument()
    await userEvent.click(within(token).getByRole("button", { name: "Copy" }))
    expect(navigator.clipboard.writeText).toHaveBeenNthCalledWith(1, issuedMachine.accessToken)
    await userEvent.click(within(token).getByRole("button", { name: "Configure access" }))

    const access = await screen.findByRole("dialog", { name: `Manage access · ${issuedMachine.name}` })
    const permission = await within(access).findByRole("combobox", { name: `${project.name} Permission` })
    expect(mocks.machineAccess).toHaveBeenCalledWith(issuedMachine.id)
    await userEvent.selectOptions(permission, "write")
    await userEvent.click(within(access).getByRole("button", { name: action }))

    const returnedToken = await screen.findByRole("dialog", { name: "Access token" })
    expect(within(returnedToken).getByText(issuedMachine.accessToken)).toBeInTheDocument()
    await userEvent.click(within(returnedToken).getByRole("button", { name: "Copy" }))
    expect(navigator.clipboard.writeText).toHaveBeenCalledTimes(2)
    expect(navigator.clipboard.writeText).toHaveBeenNthCalledWith(2, issuedMachine.accessToken)
    expect(mocks.createMachine).toHaveBeenCalledTimes(1)
    expect(mocks.createMachine).toHaveBeenCalledWith(issuedMachine.name)
    expect(mocks.createMachineToken).not.toHaveBeenCalled()
    if (action === "Save") {
      expect(mocks.updateMachineAccess).toHaveBeenCalledTimes(1)
      expect(mocks.updateMachineAccess).toHaveBeenCalledWith(issuedMachine.id, {
        users: [],
        groups: [],
        projects: [{ granteeId: project.id, read: true, write: true }],
      })
    } else {
      expect(mocks.updateMachineAccess).not.toHaveBeenCalled()
    }

    await userEvent.keyboard("{Escape}")

    expect(screen.queryByRole("dialog", { name: "Access token" })).not.toBeInTheDocument()
    expect(screen.queryByText(issuedMachine.accessToken)).not.toBeInTheDocument()
    expect(localWrites).not.toHaveBeenCalled()
    expect(sessionWrites).not.toHaveBeenCalled()
  })
})
