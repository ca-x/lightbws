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
export interface Project { id: string; name: string; sdkEncrypted: boolean; deletedAt: number | null; createdAt: number; updatedAt: number }
export interface Secret { id: string; projectId: string | null; key: string; value: string | null; note: string; sdkEncrypted: boolean; deletedAt: number | null; createdAt: number; updatedAt: number }
export interface MachineAccount { id: string; name: string; clientId: string; lastUsedAt: number | null; revokedAt: number | null; createdAt: number }
export interface IssuedMachineAccount extends MachineAccount { accessToken: string }

export interface S3Config { endpoint: string; region: string; bucket: string; prefix: string; pathStyle: boolean }
export interface WebDavConfig { endpoint: string; prefix: string }
export type BackupConfig = { kind: "S3"; settings: S3Config } | { kind: "WEBDAV"; settings: WebDavConfig }
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
  createdAt: number
  updatedAt: number
}
export interface BackupJob { id: string; targetId: string; triggerKind: string; status: string; objectKey: string; byteSize: number | null; errorCode: string | null; createdAt: number; completedAt: number | null }
