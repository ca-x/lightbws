export type Role = "admin" | "user"
export type Locale = "zh-CN" | "en"
export type ThemeMode = "system" | "light" | "dark"

export interface User {
  id: string
  username: string
  displayName: string
  role: Role
  disabled: boolean
  createdAt: number
  updatedAt: number
  lastLoginAt: number | null
}

export interface Session { user: User; csrfToken: string }
export interface Overview { projects: number; secrets: number; trash: number }
export interface Permission { read: boolean; write: boolean }
export interface Project { id: string; name: string; sdkEncrypted: boolean; deletedAt: number | null; createdAt: number; updatedAt: number; permissions: Permission }
export interface Secret { id: string; projectId: string; key: string; value: string | null; note: string; sdkEncrypted: boolean; deletedAt: number | null; createdAt: number; updatedAt: number; permissions: Permission }
export interface MachineAccount { id: string; name: string; clientId: string; lastUsedAt: number | null; revokedAt: number | null; compatibilityAccount: boolean; createdAt: number }
export interface IssuedMachineAccount extends MachineAccount { accessToken: string }
export interface Group { id: string; name: string; memberIds: string[]; createdAt: number; updatedAt: number }
export interface NamedGrant { granteeId: string; name: string; read: boolean; write: boolean }
export interface GrantInput { granteeId: string; read: boolean; write: boolean }
export interface AccessPolicy { users: NamedGrant[]; groups: NamedGrant[]; machines: NamedGrant[] }
export interface AccessPolicyInput { users: GrantInput[]; groups: GrantInput[]; machines: GrantInput[] }
export interface MachineAccess { users: NamedGrant[]; groups: NamedGrant[]; projects: NamedGrant[] }
export interface MachineAccessInput { users: GrantInput[]; groups: GrantInput[]; projects: GrantInput[] }

export interface S3Config { endpoint: string; region: string; bucket: string; prefix: string; pathStyle: boolean }
export interface WebDavConfig { endpoint: string; prefix: string }
export type BackupConfig = { kind: "S3"; settings: S3Config } | { kind: "WEBDAV"; settings: WebDavConfig }
export interface BackupScopes { identities: boolean; machineAccounts: boolean; accessPolicies: boolean; audit: boolean; backupTargets: boolean }
export type BackupEncryption = "masterKey" | "plaintext"
export interface BackupCapabilities { plaintextAllowed: boolean }
export interface BackupTarget {
  id: string
  displayName: string
  config: BackupConfig
  enabled: boolean
  scheduleEnabled: boolean
  intervalHours: number
  nextRunAt: number | null
  lastRunAt: number | null
  lastStatus: string | null
  lastError: string | null
  hasCredentials: boolean
  scopes: BackupScopes
  encryption: BackupEncryption
  createdAt: number
  updatedAt: number
}
export interface BackupJob { id: string; targetId: string; triggerKind: string; status: string; objectKey: string; byteSize: number | null; errorCode: string | null; createdAt: number; completedAt: number | null }
export interface AuditEvent { id: string; actorKind: string; actorId: string | null; action: string; resourceKind: string; resourceId: string | null; outcome: string; createdAt: number }
export interface AuditSettings { enabled: boolean; autoCleanupEnabled: boolean; retentionDays: number; lastCleanupAt: number | null; updatedAt: number }
