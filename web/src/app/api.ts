import type { AccessPolicy, AccessPolicyInput, AuditEvent, AuditSettings, BackupJob, BackupTarget, Group, IssuedMachineAccount, MachineAccess, MachineAccessInput, MachineAccount, Overview, Project, Role, Secret, Session, User } from "./types"

export class ApiError extends Error {
  constructor(public status: number, public code: string) { super(code) }
}

let csrfToken = ""

async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const method = init.method || "GET"
  const headers = new Headers(init.headers)
  if (init.body && !(init.body instanceof FormData)) headers.set("content-type", "application/json")
  if (!["GET", "HEAD"].includes(method)) headers.set("x-csrf-token", csrfToken)
  const response = await fetch(`/api/v1${path}`, { ...init, headers, credentials: "same-origin" })
  if (!response.ok) {
    const body = await response.json().catch(() => null) as { error?: { code?: string } } | null
    throw new ApiError(response.status, body?.error?.code || "UNKNOWN")
  }
  if (response.status === 204) return undefined as T
  return response.json() as Promise<T>
}

function json(method: string, body: unknown): RequestInit {
  return { method, body: JSON.stringify(body) }
}

export const api = {
  async session() { const value = await request<Session>("/auth/session"); csrfToken = value.csrfToken; return value },
  async login(username: string, password: string) { const value = await request<Session>("/auth/login", json("POST", { username, password })); csrfToken = value.csrfToken; return value },
  async logout() { await request<void>("/auth/logout", { method: "POST" }); csrfToken = "" },
  overview: () => request<Overview>("/overview"),
  projects: (trash = false) => request<Project[]>(`/projects?trash=${trash}`),
  createProject: (name: string) => request<Project>("/projects", json("POST", { name })),
  updateProject: (id: string, name: string) => request<Project>(`/projects/${id}`, json("PUT", { name })),
  trashProject: (id: string) => request<void>(`/projects/${id}`, { method: "DELETE" }),
  restoreProject: (id: string) => request<void>(`/projects/${id}/restore`, { method: "PUT" }),
  purgeProject: (id: string) => request<void>(`/projects/${id}/purge`, { method: "DELETE" }),
  secrets: (trash = false, projectId?: string) => request<Secret[]>(`/secrets?trash=${trash}${projectId ? `&projectId=${projectId}` : ""}`),
  createSecret: (input: { key: string; value: string; note: string; projectId: string }) => request<Secret>("/secrets", json("POST", input)),
  updateSecret: (id: string, input: { key: string; value: string; note: string; projectId: string }) => request<Secret>(`/secrets/${id}`, json("PUT", input)),
  trashSecret: (id: string) => request<void>(`/secrets/${id}`, { method: "DELETE" }),
  restoreSecret: (id: string) => request<void>(`/secrets/${id}/restore`, { method: "PUT" }),
  purgeSecret: (id: string) => request<void>(`/secrets/${id}/purge`, { method: "DELETE" }),
  users: () => request<User[]>("/admin/users"),
  createUser: (input: { username: string; displayName: string; role: Role; password: string }) => request<User>("/admin/users", json("POST", input)),
  updateUser: (id: string, input: { displayName: string; role: Role; disabled: boolean }) => request<User>(`/admin/users/${id}`, json("PUT", input)),
  resetPassword: (id: string, password: string) => request<void>(`/admin/users/${id}/password`, json("PUT", { password })),
  groups: () => request<Group[]>("/admin/groups"),
  createGroup: (name: string) => request<Group>("/admin/groups", json("POST", { name })),
  updateGroup: (id: string, name: string) => request<Group>(`/admin/groups/${id}`, json("PUT", { name })),
  deleteGroup: (id: string) => request<void>(`/admin/groups/${id}`, { method: "DELETE" }),
  replaceGroupMembers: (id: string, memberIds: string[]) => request<Group>(`/admin/groups/${id}/members`, json("PUT", { memberIds })),
  machines: () => request<MachineAccount[]>("/admin/machines"),
  createMachine: (name: string) => request<IssuedMachineAccount>("/admin/machines", json("POST", { name })),
  setMachineRevoked: (id: string, revoked: boolean) => request<MachineAccount>(`/admin/machines/${id}/${revoked ? "revoke" : "restore"}`, { method: "PUT" }),
  deleteMachine: (id: string) => request<void>(`/admin/machines/${id}`, { method: "DELETE" }),
  projectAccess: (id: string) => request<AccessPolicy>(`/projects/${id}/access`),
  updateProjectAccess: (id: string, input: AccessPolicyInput) => request<AccessPolicy>(`/projects/${id}/access`, json("PUT", input)),
  secretAccess: (id: string) => request<AccessPolicy>(`/secrets/${id}/access`),
  updateSecretAccess: (id: string, input: AccessPolicyInput) => request<AccessPolicy>(`/secrets/${id}/access`, json("PUT", input)),
  machineAccess: (id: string) => request<MachineAccess>(`/machines/${id}/access`),
  updateMachineAccess: (id: string, input: MachineAccessInput) => request<MachineAccess>(`/machines/${id}/access`, json("PUT", input)),
  backupTargets: () => request<BackupTarget[]>("/backups/targets"),
  createBackupTarget: (input: unknown) => request<BackupTarget>("/backups/targets", json("POST", input)),
  updateBackupTarget: (id: string, input: unknown) => request<BackupTarget>(`/backups/targets/${id}`, json("PUT", input)),
  deleteBackupTarget: (id: string) => request<void>(`/backups/targets/${id}`, { method: "DELETE" }),
  testBackupTarget: (id: string) => request<void>(`/backups/targets/${id}/test`, { method: "POST" }),
  runBackup: (id: string) => request<BackupJob>(`/backups/targets/${id}/run`, { method: "POST" }),
  backupJobs: () => request<BackupJob[]>("/backups/jobs"),
  auditEvents: () => request<AuditEvent[]>("/audit"),
  auditSettings: () => request<AuditSettings>("/audit/settings"),
  updateAuditSettings: (input: { enabled: boolean; autoCleanupEnabled: boolean; retentionDays: number }) => request<AuditSettings>("/audit/settings", json("PUT", input)),
  clearAudit: () => request<{ deleted: number }>("/audit", { method: "DELETE" }),
  async export(passphrase: string) {
    const response = await fetch("/api/v1/transfer/export", { ...json("POST", { passphrase }), headers: { "content-type": "application/json", "x-csrf-token": csrfToken } })
    if (!response.ok) throw new ApiError(response.status, "EXPORT_FAILED")
    return response.blob()
  },
  import: (passphrase: string, dataBase64: string) => request<{ imported: { projects: number; secrets: number } }>("/transfer/import", json("POST", { passphrase, dataBase64 })),
}
