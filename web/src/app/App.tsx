import { Banner } from "@astryxdesign/core/Banner"
import { Button } from "@astryxdesign/core/Button"
import { Card } from "@astryxdesign/core/Card"
import { IconButton } from "@astryxdesign/core/IconButton"
import { Tooltip } from "@astryxdesign/core/Tooltip"
import {
  ArchiveRestore, Boxes, Check, ChevronRight, CircleGauge, CloudUpload, Copy, DatabaseBackup,
  ExternalLink, FileDown, FileUp, FolderKanban, Globe2, Info, KeyRound, Languages, LogOut, Menu,
  Monitor, Moon, Network, Pencil, Plus, RefreshCw, Search, ServerCog, Settings, ShieldCheck,
  Sun, Trash2, UserCog, UsersRound, X,
} from "lucide-react"
import { useCallback, useEffect, useMemo, useState, type FormEvent, type ReactNode } from "react"

import { ApiError, api } from "./api"
import type {
  AccessPolicy, AuditEvent, AuditSettings, BackupCapabilities, BackupConfig, BackupEncryption, BackupJob, BackupScopes, BackupTarget, GrantInput, Group,
  IssuedMachineAccount, Locale, MachineAccount, NamedGrant, Overview, Project, Role,
  Secret, Session, ThemeMode, User,
} from "./types"
import { useI18n } from "../i18n/I18nProvider"
import type { MessageKey } from "../i18n/messages"
import type { AstryxThemeName } from "../theme/astryxThemes"
import { ThemePicker } from "../theme/ThemePicker"
import { useTheme } from "../theme/ThemeProvider"

type Page = "dashboard" | "projects" | "secrets" | "machines" | "users" | "groups" | "audit" | "trash" | "integrations" | "backups" | "transfer" | "settings"
type Notice = { text: string; error?: boolean } | null
const adminPages = new Set<Page>(["machines", "users", "groups", "audit", "backups", "transfer"])
type AccessResource = { kind: "project" | "secret" | "machine"; id: string; name: string }
type GrantBucketKey = "users" | "groups" | "machines" | "projects"
type GrantLevel = "none" | "read" | "write"
type GrantBuckets<T> = Record<GrantBucketKey, T[]>
type AccessSection = { key: GrantBucketKey; label: string; items: Array<{ id: string; name: string; detail?: string }> }

export function App() {
  const [session, setSession] = useState<Session | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    api.session().then(setSession).catch(() => setSession(null)).finally(() => setLoading(false))
  }, [])

  if (loading) return <LoadingScreen />
  if (!session) return <LoginPage onAuthenticated={setSession} />
  return <Workspace session={session} onSignedOut={() => setSession(null)} />
}

function LoadingScreen() {
  const { t } = useI18n()
  return <main className="loading-screen"><BrandMark /><strong>{t("loading")}</strong></main>
}

function LoginPage({ onAuthenticated }: { onAuthenticated: (session: Session) => void }) {
  const { locale, setLocale, t } = useI18n()
  const { resolved, setMode } = useTheme()
  const [username, setUsername] = useState("")
  const [password, setPassword] = useState("")
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState(false)

  async function submit(event: FormEvent) {
    event.preventDefault()
    if (!username || !password || busy) return
    setBusy(true); setError(false)
    try { onAuthenticated(await api.login(username, password)) }
    catch { setError(true) }
    finally { setBusy(false) }
  }

  return (
    <main className="login-page">
      <header className="login-tools">
        <div className="login-language" role="group" aria-label={t("language")}>
          <Languages aria-hidden="true" />
          <button type="button" aria-pressed={locale === "zh-CN"} data-active={locale === "zh-CN"} onClick={() => setLocale("zh-CN")}>{t("chinese")}</button>
          <button type="button" aria-pressed={locale === "en"} data-active={locale === "en"} onClick={() => setLocale("en")}>{t("english")}</button>
        </div>
        <IconButton variant="ghost" label={resolved === "dark" ? t("light") : t("dark")} tooltip={resolved === "dark" ? t("light") : t("dark")} icon={resolved === "dark" ? <Sun /> : <Moon />} onClick={() => setMode(resolved === "dark" ? "light" : "dark")} />
      </header>
      <section className="login-grid">
        <div className="login-intro">
          <Brand hero />
          <p className="eyebrow">SELF-HOSTED · SDK-COMPATIBLE</p>
          <h1>{t("appTagline")}</h1>
          <p>{t("loginDescription")}</p>
          <div className="trust-line"><ShieldCheck /><span>SQLite WAL · Argon2id · AES-256-GCM</span></div>
        </div>
        <Card width="100%" maxWidth={440} padding={8} className="login-card">
          <form onSubmit={submit} className="form-stack">
            <div><h2>{t("loginTitle")}</h2><p className="muted">{t("loginDescription")}</p></div>
            <Field label={t("username")} value={username} onChange={setUsername} autoComplete="username" autoFocus />
            <Field label={t("password")} value={password} onChange={setPassword} type="password" autoComplete="current-password" />
            {error && <Banner status="error" title={t("loginError")} />}
            <Button type="submit" variant="primary" size="lg" width="100%" label={busy ? t("signingIn") : t("signIn")} isLoading={busy} isDisabled={!username || !password} />
          </form>
        </Card>
      </section>
    </main>
  )
}

function Workspace({ session, onSignedOut }: { session: Session; onSignedOut: () => void }) {
  const { t } = useI18n()
  const isAdmin = session.user.role === "admin"
  const [page, setPage] = useState<Page>(() => availablePage(location.hash.slice(1), isAdmin))
  const [mobileOpen, setMobileOpen] = useState(false)
  const [notice, setNotice] = useState<Notice>(null)
  const notify = useCallback((text: string, error = false) => setNotice({ text, error }), [])

  useEffect(() => {
    const change = () => {
      const next = availablePage(location.hash.slice(1), isAdmin)
      if (location.hash.slice(1) !== next) history.replaceState(null, "", `#${next}`)
      setPage(next)
    }
    change()
    addEventListener("hashchange", change)
    return () => removeEventListener("hashchange", change)
  }, [isAdmin])
  useEffect(() => {
    if (!notice) return
    const timer = setTimeout(() => setNotice(null), 5000)
    return () => clearTimeout(timer)
  }, [notice])
  function navigate(next: Page) {
    location.hash = next
    setPage(next)
    setMobileOpen(false)
  }
  async function logout() {
    await api.logout().catch(() => undefined)
    onSignedOut()
  }
  const pageProps = { notify }

  return (
    <div className="workspace">
      <aside className="sidebar" data-open={mobileOpen}>
        <div className="sidebar-head"><Brand /><IconButton className="mobile-only" variant="ghost" label={t("mobileClose")} icon={<X />} onClick={() => setMobileOpen(false)} /></div>
        <nav aria-label={t("menu")}>
          <NavItem page="dashboard" current={page} icon={<CircleGauge />} label={t("dashboard")} onClick={navigate} />
          <NavItem page="projects" current={page} icon={<FolderKanban />} label={t("projects")} onClick={navigate} />
          <NavItem page="secrets" current={page} icon={<KeyRound />} label={t("secrets")} onClick={navigate} />
          {isAdmin && <NavItem page="machines" current={page} icon={<ServerCog />} label={t("machines")} onClick={navigate} />}
          {isAdmin && <NavItem page="users" current={page} icon={<UsersRound />} label={t("users")} onClick={navigate} />}
          {isAdmin && <NavItem page="groups" current={page} icon={<Boxes />} label={t("groups")} onClick={navigate} />}
          {isAdmin && <NavItem page="audit" current={page} icon={<ShieldCheck />} label={t("auditLog")} onClick={navigate} />}
          <NavItem page="trash" current={page} icon={<Trash2 />} label={t("trash")} onClick={navigate} />
          <NavItem page="integrations" current={page} icon={<Network />} label={t("integrations")} onClick={navigate} />
          {isAdmin && <div className="nav-separator" />}
          {isAdmin && <NavItem page="backups" current={page} icon={<DatabaseBackup />} label={t("backups")} onClick={navigate} />}
          {isAdmin && <NavItem page="transfer" current={page} icon={<ArchiveRestore />} label={t("transfer")} onClick={navigate} />}
          <NavItem page="settings" current={page} icon={<Settings />} label={t("settings")} onClick={navigate} />
        </nav>
        <div className="sidebar-account">
          <div className="avatar" aria-hidden="true">{session.user.displayName.slice(0, 2).toUpperCase()}</div>
          <div><strong>{session.user.displayName}</strong><small>{session.user.role === "admin" ? t("administrator") : t("member")}</small></div>
          <IconButton variant="ghost" label={t("logout")} tooltip={t("logout")} icon={<LogOut />} onClick={() => void logout()} />
        </div>
      </aside>
      {mobileOpen && <button className="sidebar-scrim" aria-label={t("mobileClose")} onClick={() => setMobileOpen(false)} />}
      <main className="main-canvas">
        <div className="mobile-bar"><IconButton variant="ghost" label={t("menu")} icon={<Menu />} onClick={() => setMobileOpen(true)} /><Brand compact /></div>
        {page === "dashboard" && <DashboardPage {...pageProps} />}
        {page === "projects" && <ProjectsPage isAdmin={isAdmin} {...pageProps} />}
        {page === "secrets" && <SecretsPage isAdmin={isAdmin} {...pageProps} />}
        {page === "machines" && isAdmin && <MachinesPage {...pageProps} />}
        {page === "users" && isAdmin && <UsersPage currentUser={session.user} {...pageProps} />}
        {page === "groups" && isAdmin && <GroupsPage {...pageProps} />}
        {page === "audit" && isAdmin && <AuditPage {...pageProps} />}
        {page === "trash" && <TrashPage isAdmin={isAdmin} {...pageProps} />}
        {page === "integrations" && <IntegrationsPage />}
        {page === "backups" && isAdmin && <BackupsPage {...pageProps} />}
        {page === "transfer" && isAdmin && <TransferPage {...pageProps} />}
        {page === "settings" && <SettingsPage />}
      </main>
      {notice && <div className="toast" data-error={notice.error || undefined} role={notice.error ? "alert" : "status"}>{notice.error ? <X /> : <Check />}<span>{notice.text}</span></div>}
    </div>
  )
}

function BrandMark() {
  return <span className="brand-mark" aria-hidden="true"><img src="/lightbws-logo.webp" alt="" /></span>
}

function Brand({ compact = false, hero = false }: { compact?: boolean; hero?: boolean }) {
  return <div className="brand" data-hero={hero || undefined}><BrandMark />{!compact && <div><strong>LightBWS</strong><small>Secrets Control Plane</small></div>}</div>
}

function NavItem({ page, current, icon, label, onClick }: { page: Page; current: Page; icon: ReactNode; label: string; onClick: (page: Page) => void }) {
  return <button className="nav-item" data-active={current === page} aria-current={current === page ? "page" : undefined} onClick={() => onClick(page)}>{icon}<span>{label}</span><ChevronRight className="nav-chevron" /></button>
}

function PageHeader({ title, description, action }: { title: string; description?: string; action?: ReactNode }) {
  return <header className="page-header"><div><h1>{title}</h1>{description && <p>{description}</p>}</div>{action}</header>
}

function DashboardPage({ notify }: { notify: (text: string, error?: boolean) => void }) {
  const { t } = useI18n()
  const [overview, setOverview] = useState<Overview | null>(null)
  useEffect(() => { api.overview().then(setOverview).catch(() => notify(t("genericError"), true)) }, [notify, t])
  return <div className="page"><PageHeader title={t("dashboard")} description={t("appTagline")} />
    <section className="metric-grid">
      <Metric icon={<FolderKanban />} label={t("projectsCount")} value={overview?.projects} />
      <Metric icon={<KeyRound />} label={t("secretsCount")} value={overview?.secrets} />
      <Metric icon={<Trash2 />} label={t("trashCount")} value={overview?.trash} />
    </section>
    <section className="dashboard-grid">
      <article className="panel callout-panel"><span className="panel-icon"><ShieldCheck /></span><div><h2>{t("securityBoundary")}</h2><p>{t("securityBoundaryText")}</p></div></article>
      <article className="panel"><div className="panel-heading"><div><p className="eyebrow">SYSTEM</p><h2>{t("recentActivity")}</h2></div><span className="status-dot" /></div><p>{t("healthReady")}</p><code>GET /health · 200 OK</code></article>
    </section>
  </div>
}

function Metric({ icon, label, value }: { icon: ReactNode; label: string; value?: number }) {
  return <article className="metric"><span>{icon}</span><div><strong>{value ?? "—"}</strong><small>{label}</small></div></article>
}

function ProjectsPage({ isAdmin, notify }: { isAdmin: boolean; notify: (text: string, error?: boolean) => void }) {
  const { t } = useI18n()
  const [items, setItems] = useState<Project[]>([])
  const [query, setQuery] = useState("")
  const [editing, setEditing] = useState<Project | "new" | null>(null)
  const [accessItem, setAccessItem] = useState<Project | null>(null)
  const load = useCallback(() => api.projects().then(setItems).catch(() => notify(t("genericError"), true)), [notify, t])
  useEffect(() => { void load() }, [load])
  const filtered = items.filter((item) => item.name.toLowerCase().includes(query.toLowerCase()))
  async function save(name: string) {
    try { editing === "new" ? await api.createProject(name) : editing && await api.updateProject(editing.id, name); setEditing(null); load() }
    catch { notify(t("genericError"), true) }
  }
  async function remove(item: Project) { try { await api.trashProject(item.id); load() } catch { notify(t("genericError"), true) } }
  return <div className="page"><PageHeader title={t("projects")} action={isAdmin ? <Button variant="primary" icon={<Plus />} label={`${t("new")} ${t("project")}`} onClick={() => setEditing("new")} /> : undefined} />
    <ListToolbar query={query} onQuery={setQuery} />
    <DataPanel empty={!filtered.length} onEmptyAction={isAdmin ? () => setEditing("new") : undefined}>
      <table><thead><tr><th>{t("projectName")}</th><th>{t("permission")}</th><th>{t("lastUpdated")}</th><th className="action-column">{t("actions")}</th></tr></thead>
        <tbody>{filtered.map((item) => <tr key={item.id}><td><div className="title-cell"><span className="row-icon"><FolderKanban /></span><div><strong>{item.name}</strong><small>{item.sdkEncrypted ? t("encryptedSdk") : item.id}</small></div></div></td><td><StatusPill value={item.permissions.write ? "readWrite" : "readOnly"} /></td><td>{formatDate(item.updatedAt)}</td><td><RowActions>{isAdmin && <IconButton variant="ghost" label={t("manageAccess")} icon={<ShieldCheck />} onClick={() => setAccessItem(item)} />}{isAdmin && <IconButton variant="ghost" label={t("edit")} icon={<Pencil />} onClick={() => setEditing(item)} />}{isAdmin && <IconButton variant="ghost" label={t("delete")} icon={<Trash2 />} onClick={() => void remove(item)} />}</RowActions></td></tr>)}</tbody>
      </table>
    </DataPanel>
    {isAdmin && editing && <ProjectDialog item={editing === "new" ? null : editing} onClose={() => setEditing(null)} onSave={save} />}
    {isAdmin && accessItem && <ResourceAccessDialog resource={{ kind: "project", id: accessItem.id, name: accessItem.name }} onClose={() => setAccessItem(null)} notify={notify} />}
  </div>
}

function ProjectDialog({ item, onClose, onSave }: { item: Project | null; onClose: () => void; onSave: (name: string) => Promise<void> }) {
  const { t } = useI18n(); const [name, setName] = useState(item?.name || ""); const [busy, setBusy] = useState(false)
  async function submit(event: FormEvent) { event.preventDefault(); if (!name.trim()) return; setBusy(true); await onSave(name); setBusy(false) }
  return <Modal title={`${item ? t("edit") : t("new")} ${t("project")}`} onClose={onClose}><form className="form-stack" onSubmit={submit}><Field label={t("projectName")} value={name} onChange={setName} autoFocus /><DialogActions onClose={onClose} busy={busy} disabled={!name.trim()} /></form></Modal>
}

function SecretsPage({ isAdmin, notify }: { isAdmin: boolean; notify: (text: string, error?: boolean) => void }) {
  const { t } = useI18n()
  const [items, setItems] = useState<Secret[]>([])
  const [projects, setProjects] = useState<Project[]>([])
  const [query, setQuery] = useState("")
  const [editing, setEditing] = useState<Secret | "new" | null>(null)
  const [accessItem, setAccessItem] = useState<Secret | null>(null)
  const [revealed, setRevealed] = useState<Set<string>>(new Set())
  const load = useCallback(() => Promise.all([api.secrets(), api.projects()]).then(([secrets, projects]) => { setItems(secrets); setProjects(projects) }).catch(() => notify(t("genericError"), true)), [notify, t])
  useEffect(() => { void load() }, [load])
  const filtered = items.filter((item) => `${item.key} ${item.note}`.toLowerCase().includes(query.toLowerCase()))
  const canCreate = projects.some((project) => project.permissions.write)
  async function save(input: { key: string; value: string; note: string; projectId: string }) {
    try { editing === "new" ? await api.createSecret(input) : editing && await api.updateSecret(editing.id, input); setEditing(null); await load() }
    catch { notify(t("genericError"), true) }
  }
  async function remove(item: Secret) { try { await api.trashSecret(item.id); await load() } catch { notify(t("genericError"), true) } }
  async function copy(value: string) { await navigator.clipboard.writeText(value); notify(t("copied")) }
  return <div className="page"><PageHeader title={t("secrets")} action={canCreate ? <Button variant="primary" icon={<Plus />} label={`${t("new")} ${t("secret")}`} onClick={() => setEditing("new")} /> : undefined} />
    <ListToolbar query={query} onQuery={setQuery} />
    <DataPanel empty={!filtered.length} onEmptyAction={canCreate ? () => setEditing("new") : undefined}>
      <table><thead><tr><th>{t("secretKey")}</th><th>{t("secretValue")}</th><th>{t("project")}</th><th>{t("permission")}</th><th>{t("lastUpdated")}</th><th className="action-column">{t("actions")}</th></tr></thead>
        <tbody>{filtered.map((item) => <tr key={item.id}><td><div className="title-cell"><span className="row-icon"><KeyRound /></span><div><strong>{item.key}</strong>{item.sdkEncrypted && <small>{t("encryptedSdk")}</small>}</div></div></td><td><code className="secret-value">{item.value === null ? "••••••••" : revealed.has(item.id) ? item.value : "••••••••••••"}</code></td><td>{projects.find((project) => project.id === item.projectId)?.name || "—"}</td><td><StatusPill value={item.permissions.write ? "readWrite" : "readOnly"} /></td><td>{formatDate(item.updatedAt)}</td><td><RowActions>{item.value !== null && <IconButton variant="ghost" label={t("secretValue")} icon={<KeyRound />} onClick={() => setRevealed((current) => { const next = new Set(current); next.has(item.id) ? next.delete(item.id) : next.add(item.id); return next })} />}{item.value !== null && <IconButton variant="ghost" label={t("copy")} icon={<Copy />} onClick={() => void copy(item.value || "")} />}{isAdmin && <IconButton variant="ghost" label={t("manageAccess")} icon={<ShieldCheck />} onClick={() => setAccessItem(item)} />}{!item.sdkEncrypted && item.permissions.write && <IconButton variant="ghost" label={t("edit")} icon={<Pencil />} onClick={() => setEditing(item)} />}{item.permissions.write && <IconButton variant="ghost" label={t("delete")} icon={<Trash2 />} onClick={() => void remove(item)} />}</RowActions></td></tr>)}</tbody>
      </table>
    </DataPanel>
    {editing && <SecretDialog item={editing === "new" ? null : editing} projects={projects} onClose={() => setEditing(null)} onSave={save} />}
    {isAdmin && accessItem && <ResourceAccessDialog resource={{ kind: "secret", id: accessItem.id, name: accessItem.key }} onClose={() => setAccessItem(null)} notify={notify} />}
  </div>
}

function SecretDialog({ item, projects, onClose, onSave }: { item: Secret | null; projects: Project[]; onClose: () => void; onSave: (input: { key: string; value: string; note: string; projectId: string }) => Promise<void> }) {
  const { t } = useI18n(); const allowedProjects = projects.filter((project) => project.permissions.write || project.id === item?.projectId); const [key, setKey] = useState(item?.key || ""); const [value, setValue] = useState(item?.value || ""); const [note, setNote] = useState(item?.note || ""); const [projectId, setProjectId] = useState(item?.projectId || allowedProjects[0]?.id || ""); const [busy, setBusy] = useState(false)
  async function submit(event: FormEvent) { event.preventDefault(); if (!key.trim() || !projectId) return; setBusy(true); await onSave({ key, value, note, projectId }); setBusy(false) }
  return <Modal title={`${item ? t("edit") : t("new")} ${t("secret")}`} onClose={onClose} wide><form className="form-stack" onSubmit={submit}><div className="form-grid"><Field label={t("secretKey")} value={key} onChange={setKey} autoFocus /><Field label={t("secretValue")} value={value} onChange={setValue} /></div><TextArea label={t("note")} value={note} onChange={setNote} /><SelectField label={t("project")} value={projectId} onChange={setProjectId} options={allowedProjects.map((project) => ({ value: project.id, label: project.name }))} /><DialogActions onClose={onClose} busy={busy} disabled={!key.trim() || !projectId} /></form></Modal>
}

function MachinesPage({ notify }: { notify: (text: string, error?: boolean) => void }) {
  const { t } = useI18n(); const [items, setItems] = useState<MachineAccount[]>([]); const [creating, setCreating] = useState(false); const [issued, setIssued] = useState<IssuedMachineAccount | null>(null); const [accessItem, setAccessItem] = useState<MachineAccount | null>(null)
  const load = useCallback(() => api.machines().then(setItems).catch(() => notify(t("genericError"), true)), [notify, t]); useEffect(() => { void load() }, [load])
  async function create(name: string) { try { const value = await api.createMachine(name); setCreating(false); setIssued(value); load() } catch { notify(t("genericError"), true) } }
  async function toggle(item: MachineAccount) { try { await api.setMachineRevoked(item.id, !item.revokedAt); load() } catch { notify(t("genericError"), true) } }
  async function remove(item: MachineAccount) { try { await api.deleteMachine(item.id); load() } catch { notify(t("genericError"), true) } }
  return <div className="page"><PageHeader title={t("machines")} action={<Button variant="primary" icon={<Plus />} label={`${t("new")} ${t("machineAccount")}`} onClick={() => setCreating(true)} />} /><DataPanel empty={!items.length} onEmptyAction={() => setCreating(true)}><table><thead><tr><th>{t("machineName")}</th><th>{t("clientId")}</th><th>{t("lastUsed")}</th><th>{t("status")}</th><th className="action-column">{t("actions")}</th></tr></thead><tbody>{items.map((item) => <tr key={item.id}><td><div className="title-cell"><span className="row-icon"><ServerCog /></span><div><strong>{item.name}</strong>{item.compatibilityAccount && <small>{t("compatibilityAccount")}</small>}</div></div></td><td><code>{item.clientId}</code></td><td>{item.lastUsedAt ? formatDate(item.lastUsedAt) : t("never")}</td><td><StatusPill value={item.revokedAt ? "revoked" : "active"} /></td><td><RowActions>{!item.compatibilityAccount && <IconButton variant="ghost" label={t("manageAccess")} icon={<ShieldCheck />} onClick={() => setAccessItem(item)} />}<Button size="sm" variant="ghost" label={item.revokedAt ? t("enable") : t("revoke")} onClick={() => void toggle(item)} />{!item.compatibilityAccount && <IconButton variant="ghost" label={t("delete")} icon={<Trash2 />} onClick={() => void remove(item)} />}</RowActions></td></tr>)}</tbody></table></DataPanel>{creating && <NameDialog title={`${t("new")} ${t("machineAccount")}`} label={t("machineName")} onClose={() => setCreating(false)} onSave={create} />}{issued && <TokenDialog item={issued} onClose={() => setIssued(null)} notify={notify} />}{accessItem && <ResourceAccessDialog resource={{ kind: "machine", id: accessItem.id, name: accessItem.name }} onClose={() => setAccessItem(null)} notify={notify} />}</div>
}

function TokenDialog({ item, onClose, notify }: { item: IssuedMachineAccount; onClose: () => void; notify: (text: string) => void }) {
  const { t } = useI18n(); async function copy() { await navigator.clipboard.writeText(item.accessToken); notify(t("copied")) }
  return <Modal title={t("accessToken")} onClose={onClose} wide><Banner status="warning" title={t("tokenOneTime")} /><div className="token-box"><code>{item.accessToken}</code><IconButton variant="ghost" label={t("copy")} icon={<Copy />} onClick={() => void copy()} /></div><div className="dialog-actions"><Button variant="primary" label={t("close")} onClick={onClose} /></div></Modal>
}

function UsersPage({ currentUser, notify }: { currentUser: User; notify: (text: string, error?: boolean) => void }) {
  const { t } = useI18n(); const [items, setItems] = useState<User[]>([]); const [editing, setEditing] = useState<User | "new" | null>(null); const [passwordUser, setPasswordUser] = useState<User | null>(null)
  const load = useCallback(() => api.users().then(setItems).catch(() => notify(t("genericError"), true)), [notify, t]); useEffect(() => { void load() }, [load])
  async function save(input: { username: string; displayName: string; role: Role; password: string; disabled: boolean }) { try { editing === "new" ? await api.createUser(input) : editing && await api.updateUser(editing.id, input); setEditing(null); load() } catch { notify(t("genericError"), true) } }
  async function password(value: string) { if (!passwordUser) return; try { await api.resetPassword(passwordUser.id, value); setPasswordUser(null); notify(t("save")); } catch { notify(t("genericError"), true) } }
  return <div className="page"><PageHeader title={t("users")} action={<Button variant="primary" icon={<Plus />} label={`${t("new")} ${t("user")}`} onClick={() => setEditing("new")} />} /><DataPanel empty={!items.length} onEmptyAction={() => setEditing("new")}><table><thead><tr><th>{t("displayName")}</th><th>{t("username")}</th><th>{t("role")}</th><th>{t("status")}</th><th className="action-column">{t("actions")}</th></tr></thead><tbody>{items.map((item) => <tr key={item.id}><td><div className="title-cell"><span className="avatar small">{item.displayName.slice(0, 2).toUpperCase()}</span><strong>{item.displayName}</strong></div></td><td>{item.username}</td><td>{item.role === "admin" ? t("administrator") : t("member")}</td><td><StatusPill value={item.disabled ? "disabled" : "active"} /></td><td><RowActions><IconButton variant="ghost" label={t("edit")} icon={<UserCog />} onClick={() => setEditing(item)} /><Button size="sm" variant="ghost" label={t("resetPassword")} onClick={() => setPasswordUser(item)} /></RowActions></td></tr>)}</tbody></table></DataPanel>{editing && <UserDialog item={editing === "new" ? null : editing} currentUser={currentUser} onClose={() => setEditing(null)} onSave={save} />}{passwordUser && <NameDialog title={t("resetPassword")} label={t("newPassword")} type="password" onClose={() => setPasswordUser(null)} onSave={password} />}</div>
}

function UserDialog({ item, currentUser, onClose, onSave }: { item: User | null; currentUser: User; onClose: () => void; onSave: (input: { username: string; displayName: string; role: Role; password: string; disabled: boolean }) => Promise<void> }) {
  const { t } = useI18n(); const [username, setUsername] = useState(item?.username || ""); const [displayName, setDisplayName] = useState(item?.displayName || ""); const [password, setPassword] = useState(""); const [role, setRole] = useState<Role>(item?.role || "user"); const [disabled, setDisabled] = useState(item?.disabled || false); const [busy, setBusy] = useState(false); const self = item?.id === currentUser.id
  async function submit(event: FormEvent) { event.preventDefault(); setBusy(true); await onSave({ username, displayName, password, role, disabled }); setBusy(false) }
  return <Modal title={`${item ? t("edit") : t("new")} ${t("user")}`} onClose={onClose}><form className="form-stack" onSubmit={submit}><Field label={t("username")} value={username} onChange={setUsername} disabled={Boolean(item)} autoFocus /><Field label={t("displayName")} value={displayName} onChange={setDisplayName} />{!item && <Field label={t("password")} value={password} onChange={setPassword} type="password" />}<SelectField label={t("role")} value={role} onChange={(value) => setRole(value as Role)} disabled={self} options={[{ value: "user", label: t("member") }, { value: "admin", label: t("administrator") }]} />{item && <CheckField label={t("disabled")} checked={disabled} onChange={setDisabled} disabled={self} />}<DialogActions onClose={onClose} busy={busy} disabled={!username.trim() || !displayName.trim() || (!item && password.length < 8)} /></form></Modal>
}

function GroupsPage({ notify }: { notify: (text: string, error?: boolean) => void }) {
  const { t } = useI18n()
  const [groups, setGroups] = useState<Group[]>([])
  const [users, setUsers] = useState<User[]>([])
  const [editing, setEditing] = useState<Group | "new" | null>(null)
  const [membersGroup, setMembersGroup] = useState<Group | null>(null)
  const load = useCallback(() => Promise.all([api.groups(), api.users()]).then(([nextGroups, nextUsers]) => { setGroups(nextGroups); setUsers(nextUsers) }).catch(() => notify(t("genericError"), true)), [notify, t])
  useEffect(() => { void load() }, [load])
  async function save(name: string) { try { editing === "new" ? await api.createGroup(name) : editing && await api.updateGroup(editing.id, name); setEditing(null); await load() } catch { notify(t("genericError"), true) } }
  async function remove(group: Group) { if (!confirm(t("confirmDeleteGroup"))) return; try { await api.deleteGroup(group.id); await load() } catch { notify(t("genericError"), true) } }
  async function saveMembers(memberIds: string[]) { if (!membersGroup) return; try { await api.replaceGroupMembers(membersGroup.id, memberIds); setMembersGroup(null); notify(t("groupMembersSaved")); await load() } catch { notify(t("genericError"), true) } }
  return <div className="page"><PageHeader title={t("groups")} description={t("groupsIntro")} action={<Button variant="primary" icon={<Plus />} label={`${t("new")} ${t("group")}`} onClick={() => setEditing("new")} />} /><DataPanel empty={!groups.length} onEmptyAction={() => setEditing("new")}><table><thead><tr><th>{t("groupName")}</th><th>{t("members")}</th><th>{t("lastUpdated")}</th><th className="action-column">{t("actions")}</th></tr></thead><tbody>{groups.map((group) => <tr key={group.id}><td><div className="title-cell"><span className="row-icon"><Boxes /></span><strong>{group.name}</strong></div></td><td>{group.memberIds.length}</td><td>{formatDate(group.updatedAt)}</td><td><RowActions><Button size="sm" variant="ghost" label={t("manageMembers")} onClick={() => setMembersGroup(group)} /><IconButton variant="ghost" label={t("edit")} icon={<Pencil />} onClick={() => setEditing(group)} /><IconButton variant="ghost" label={t("delete")} icon={<Trash2 />} onClick={() => void remove(group)} /></RowActions></td></tr>)}</tbody></table></DataPanel>{editing && <NameDialog title={`${editing === "new" ? t("new") : t("edit")} ${t("group")}`} label={t("groupName")} initialValue={editing === "new" ? "" : editing.name} onClose={() => setEditing(null)} onSave={save} />}{membersGroup && <GroupMembersDialog group={membersGroup} users={users} onClose={() => setMembersGroup(null)} onSave={saveMembers} />}</div>
}

function GroupMembersDialog({ group, users, onClose, onSave }: { group: Group; users: User[]; onClose: () => void; onSave: (memberIds: string[]) => Promise<void> }) {
  const { t } = useI18n(); const [selected, setSelected] = useState(() => new Set(group.memberIds)); const [busy, setBusy] = useState(false)
  async function submit(event: FormEvent) { event.preventDefault(); setBusy(true); await onSave([...selected]); setBusy(false) }
  return <Modal title={`${t("manageMembers")} · ${group.name}`} onClose={onClose} wide><form className="form-stack" onSubmit={submit}><div className="member-picker">{users.map((user) => <label className="member-option" key={user.id}><input type="checkbox" checked={selected.has(user.id)} onChange={(event) => setSelected((current) => { const next = new Set(current); event.target.checked ? next.add(user.id) : next.delete(user.id); return next })} /><span className="avatar small">{user.displayName.slice(0, 2).toUpperCase()}</span><span><strong>{user.displayName}</strong><small>{user.username}</small></span></label>)}</div><DialogActions onClose={onClose} busy={busy} /></form></Modal>
}

function ResourceAccessDialog({ resource, onClose, notify }: { resource: AccessResource; onClose: () => void; notify: (text: string, error?: boolean) => void }) {
  const { t } = useI18n()
  const [sections, setSections] = useState<AccessSection[]>([])
  const [levels, setLevels] = useState<Record<string, GrantLevel>>({})
  const [loading, setLoading] = useState(true)
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    let active = true
    async function load() {
      try {
        const [users, groups] = await Promise.all([api.users(), api.groups()])
        let nextSections: AccessSection[]
        let current: GrantBuckets<NamedGrant>
        if (resource.kind === "machine") {
          const [projects, policy] = await Promise.all([api.projects(), api.machineAccess(resource.id)])
          nextSections = [
            { key: "users", label: t("users"), items: users.map((user) => ({ id: user.id, name: user.displayName, detail: user.username })) },
            { key: "groups", label: t("groups"), items: groups.map((group) => ({ id: group.id, name: group.name, detail: `${group.memberIds.length} ${t("members")}` })) },
            { key: "projects", label: t("projects"), items: projects.map((project) => ({ id: project.id, name: project.name })) },
          ]
          current = { users: policy.users, groups: policy.groups, machines: [], projects: policy.projects }
        } else {
          const [machines, policy] = await Promise.all([
            api.machines(),
            resource.kind === "project" ? api.projectAccess(resource.id) : api.secretAccess(resource.id),
          ])
          nextSections = [
            { key: "users", label: t("users"), items: users.map((user) => ({ id: user.id, name: user.displayName, detail: user.username })) },
            { key: "groups", label: t("groups"), items: groups.map((group) => ({ id: group.id, name: group.name, detail: `${group.memberIds.length} ${t("members")}` })) },
            { key: "machines", label: t("machines"), items: machines.filter((machine) => !machine.compatibilityAccount).map((machine) => ({ id: machine.id, name: machine.name, detail: machine.clientId })) },
          ]
          const access = policy as AccessPolicy
          current = { users: access.users, groups: access.groups, machines: access.machines, projects: [] }
        }
        const nextLevels: Record<string, GrantLevel> = {}
        for (const section of nextSections) {
          for (const item of section.items) {
            const grant = current[section.key].find((candidate) => candidate.granteeId === item.id)
            nextLevels[grantKey(section.key, item.id)] = grant?.write ? "write" : grant?.read ? "read" : "none"
          }
        }
        if (active) { setSections(nextSections); setLevels(nextLevels); setLoading(false) }
      } catch {
        if (active) { notify(t("genericError"), true); onClose() }
      }
    }
    void load()
    return () => { active = false }
  }, [notify, onClose, resource, t])

  async function submit(event: FormEvent) {
    event.preventDefault()
    setBusy(true)
    const input: GrantBuckets<GrantInput> = { users: [], groups: [], machines: [], projects: [] }
    for (const section of sections) {
      for (const item of section.items) {
        const level = levels[grantKey(section.key, item.id)] || "none"
        if (level !== "none") input[section.key].push({ granteeId: item.id, read: true, write: level === "write" })
      }
    }
    try {
      if (resource.kind === "machine") {
        await api.updateMachineAccess(resource.id, { users: input.users, groups: input.groups, projects: input.projects })
      } else if (resource.kind === "project") {
        await api.updateProjectAccess(resource.id, { users: input.users, groups: input.groups, machines: input.machines })
      } else {
        await api.updateSecretAccess(resource.id, { users: input.users, groups: input.groups, machines: input.machines })
      }
      notify(t("accessSaved"))
      onClose()
    } catch {
      notify(t("genericError"), true)
    } finally {
      setBusy(false)
    }
  }

  return <Modal title={`${t("manageAccess")} · ${resource.name}`} onClose={onClose} wide>{loading ? <div className="access-loading"><RefreshCw /><span>{t("loading")}</span></div> : <form className="form-stack" onSubmit={submit}><Banner status="info" title={t("accessPolicyHint")} />{sections.map((section) => <section className="access-section" key={section.key}><h3>{section.label}</h3>{section.items.length ? <div className="access-list">{section.items.map((item) => <label className="access-row" key={item.id}><span><strong>{item.name}</strong>{item.detail && <small>{item.detail}</small>}</span><select aria-label={`${item.name} ${t("permission")}`} value={levels[grantKey(section.key, item.id)] || "none"} onChange={(event) => setLevels((current) => ({ ...current, [grantKey(section.key, item.id)]: event.target.value as GrantLevel }))}><option value="none">{t("noAccess")}</option><option value="read">{t("readOnly")}</option><option value="write">{t("readWrite")}</option></select></label>)}</div> : <p className="muted">{t("noGrantees")}</p>}</section>)}<DialogActions onClose={onClose} busy={busy} /></form>}</Modal>
}

function AuditPage({ notify }: { notify: (text: string, error?: boolean) => void }) {
  const { t } = useI18n()
  const [events, setEvents] = useState<AuditEvent[]>([])
  const [settings, setSettings] = useState<AuditSettings | null>(null)
  const [enabled, setEnabled] = useState(true)
  const [autoCleanupEnabled, setAutoCleanupEnabled] = useState(true)
  const [retentionDays, setRetentionDays] = useState("90")
  const [busy, setBusy] = useState<"save" | "clear" | null>(null)
  const load = useCallback(() => Promise.all([api.auditEvents(), api.auditSettings()]).then(([nextEvents, nextSettings]) => {
    setEvents(nextEvents)
    setSettings(nextSettings)
    setEnabled(nextSettings.enabled)
    setAutoCleanupEnabled(nextSettings.autoCleanupEnabled)
    setRetentionDays(String(nextSettings.retentionDays))
  }).catch(() => notify(t("genericError"), true)), [notify, t])
  useEffect(() => { void load() }, [load])

  async function save(event: FormEvent) {
    event.preventDefault()
    const days = Number(retentionDays)
    if (!Number.isInteger(days) || days < 1 || days > 3650) return
    setBusy("save")
    try {
      await api.updateAuditSettings({ enabled, autoCleanupEnabled, retentionDays: days })
      notify(t("auditSettingsSaved"))
      await load()
    } catch {
      notify(t("genericError"), true)
    } finally {
      setBusy(null)
    }
  }

  async function clear() {
    if (!confirm(t("confirmClearAudit"))) return
    setBusy("clear")
    try {
      const result = await api.clearAudit()
      notify(`${t("auditCleared")} · ${result.deleted}`)
      await load()
    } catch {
      notify(t("genericError"), true)
    } finally {
      setBusy(null)
    }
  }

  return <div className="page">
    <PageHeader title={t("auditLog")} description={t("auditIntro")} action={<Button variant="ghost" icon={<Trash2 />} label={t("clearAudit")} isLoading={busy === "clear"} isDisabled={!events.length} onClick={() => void clear()} />} />
    <article className="panel settings-panel audit-settings-panel">
      <div><p className="eyebrow">RETENTION</p><h2>{t("auditPolicy")}</h2><p>{t("auditPolicyText")}</p>{settings?.lastCleanupAt && <small>{t("lastCleanup")}: {formatDate(settings.lastCleanupAt)}</small>}</div>
      <form className="form-stack" onSubmit={save}>
        <CheckField label={t("auditEnabled")} checked={enabled} onChange={setEnabled} />
        <CheckField label={t("auditAutoCleanup")} checked={autoCleanupEnabled} onChange={setAutoCleanupEnabled} />
        <Field label={t("auditRetentionDays")} value={retentionDays} onChange={setRetentionDays} type="number" hint={t("auditRetentionHint")} />
        <div className="audit-settings-actions"><Button type="submit" variant="primary" label={t("saveAuditPolicy")} isLoading={busy === "save"} isDisabled={!Number.isInteger(Number(retentionDays)) || Number(retentionDays) < 1 || Number(retentionDays) > 3650} /></div>
      </form>
    </article>
    <DataPanel empty={!events.length}>
      <table><thead><tr><th>{t("auditAction")}</th><th>{t("auditActor")}</th><th>{t("auditResource")}</th><th>{t("outcome")}</th><th>{t("time")}</th></tr></thead><tbody>{events.map((item) => <tr key={item.id}><td><code>{item.action}</code></td><td>{item.actorKind === "system" ? t("systemActor") : <span className="audit-identity">{item.actorKind === "user" ? t("user") : t("machineAccount")}<code>{shortId(item.actorId)}</code></span>}</td><td><span className="audit-identity">{item.resourceKind}<code>{shortId(item.resourceId)}</code></span></td><td><StatusPill value={item.outcome} /></td><td>{formatDate(item.createdAt)}</td></tr>)}</tbody></table>
    </DataPanel>
  </div>
}

function TrashPage({ isAdmin, notify }: { isAdmin: boolean; notify: (text: string, error?: boolean) => void }) {
  const { t } = useI18n(); const [projects, setProjects] = useState<Project[]>([]); const [secrets, setSecrets] = useState<Secret[]>([])
  const load = useCallback(() => Promise.all([api.projects(true), api.secrets(true)]).then(([projects, secrets]) => { setProjects(projects); setSecrets(secrets) }).catch(() => notify(t("genericError"), true)), [notify, t]); useEffect(() => { void load() }, [load])
  async function act(kind: "project" | "secret", id: string, purge: boolean) { if (purge && !confirm(t("confirmDelete"))) return; try { if (kind === "project") purge ? await api.purgeProject(id) : await api.restoreProject(id); else purge ? await api.purgeSecret(id) : await api.restoreSecret(id); await load() } catch { notify(t("genericError"), true) } }
  const empty = !projects.length && !secrets.length
  return <div className="page"><PageHeader title={t("trash")} description={t("dangerousAction")} /><DataPanel empty={empty}><div className="trash-groups">{projects.length > 0 && <section><h2>{t("projects")}</h2>{projects.map((item) => <TrashRow key={item.id} name={item.name} canAct={isAdmin} onRestore={() => void act("project", item.id, false)} onPurge={() => void act("project", item.id, true)} />)}</section>}{secrets.length > 0 && <section><h2>{t("secrets")}</h2>{secrets.map((item) => <TrashRow key={item.id} name={item.key} canAct={item.permissions.write} onRestore={() => void act("secret", item.id, false)} onPurge={() => void act("secret", item.id, true)} />)}</section>}</div></DataPanel></div>
}

function TrashRow({ name, canAct, onRestore, onPurge }: { name: string; canAct: boolean; onRestore: () => void; onPurge: () => void }) { const { t } = useI18n(); return <div className="trash-row"><strong>{name}</strong>{canAct ? <RowActions><Button variant="ghost" size="sm" icon={<RefreshCw />} label={t("restore")} onClick={onRestore} /><Button variant="ghost" size="sm" icon={<Trash2 />} label={t("purge")} onClick={onPurge} /></RowActions> : <StatusPill value="readOnly" />}</div> }

function IntegrationsPage() {
  const { t } = useI18n()
  const links: Array<{ title: MessageKey; href: string; detail: string }> = [
    { title: "officialSdk", href: "https://github.com/bitwarden/sdk-sm", detail: "Rust · JavaScript · Python · C#" },
    { title: "bwsCli", href: "https://github.com/bitwarden/sdk-sm/tree/main/crates/bws", detail: "bws --server-url <LIGHTBWS_URL>" },
    { title: "fnox", href: "https://fnox.jdx.dev/providers/bitwarden-sm", detail: "provider = \"bitwarden-sm\"" },
    { title: "bitwardenHelp", href: "https://bitwarden.com/help/secrets-manager-overview/", detail: "Concepts · SDK · Machine access" },
  ]
  return <div className="page"><PageHeader title={t("integrations")} description={t("integrationsIntro")} /><section className="integration-grid">{links.map((link) => <a className="integration-card" key={link.href} href={link.href} target="_blank" rel="noreferrer"><span className="panel-icon"><Network /></span><div><h2>{t(link.title)}</h2><code>{link.detail}</code><span>{t("openLink")} <ExternalLink /></span></div></a>)}</section></div>
}

function BackupsPage({ notify }: { notify: (text: string, error?: boolean) => void }) {
  const { t } = useI18n(); const [targets, setTargets] = useState<BackupTarget[]>([]); const [jobs, setJobs] = useState<BackupJob[]>([]); const [capabilities, setCapabilities] = useState<BackupCapabilities>({ plaintextAllowed: false }); const [editing, setEditing] = useState<BackupTarget | "new" | null>(null); const [busyId, setBusyId] = useState<string | null>(null)
  const load = useCallback(() => Promise.all([api.backupTargets(), api.backupJobs(), api.backupCapabilities()]).then(([targets, jobs, capabilities]) => { setTargets(targets); setJobs(jobs); setCapabilities(capabilities) }).catch(() => notify(t("genericError"), true)), [notify, t]); useEffect(() => { void load() }, [load])
  async function save(input: unknown) { try { editing === "new" ? await api.createBackupTarget(input) : editing && await api.updateBackupTarget(editing.id, input); setEditing(null); await load() } catch { notify(t("genericError"), true) } }
  async function run(target: BackupTarget) { setBusyId(target.id); try { const job = await api.runBackup(target.id); notify(job.status === "succeeded" ? t("succeeded") : t("failed"), job.status !== "succeeded"); await load() } catch { notify(t("genericError"), true) } finally { setBusyId(null) } }
  async function test(target: BackupTarget) { setBusyId(target.id); try { await api.testBackupTarget(target.id); notify(t("succeeded")) } catch { notify(t("genericError"), true) } finally { setBusyId(null) } }
  async function remove(target: BackupTarget) { if (!confirm(t("confirmDelete"))) return; try { await api.deleteBackupTarget(target.id); await load() } catch { notify(t("genericError"), true) } }
  return <div className="page"><PageHeader title={t("backups")} description={t("backupIntro")} action={<Button variant="primary" icon={<Plus />} label={t("newBackupTarget")} onClick={() => setEditing("new")} />} /><section className="backup-target-grid">{targets.map((target) => <article className="panel backup-card" key={target.id}><div className="panel-heading"><div className="title-cell"><span className="row-icon"><CloudUpload /></span><div><h2>{target.displayName}</h2><small>{target.config.kind}</small></div></div><StatusPill value={target.enabled ? target.lastStatus || "active" : "disabled"} /></div><dl><div><dt>{t("endpoint")}</dt><dd>{target.config.settings.endpoint}</dd></div><div><dt>{t("backupEncryption")}</dt><dd>{t(target.encryption === "plaintext" ? "plaintext" : "masterKeyEncrypted")}</dd></div><div><dt>{t("backupScope")}</dt><dd>{scopeLabel(target.scopes, t)}</dd></div><div><dt>{t("nextRun")}</dt><dd>{target.nextRunAt ? formatDate(target.nextRunAt) : "—"}</dd></div><div><dt>{t("lastRun")}</dt><dd>{target.lastRunAt ? formatDate(target.lastRunAt) : t("never")}</dd></div></dl><div className="card-actions"><Button size="sm" variant="secondary" label={t("runNow")} isLoading={busyId === target.id} onClick={() => void run(target)} /><Button size="sm" variant="ghost" label={t("testTarget")} onClick={() => void test(target)} /><IconButton variant="ghost" label={t("edit")} icon={<Pencil />} onClick={() => setEditing(target)} /><IconButton variant="ghost" label={t("delete")} icon={<Trash2 />} onClick={() => void remove(target)} /></div></article>)}</section>{!targets.length && <DataPanel empty onEmptyAction={() => setEditing("new")} />}
    <section className="panel history-panel"><h2>{t("backupHistory")}</h2>{jobs.length ? <table><thead><tr><th>{t("backupTarget")}</th><th>{t("status")}</th><th>{t("lastRun")}</th><th>{t("actions")}</th></tr></thead><tbody>{jobs.map((job) => <tr key={job.id}><td><code>{job.objectKey}</code></td><td><StatusPill value={job.status} /></td><td>{formatDate(job.createdAt)}</td><td>{job.triggerKind === "manual" ? t("manual") : t("scheduled")}</td></tr>)}</tbody></table> : <p className="muted">{t("emptyBackupHistory")}</p>}</section>{editing && <BackupDialog item={editing === "new" ? null : editing} plaintextAllowed={capabilities.plaintextAllowed} onClose={() => setEditing(null)} onSave={save} />}</div>
}

interface BackupForm { kind: "S3" | "WEBDAV"; displayName: string; endpoint: string; region: string; bucket: string; prefix: string; pathStyle: boolean; accessKeyId: string; secretAccessKey: string; sessionToken: string; username: string; password: string; enabled: boolean; scheduleEnabled: boolean; intervalHours: string; scopes: BackupScopes; encryption: BackupEncryption; confirmPlaintext: boolean }
function BackupDialog({ item, plaintextAllowed, onClose, onSave }: { item: BackupTarget | null; plaintextAllowed: boolean; onClose: () => void; onSave: (input: unknown) => Promise<void> }) {
  const { t } = useI18n(); const current = item?.config; const [form, setForm] = useState<BackupForm>({ kind: current?.kind || "S3", displayName: item?.displayName || "", endpoint: current?.settings.endpoint || "", region: current?.kind === "S3" ? current.settings.region : "us-east-1", bucket: current?.kind === "S3" ? current.settings.bucket : "", prefix: current?.settings.prefix || "", pathStyle: current?.kind === "S3" ? current.settings.pathStyle : true, accessKeyId: "", secretAccessKey: "", sessionToken: "", username: "", password: "", enabled: item?.enabled ?? true, scheduleEnabled: item?.scheduleEnabled ?? false, intervalHours: String(item?.intervalHours || 24), scopes: item?.scopes || defaultBackupScopes(), encryption: item?.encryption || "masterKey", confirmPlaintext: false }); const [busy, setBusy] = useState(false)
  function set<K extends keyof BackupForm>(key: K, value: BackupForm[K]) { setForm((current) => ({ ...current, [key]: value })) }
  function setScope(key: keyof BackupScopes, checked: boolean) { set("scopes", normalizeScopes({ ...form.scopes, [key]: checked }, key, checked)) }
  async function submit(event: FormEvent) { event.preventDefault(); setBusy(true); const config: BackupConfig = form.kind === "S3" ? { kind: "S3", settings: { endpoint: form.endpoint, region: form.region, bucket: form.bucket, prefix: form.prefix, pathStyle: form.pathStyle } } : { kind: "WEBDAV", settings: { endpoint: form.endpoint, prefix: form.prefix } }; const credentials = form.kind === "S3" ? { kind: "S3", values: { accessKeyId: form.accessKeyId, secretAccessKey: form.secretAccessKey, sessionToken: form.sessionToken || null } } : { kind: "WEBDAV", values: { username: form.username, password: form.password } }; await onSave({ displayName: form.displayName, config, credentials: item && !form.accessKeyId && !form.username ? null : credentials, enabled: form.enabled, scheduleEnabled: form.scheduleEnabled, intervalHours: Number(form.intervalHours), scopes: form.scopes, encryption: form.encryption, confirmPlaintext: form.confirmPlaintext }); setBusy(false) }
  const credentialReady = item || (form.kind === "S3" ? form.accessKeyId && form.secretAccessKey : form.username && form.password)
  const plaintextDanger = form.encryption === "plaintext"
  return <Modal title={item ? t("editBackupTarget") : t("newBackupTarget")} onClose={onClose} wide><form className="form-stack" onSubmit={submit}><div className="form-grid"><Field label={t("targetName")} value={form.displayName} onChange={(value) => set("displayName", value)} autoFocus /><SelectField label={t("targetType")} value={form.kind} disabled={Boolean(item)} onChange={(value) => set("kind", value as BackupForm["kind"])} options={[{ value: "S3", label: "S3" }, { value: "WEBDAV", label: "WebDAV" }]} /></div><Field label={t("endpoint")} value={form.endpoint} onChange={(value) => set("endpoint", value)} placeholder="https://…" />{form.kind === "S3" ? <><div className="form-grid"><Field label={t("region")} value={form.region} onChange={(value) => set("region", value)} /><Field label={t("bucket")} value={form.bucket} onChange={(value) => set("bucket", value)} /></div><Field label={t("accessKeyId")} value={form.accessKeyId} onChange={(value) => set("accessKeyId", value)} placeholder={item ? "••••••••" : ""} /><Field label={t("secretAccessKey")} value={form.secretAccessKey} onChange={(value) => set("secretAccessKey", value)} type="password" placeholder={item ? "••••••••" : ""} /><Field label={t("sessionToken")} value={form.sessionToken} onChange={(value) => set("sessionToken", value)} type="password" /><CheckField label={t("pathStyle")} checked={form.pathStyle} onChange={(value) => set("pathStyle", value)} /></> : <><Field label={t("webdavUsername")} value={form.username} onChange={(value) => set("username", value)} placeholder={item ? "••••••••" : ""} /><Field label={t("webdavPassword")} value={form.password} onChange={(value) => set("password", value)} type="password" placeholder={item ? "••••••••" : ""} /></>}<Field label={t("prefix")} value={form.prefix} onChange={(value) => set("prefix", value)} /><ScopePicker scopes={form.scopes} onChange={setScope} onPreset={(scopes) => set("scopes", scopes)} /><section className="backup-options"><LabelWithTip label={t("backupEncryption")} tip={t("encryptionTip")} /><div className="choice-grid two"><button type="button" className="choice-card" data-selected={form.encryption === "masterKey"} onClick={() => { set("encryption", "masterKey"); set("confirmPlaintext", false) }}><KeyRound /><span>{t("masterKeyEncrypted")}</span>{form.encryption === "masterKey" && <Check className="selection-check" />}</button>{canUsePlaintext({ plaintextAllowed }) && <button type="button" className="choice-card danger-choice" data-selected={plaintextDanger} onClick={() => set("encryption", "plaintext")}><ArchiveRestore /><span>{t("plaintext")}</span>{plaintextDanger && <Check className="selection-check" />}</button>}</div>{plaintextDanger && <><Banner status="warning" title={t("plaintextWarning")} /><CheckField label={t("confirmPlaintext")} checked={form.confirmPlaintext} onChange={(value) => set("confirmPlaintext", value)} /></>}</section><div className="form-grid"><CheckField label={t("enable")} checked={form.enabled} onChange={(value) => set("enabled", value)} /><CheckField label={t("scheduled")} checked={form.scheduleEnabled} onChange={(value) => set("scheduleEnabled", value)} /></div>{form.scheduleEnabled && <Field label={t("intervalHours")} value={form.intervalHours} onChange={(value) => set("intervalHours", value)} type="number" />}<DialogActions onClose={onClose} busy={busy} disabled={!form.displayName || !form.endpoint || !credentialReady || !plaintextSelectionReady(plaintextDanger, form.confirmPlaintext)} /></form></Modal>
}

function TransferPage({ notify }: { notify: (text: string, error?: boolean) => void }) {
  const { t } = useI18n(); const [capabilities, setCapabilities] = useState<BackupCapabilities>({ plaintextAllowed: false }); const [exportPassphrase, setExportPassphrase] = useState(""); const [exportScopes, setExportScopes] = useState(defaultBackupScopes()); const [plaintext, setPlaintext] = useState(false); const [confirmPlaintext, setConfirmPlaintext] = useState(false); const [importPassphrase, setImportPassphrase] = useState(""); const [masterKey, setMasterKey] = useState(""); const [file, setFile] = useState<File | null>(null); const [archiveKind, setArchiveKind] = useState<ArchiveKind | null>(null); const [replace, setReplace] = useState(false); const [busy, setBusy] = useState<"export" | "import" | null>(null)
  useEffect(() => { api.backupCapabilities().then(setCapabilities).catch(() => notify(t("genericError"), true)) }, [notify, t])
  async function chooseArchive(selected: File | null) { setFile(selected); setArchiveKind(selected ? await detectArchiveKind(selected) : null); setImportPassphrase(""); setMasterKey("") }
  async function exportData(event: FormEvent) { event.preventDefault(); setBusy("export"); try { const blob = await api.export({ passphrase: plaintext ? undefined : exportPassphrase, scopes: exportScopes, plaintext, confirmPlaintext }); const url = URL.createObjectURL(blob); const anchor = document.createElement("a"); anchor.href = url; anchor.download = `lightbws-${new Date().toISOString().slice(0, 10)}${plaintext ? ".plain" : ""}.lightbws`; anchor.click(); URL.revokeObjectURL(url); notify(t("exportComplete")) } catch { notify(t("genericError"), true) } finally { setBusy(null) } }
  async function importData(event: FormEvent) { event.preventDefault(); if (!file) return; if (replace && !confirm(t("confirmReplaceImport"))) return; setBusy("import"); try { const dataBase64 = await fileBase64(file); await api.import({ passphrase: archiveKind === "passphrase" ? importPassphrase : undefined, masterKey: archiveKind === "masterKey" ? masterKey : undefined, dataBase64, replace }); notify(t("importComplete")); setFile(null); setArchiveKind(null) } catch { notify(t("genericError"), true) } finally { setBusy(null) } }
  const importCredentialReady = archiveKind === "plaintext" || (archiveKind === "passphrase" && importPassphrase.length >= 12) || (archiveKind === "masterKey" && masterKey.length > 0)
  return <div className="page"><PageHeader title={t("transfer")} /><section className="transfer-grid"><article className="panel transfer-card"><span className="panel-icon"><FileDown /></span><h2>{t("exportTitle")}</h2><p>{t("exportText")}</p><form className="form-stack" onSubmit={exportData}><ScopePicker scopes={exportScopes} onChange={(key, checked) => setExportScopes(normalizeScopes({ ...exportScopes, [key]: checked }, key, checked))} onPreset={setExportScopes} /><LabelWithTip label={t("backupEncryption")} tip={t("manualEncryptionTip")} /><div className="choice-grid two"><button type="button" className="choice-card" data-selected={!plaintext} onClick={() => { setPlaintext(false); setConfirmPlaintext(false) }}><KeyRound /><span>{t("passphraseEncrypted")}</span>{!plaintext && <Check className="selection-check" />}</button>{capabilities.plaintextAllowed && <button type="button" className="choice-card danger-choice" data-selected={plaintext} onClick={() => setPlaintext(true)}><ArchiveRestore /><span>{t("plaintext")}</span>{plaintext && <Check className="selection-check" />}</button>}</div>{plaintext ? <><Banner status="warning" title={t("plaintextWarning")} /><CheckField label={t("confirmPlaintext")} checked={confirmPlaintext} onChange={setConfirmPlaintext} /></> : <Field label={t("passphrase")} value={exportPassphrase} onChange={setExportPassphrase} type="password" hint={t("passphraseHint")} />}<Button type="submit" variant="primary" icon={<FileDown />} label={t("downloadExport")} isLoading={busy === "export"} isDisabled={plaintext ? !confirmPlaintext : exportPassphrase.length < 12} /></form></article><article className="panel transfer-card"><span className="panel-icon"><FileUp /></span><h2>{t("importTitle")}</h2><p>{t("importText")}</p><form className="form-stack" onSubmit={importData}><label className="field"><span>{t("chooseFile")}</span><input type="file" accept=".lightbws,application/vnd.lightbws.backup" onChange={(event) => void chooseArchive(event.target.files?.[0] || null)} /></label>{archiveKind && <Banner status={archiveKind === "plaintext" ? "warning" : "info"} title={t(archiveKind === "passphrase" ? "passphraseArchive" : archiveKind === "masterKey" ? "automaticArchive" : archiveKind === "plaintext" ? "plaintextArchive" : "invalidArchive")} />}{archiveKind === "passphrase" && <Field label={t("passphrase")} value={importPassphrase} onChange={setImportPassphrase} type="password" hint={t("passphraseHint")} />}{archiveKind === "masterKey" && <><LabelWithTip label={t("oldMasterKey")} tip={t("oldMasterKeyTip")} /><Field label={t("oldMasterKey")} value={masterKey} onChange={setMasterKey} type="password" /><label className="field"><span>{t("chooseMasterKeyFile")}</span><input type="file" accept=".key,text/plain" onChange={(event) => void readMasterKey(event.target.files?.[0] || null).then(setMasterKey)} /></label></>}<CheckField label={t("replaceDatabase")} checked={replace} onChange={setReplace} />{replace && <Banner status="warning" title={t("replaceDatabaseWarning")} />}<Button type="submit" variant="primary" icon={<FileUp />} label={t("importFile")} isLoading={busy === "import"} isDisabled={!file || archiveKind === "unknown" || !importCredentialReady} /></form></article></section></div>
}

type ArchiveKind = "passphrase" | "masterKey" | "plaintext" | "unknown"
export function defaultBackupScopes(): BackupScopes { return { identities: false, machineAccounts: false, accessPolicies: false, audit: false, backupTargets: false } }
export function canUsePlaintext(capabilities: BackupCapabilities) { return capabilities.plaintextAllowed }
export function plaintextSelectionReady(selected: boolean, confirmed: boolean) { return !selected || confirmed }
export function fullBackupScopes(): BackupScopes { return { identities: true, machineAccounts: true, accessPolicies: true, audit: true, backupTargets: true } }
function isFullScope(scopes: BackupScopes) { return Object.values(scopes).every(Boolean) }
export function normalizeScopes(scopes: BackupScopes, key: keyof BackupScopes, checked: boolean): BackupScopes {
  const next = { ...scopes }
  next[key] = checked
  if (checked && key === "machineAccounts") next.identities = true
  if (checked && key === "accessPolicies") { next.identities = true; next.machineAccounts = true }
  if (!checked && key === "identities") { next.machineAccounts = false; next.accessPolicies = false }
  if (!checked && key === "machineAccounts") next.accessPolicies = false
  return next
}
function scopeLabel(scopes: BackupScopes, t: (key: MessageKey) => string) { return isFullScope(scopes) ? t("fullInstance") : Object.values(scopes).some(Boolean) ? t("customScope") : t("defaultScope") }
function ScopePicker({ scopes, onChange, onPreset }: { scopes: BackupScopes; onChange: (key: keyof BackupScopes, checked: boolean) => void; onPreset: (scopes: BackupScopes) => void }) {
  const { t } = useI18n(); const choices: Array<[keyof BackupScopes, MessageKey]> = [["identities", "scopeIdentities"], ["machineAccounts", "scopeMachines"], ["accessPolicies", "scopePolicies"], ["audit", "scopeAudit"], ["backupTargets", "scopeTargets"]]
  return <section className="backup-options"><LabelWithTip label={t("backupScope")} tip={t("scopeTip")} /><div className="scope-presets"><Button size="sm" variant={!Object.values(scopes).some(Boolean) ? "primary" : "ghost"} label={t("defaultScope")} onClick={() => onPreset(defaultBackupScopes())} /><Button size="sm" variant={isFullScope(scopes) ? "primary" : "ghost"} label={t("fullInstance")} onClick={() => onPreset(fullBackupScopes())} /></div><p className="muted scope-base">{t("scopeAlwaysIncluded")}</p><div className="scope-grid">{choices.map(([key, label]) => <CheckField key={key} label={t(label)} checked={scopes[key]} onChange={(checked) => onChange(key, checked)} />)}</div></section>
}
export function LabelWithTip({ label, tip }: { label: string; tip: string }) { const { t } = useI18n(); const [open, setOpen] = useState(false); return <div className="label-with-tip"><strong>{label}</strong><Tooltip content={tip} focusTrigger="always" isOpen={open} onOpenChange={setOpen}><button type="button" className="info-tip" aria-label={`${t("moreInformation")}: ${label}`} aria-expanded={open} onClick={() => setOpen((value) => !value)}><Info /></button></Tooltip></div> }
export async function detectArchiveKind(file: File): Promise<ArchiveKind> { const header = new TextDecoder().decode(await file.slice(0, 32).arrayBuffer()); if (header.startsWith("LBWSX01")) return "passphrase"; if (header.startsWith("LIGHTBWS-BACKUP-V1")) return "masterKey"; if (header.startsWith("LIGHTBWS-PLAIN-V2")) return "plaintext"; return "unknown" }
async function readMasterKey(file: File | null) { return file ? (await file.text()).trim() : "" }

function SettingsPage() {
  const { locale, setLocale, t } = useI18n(); const { mode, setMode } = useTheme()
  const modes: Array<{ value: ThemeMode; icon: ReactNode; label: MessageKey }> = [{ value: "system", icon: <Monitor />, label: "system" }, { value: "light", icon: <Sun />, label: "light" }, { value: "dark", icon: <Moon />, label: "dark" }]
  const themeLabels: Record<AstryxThemeName, string> = { neutral: t("themeNeutral"), stone: t("themeStone"), butter: t("themeButter"), matcha: t("themeMatcha"), chocolate: t("themeChocolate"), gothic: t("themeGothic"), y2k: t("themeY2k") }
  return <div className="page"><PageHeader title={t("settings")} description={t("appearanceText")} /><section className="settings-stack"><article className="panel settings-panel"><div><p className="eyebrow">ASTRYX</p><h2>{t("themeStyle")}</h2><p>{t("themeStyleText")}</p></div><ThemePicker labels={themeLabels} /></article><article className="panel settings-panel"><div><p className="eyebrow">MODE</p><h2>{t("themeMode")}</h2></div><div className="choice-grid">{modes.map((choice) => <button key={choice.value} className="choice-card" data-selected={mode === choice.value} onClick={() => setMode(choice.value)}>{choice.icon}<span>{t(choice.label)}</span>{mode === choice.value && <Check className="selection-check" />}</button>)}</div></article><article className="panel settings-panel"><div><p className="eyebrow">I18N</p><h2>{t("language")}</h2></div><div className="choice-grid two"><button className="choice-card" data-selected={locale === "zh-CN"} onClick={() => setLocale("zh-CN")}><Globe2 /><span>{t("chinese")}</span>{locale === "zh-CN" && <Check className="selection-check" />}</button><button className="choice-card" data-selected={locale === "en"} onClick={() => setLocale("en")}><Languages /><span>{t("english")}</span>{locale === "en" && <Check className="selection-check" />}</button></div></article></section></div>
}

function ListToolbar({ query, onQuery }: { query: string; onQuery: (value: string) => void }) { const { t } = useI18n(); return <div className="list-toolbar"><Search /><input aria-label={t("search")} placeholder={`${t("search")}…`} value={query} onChange={(event) => onQuery(event.target.value)} /></div> }

function DataPanel({ empty, onEmptyAction, children }: { empty: boolean; onEmptyAction?: () => void; children?: ReactNode }) {
  const { t } = useI18n(); return <section className="data-panel">{empty ? <div className="empty-state"><span><Boxes /></span><h2>{t("emptyTitle")}</h2><p>{t("emptyDescription")}</p>{onEmptyAction && <Button variant="secondary" icon={<Plus />} label={t("create")} onClick={onEmptyAction} />}</div> : <div className="table-scroll">{children}</div>}</section>
}

function Modal({ title, onClose, wide = false, children }: { title: string; onClose: () => void; wide?: boolean; children: ReactNode }) {
  useEffect(() => { const key = (event: KeyboardEvent) => { if (event.key === "Escape") onClose() }; addEventListener("keydown", key); return () => removeEventListener("keydown", key) }, [onClose])
  const { t } = useI18n()
  return <div className="modal-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose() }}><section className="modal" data-wide={wide} role="dialog" aria-modal="true" aria-labelledby="modal-title"><header><h2 id="modal-title">{title}</h2><IconButton variant="ghost" label={t("close")} icon={<X />} onClick={onClose} /></header>{children}</section></div>
}

function DialogActions({ onClose, busy, disabled }: { onClose: () => void; busy: boolean; disabled?: boolean }) { const { t } = useI18n(); return <div className="dialog-actions"><Button variant="ghost" label={t("cancel")} onClick={onClose} /><Button type="submit" variant="primary" label={t("save")} isLoading={busy} isDisabled={disabled} /></div> }

function NameDialog({ title, label, type = "text", initialValue = "", onClose, onSave }: { title: string; label: string; type?: string; initialValue?: string; onClose: () => void; onSave: (value: string) => Promise<void> }) { const [value, setValue] = useState(initialValue); const [busy, setBusy] = useState(false); async function submit(event: FormEvent) { event.preventDefault(); setBusy(true); await onSave(value); setBusy(false) } return <Modal title={title} onClose={onClose}><form className="form-stack" onSubmit={submit}><Field label={label} value={value} onChange={setValue} type={type} autoFocus /><DialogActions onClose={onClose} busy={busy} disabled={!value.trim() || (type === "password" && value.length < 8)} /></form></Modal> }

function Field({ label, value, onChange, type = "text", placeholder, autoComplete, autoFocus, disabled, hint }: { label: string; value: string; onChange: (value: string) => void; type?: string; placeholder?: string; autoComplete?: string; autoFocus?: boolean; disabled?: boolean; hint?: string }) { return <label className="field"><span>{label}</span><input {...{ type, value, placeholder, autoComplete, autoFocus, disabled }} onChange={(event) => onChange(event.target.value)} />{hint && <small>{hint}</small>}</label> }
function TextArea({ label, value, onChange }: { label: string; value: string; onChange: (value: string) => void }) { return <label className="field"><span>{label}</span><textarea value={value} onChange={(event) => onChange(event.target.value)} rows={3} /></label> }
function SelectField({ label, value, onChange, options, disabled }: { label: string; value: string; onChange: (value: string) => void; options: Array<{ value: string; label: string }>; disabled?: boolean }) { return <label className="field"><span>{label}</span><select value={value} disabled={disabled} onChange={(event) => onChange(event.target.value)}>{options.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}</select></label> }
function CheckField({ label, checked, onChange, disabled }: { label: string; checked: boolean; onChange: (value: boolean) => void; disabled?: boolean }) { return <label className="check-field"><input type="checkbox" checked={checked} disabled={disabled} onChange={(event) => onChange(event.target.checked)} /><span>{label}</span></label> }
function RowActions({ children }: { children: ReactNode }) { return <div className="row-actions">{children}</div> }
function StatusPill({ value }: { value: string }) { const { t } = useI18n(); const key = (["active", "disabled", "revoked", "succeeded", "failed", "running", "allowed", "denied", "changed", "readOnly", "readWrite"] as MessageKey[]).includes(value as MessageKey) ? value as MessageKey : "active"; return <span className="status-pill" data-status={value}><i />{t(key)}</span> }

function formatDate(value: number) { return new Intl.DateTimeFormat(document.documentElement.lang || "en", { dateStyle: "medium", timeStyle: "short" }).format(new Date(value * 1000)) }
function grantKey(bucket: GrantBucketKey, id: string) { return `${bucket}:${id}` }
function shortId(value: string | null) { return value ? value.slice(0, 8) : "—" }
function validPage(value: string): Page | null { return (["dashboard", "projects", "secrets", "machines", "users", "groups", "audit", "trash", "integrations", "backups", "transfer", "settings"] as Page[]).includes(value as Page) ? value as Page : null }
function availablePage(value: string, isAdmin: boolean): Page {
  const page = validPage(value) || "dashboard"
  return !isAdmin && adminPages.has(page) ? "dashboard" : page
}
function fileBase64(file: File): Promise<string> { return new Promise((resolve, reject) => { const reader = new FileReader(); reader.onerror = () => reject(reader.error); reader.onload = () => resolve(String(reader.result).split(",")[1] || ""); reader.readAsDataURL(file) }) }
