import { Banner } from "@astryxdesign/core/Banner"
import { Button } from "@astryxdesign/core/Button"
import { Card } from "@astryxdesign/core/Card"
import { IconButton } from "@astryxdesign/core/IconButton"
import {
  ArchiveRestore, Boxes, Check, ChevronRight, CircleGauge, CloudUpload, Copy, DatabaseBackup,
  ExternalLink, FileDown, FileUp, FolderKanban, Globe2, KeyRound, Languages, LogOut, Menu,
  Monitor, Moon, Network, Pencil, Plus, RefreshCw, Search, ServerCog, Settings, ShieldCheck,
  Sun, Trash2, UserCog, UsersRound, X,
} from "lucide-react"
import { useCallback, useEffect, useMemo, useState, type FormEvent, type ReactNode } from "react"

import { ApiError, api } from "./api"
import type {
  BackupConfig, BackupJob, BackupTarget, IssuedMachineAccount, Locale, MachineAccount, Overview,
  Project, Role, Secret, Session, ThemeMode, User,
} from "./types"
import { useI18n } from "../i18n/I18nProvider"
import type { MessageKey } from "../i18n/messages"
import { useTheme } from "../theme/ThemeProvider"

type Page = "dashboard" | "projects" | "secrets" | "machines" | "users" | "trash" | "integrations" | "backups" | "transfer" | "settings"
type Notice = { text: string; error?: boolean } | null

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
        <Button variant="ghost" icon={<Languages />} label={locale === "en" ? "简体中文" : "English"} onClick={() => setLocale(locale === "en" ? "zh-CN" : "en")} />
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
  const [page, setPage] = useState<Page>(() => validPage(location.hash.slice(1)) || "dashboard")
  const [mobileOpen, setMobileOpen] = useState(false)
  const [notice, setNotice] = useState<Notice>(null)
  const isAdmin = session.user.role === "admin"

  useEffect(() => {
    const change = () => setPage(validPage(location.hash.slice(1)) || "dashboard")
    addEventListener("hashchange", change)
    return () => removeEventListener("hashchange", change)
  }, [])
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
  const pageProps = { notify: (text: string, error = false) => setNotice({ text, error }) }

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
        {page === "projects" && <ProjectsPage {...pageProps} />}
        {page === "secrets" && <SecretsPage {...pageProps} />}
        {page === "machines" && isAdmin && <MachinesPage {...pageProps} />}
        {page === "users" && isAdmin && <UsersPage currentUser={session.user} {...pageProps} />}
        {page === "trash" && <TrashPage {...pageProps} />}
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

function ProjectsPage({ notify }: { notify: (text: string, error?: boolean) => void }) {
  const { t } = useI18n()
  const [items, setItems] = useState<Project[]>([])
  const [query, setQuery] = useState("")
  const [editing, setEditing] = useState<Project | "new" | null>(null)
  const load = useCallback(() => api.projects().then(setItems).catch(() => notify(t("genericError"), true)), [notify, t])
  useEffect(() => { void load() }, [load])
  const filtered = items.filter((item) => item.name.toLowerCase().includes(query.toLowerCase()))
  async function save(name: string) {
    try { editing === "new" ? await api.createProject(name) : editing && await api.updateProject(editing.id, name); setEditing(null); load() }
    catch { notify(t("genericError"), true) }
  }
  async function remove(item: Project) { try { await api.trashProject(item.id); load() } catch { notify(t("genericError"), true) } }
  return <div className="page"><PageHeader title={t("projects")} action={<Button variant="primary" icon={<Plus />} label={`${t("new")} ${t("project")}`} onClick={() => setEditing("new")} />} />
    <ListToolbar query={query} onQuery={setQuery} />
    <DataPanel empty={!filtered.length} onEmptyAction={() => setEditing("new")}>
      <table><thead><tr><th>{t("projectName")}</th><th>{t("lastUpdated")}</th><th className="action-column">{t("actions")}</th></tr></thead>
        <tbody>{filtered.map((item) => <tr key={item.id}><td><div className="title-cell"><span className="row-icon"><FolderKanban /></span><div><strong>{item.name}</strong><small>{item.sdkEncrypted ? t("encryptedSdk") : item.id}</small></div></div></td><td>{formatDate(item.updatedAt)}</td><td><RowActions><IconButton variant="ghost" label={t("edit")} icon={<Pencil />} onClick={() => setEditing(item)} /><IconButton variant="ghost" label={t("delete")} icon={<Trash2 />} onClick={() => void remove(item)} /></RowActions></td></tr>)}</tbody>
      </table>
    </DataPanel>
    {editing && <ProjectDialog item={editing === "new" ? null : editing} onClose={() => setEditing(null)} onSave={save} />}
  </div>
}

function ProjectDialog({ item, onClose, onSave }: { item: Project | null; onClose: () => void; onSave: (name: string) => Promise<void> }) {
  const { t } = useI18n(); const [name, setName] = useState(item?.name || ""); const [busy, setBusy] = useState(false)
  async function submit(event: FormEvent) { event.preventDefault(); if (!name.trim()) return; setBusy(true); await onSave(name); setBusy(false) }
  return <Modal title={`${item ? t("edit") : t("new")} ${t("project")}`} onClose={onClose}><form className="form-stack" onSubmit={submit}><Field label={t("projectName")} value={name} onChange={setName} autoFocus /><DialogActions onClose={onClose} busy={busy} disabled={!name.trim()} /></form></Modal>
}

function SecretsPage({ notify }: { notify: (text: string, error?: boolean) => void }) {
  const { t } = useI18n()
  const [items, setItems] = useState<Secret[]>([])
  const [projects, setProjects] = useState<Project[]>([])
  const [query, setQuery] = useState("")
  const [editing, setEditing] = useState<Secret | "new" | null>(null)
  const [revealed, setRevealed] = useState<Set<string>>(new Set())
  const load = useCallback(() => Promise.all([api.secrets(), api.projects()]).then(([secrets, projects]) => { setItems(secrets); setProjects(projects) }).catch(() => notify(t("genericError"), true)), [notify, t])
  useEffect(() => { void load() }, [load])
  const filtered = items.filter((item) => `${item.key} ${item.note}`.toLowerCase().includes(query.toLowerCase()))
  async function save(input: { key: string; value: string; note: string; projectId: string | null }) {
    try { editing === "new" ? await api.createSecret(input) : editing && await api.updateSecret(editing.id, input); setEditing(null); await load() }
    catch { notify(t("genericError"), true) }
  }
  async function remove(item: Secret) { try { await api.trashSecret(item.id); await load() } catch { notify(t("genericError"), true) } }
  async function copy(value: string) { await navigator.clipboard.writeText(value); notify(t("copied")) }
  return <div className="page"><PageHeader title={t("secrets")} action={<Button variant="primary" icon={<Plus />} label={`${t("new")} ${t("secret")}`} onClick={() => setEditing("new")} />} />
    <ListToolbar query={query} onQuery={setQuery} />
    <DataPanel empty={!filtered.length} onEmptyAction={() => setEditing("new")}>
      <table><thead><tr><th>{t("secretKey")}</th><th>{t("secretValue")}</th><th>{t("project")}</th><th>{t("lastUpdated")}</th><th className="action-column">{t("actions")}</th></tr></thead>
        <tbody>{filtered.map((item) => <tr key={item.id}><td><div className="title-cell"><span className="row-icon"><KeyRound /></span><div><strong>{item.key}</strong>{item.sdkEncrypted && <small>{t("encryptedSdk")}</small>}</div></div></td><td><code className="secret-value">{item.value === null ? "••••••••" : revealed.has(item.id) ? item.value : "••••••••••••"}</code></td><td>{projects.find((project) => project.id === item.projectId)?.name || t("noProject")}</td><td>{formatDate(item.updatedAt)}</td><td><RowActions>{item.value !== null && <IconButton variant="ghost" label={t("secretValue")} icon={<KeyRound />} onClick={() => setRevealed((current) => { const next = new Set(current); next.has(item.id) ? next.delete(item.id) : next.add(item.id); return next })} />}{item.value !== null && <IconButton variant="ghost" label={t("copy")} icon={<Copy />} onClick={() => void copy(item.value || "")} />}{!item.sdkEncrypted && <IconButton variant="ghost" label={t("edit")} icon={<Pencil />} onClick={() => setEditing(item)} />}<IconButton variant="ghost" label={t("delete")} icon={<Trash2 />} onClick={() => void remove(item)} /></RowActions></td></tr>)}</tbody>
      </table>
    </DataPanel>
    {editing && <SecretDialog item={editing === "new" ? null : editing} projects={projects} onClose={() => setEditing(null)} onSave={save} />}
  </div>
}

function SecretDialog({ item, projects, onClose, onSave }: { item: Secret | null; projects: Project[]; onClose: () => void; onSave: (input: { key: string; value: string; note: string; projectId: string | null }) => Promise<void> }) {
  const { t } = useI18n(); const [key, setKey] = useState(item?.key || ""); const [value, setValue] = useState(item?.value || ""); const [note, setNote] = useState(item?.note || ""); const [projectId, setProjectId] = useState(item?.projectId || ""); const [busy, setBusy] = useState(false)
  async function submit(event: FormEvent) { event.preventDefault(); if (!key.trim()) return; setBusy(true); await onSave({ key, value, note, projectId: projectId || null }); setBusy(false) }
  return <Modal title={`${item ? t("edit") : t("new")} ${t("secret")}`} onClose={onClose} wide><form className="form-stack" onSubmit={submit}><div className="form-grid"><Field label={t("secretKey")} value={key} onChange={setKey} autoFocus /><Field label={t("secretValue")} value={value} onChange={setValue} /></div><TextArea label={t("note")} value={note} onChange={setNote} /><SelectField label={t("project")} value={projectId} onChange={setProjectId} options={[{ value: "", label: t("noProject") }, ...projects.map((project) => ({ value: project.id, label: project.name }))]} /><DialogActions onClose={onClose} busy={busy} disabled={!key.trim()} /></form></Modal>
}

function MachinesPage({ notify }: { notify: (text: string, error?: boolean) => void }) {
  const { t } = useI18n(); const [items, setItems] = useState<MachineAccount[]>([]); const [creating, setCreating] = useState(false); const [issued, setIssued] = useState<IssuedMachineAccount | null>(null)
  const load = useCallback(() => api.machines().then(setItems).catch(() => notify(t("genericError"), true)), [notify, t]); useEffect(() => { void load() }, [load])
  async function create(name: string) { try { const value = await api.createMachine(name); setCreating(false); setIssued(value); load() } catch { notify(t("genericError"), true) } }
  async function toggle(item: MachineAccount) { try { await api.setMachineRevoked(item.id, !item.revokedAt); load() } catch { notify(t("genericError"), true) } }
  async function remove(item: MachineAccount) { try { await api.deleteMachine(item.id); load() } catch { notify(t("genericError"), true) } }
  return <div className="page"><PageHeader title={t("machines")} action={<Button variant="primary" icon={<Plus />} label={`${t("new")} ${t("machineAccount")}`} onClick={() => setCreating(true)} />} /><DataPanel empty={!items.length} onEmptyAction={() => setCreating(true)}><table><thead><tr><th>{t("machineName")}</th><th>{t("clientId")}</th><th>{t("lastUsed")}</th><th>{t("status")}</th><th className="action-column">{t("actions")}</th></tr></thead><tbody>{items.map((item) => <tr key={item.id}><td><div className="title-cell"><span className="row-icon"><ServerCog /></span><strong>{item.name}</strong></div></td><td><code>{item.clientId}</code></td><td>{item.lastUsedAt ? formatDate(item.lastUsedAt) : t("never")}</td><td><StatusPill value={item.revokedAt ? "revoked" : "active"} /></td><td><RowActions><Button size="sm" variant="ghost" label={item.revokedAt ? t("enable") : t("revoke")} onClick={() => void toggle(item)} /><IconButton variant="ghost" label={t("delete")} icon={<Trash2 />} onClick={() => void remove(item)} /></RowActions></td></tr>)}</tbody></table></DataPanel>{creating && <NameDialog title={`${t("new")} ${t("machineAccount")}`} label={t("machineName")} onClose={() => setCreating(false)} onSave={create} />}{issued && <TokenDialog item={issued} onClose={() => setIssued(null)} notify={notify} />}</div>
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

function TrashPage({ notify }: { notify: (text: string, error?: boolean) => void }) {
  const { t } = useI18n(); const [projects, setProjects] = useState<Project[]>([]); const [secrets, setSecrets] = useState<Secret[]>([])
  const load = useCallback(() => Promise.all([api.projects(true), api.secrets(true)]).then(([projects, secrets]) => { setProjects(projects); setSecrets(secrets) }).catch(() => notify(t("genericError"), true)), [notify, t]); useEffect(() => { void load() }, [load])
  async function act(kind: "project" | "secret", id: string, purge: boolean) { if (purge && !confirm(t("confirmDelete"))) return; try { if (kind === "project") purge ? await api.purgeProject(id) : await api.restoreProject(id); else purge ? await api.purgeSecret(id) : await api.restoreSecret(id); await load() } catch { notify(t("genericError"), true) } }
  const empty = !projects.length && !secrets.length
  return <div className="page"><PageHeader title={t("trash")} description={t("dangerousAction")} /><DataPanel empty={empty}><div className="trash-groups">{projects.length > 0 && <section><h2>{t("projects")}</h2>{projects.map((item) => <TrashRow key={item.id} name={item.name} onRestore={() => void act("project", item.id, false)} onPurge={() => void act("project", item.id, true)} />)}</section>}{secrets.length > 0 && <section><h2>{t("secrets")}</h2>{secrets.map((item) => <TrashRow key={item.id} name={item.key} onRestore={() => void act("secret", item.id, false)} onPurge={() => void act("secret", item.id, true)} />)}</section>}</div></DataPanel></div>
}

function TrashRow({ name, onRestore, onPurge }: { name: string; onRestore: () => void; onPurge: () => void }) { const { t } = useI18n(); return <div className="trash-row"><strong>{name}</strong><RowActions><Button variant="ghost" size="sm" icon={<RefreshCw />} label={t("restore")} onClick={onRestore} /><Button variant="ghost" size="sm" icon={<Trash2 />} label={t("purge")} onClick={onPurge} /></RowActions></div> }

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
  const { t } = useI18n(); const [targets, setTargets] = useState<BackupTarget[]>([]); const [jobs, setJobs] = useState<BackupJob[]>([]); const [editing, setEditing] = useState<BackupTarget | "new" | null>(null); const [busyId, setBusyId] = useState<string | null>(null)
  const load = useCallback(() => Promise.all([api.backupTargets(), api.backupJobs()]).then(([targets, jobs]) => { setTargets(targets); setJobs(jobs) }).catch(() => notify(t("genericError"), true)), [notify, t]); useEffect(() => { void load() }, [load])
  async function save(input: unknown) { try { editing === "new" ? await api.createBackupTarget(input) : editing && await api.updateBackupTarget(editing.id, input); setEditing(null); await load() } catch { notify(t("genericError"), true) } }
  async function run(target: BackupTarget) { setBusyId(target.id); try { const job = await api.runBackup(target.id); notify(job.status === "succeeded" ? t("succeeded") : t("failed"), job.status !== "succeeded"); await load() } catch { notify(t("genericError"), true) } finally { setBusyId(null) } }
  async function test(target: BackupTarget) { setBusyId(target.id); try { await api.testBackupTarget(target.id); notify(t("succeeded")) } catch { notify(t("genericError"), true) } finally { setBusyId(null) } }
  async function remove(target: BackupTarget) { if (!confirm(t("confirmDelete"))) return; try { await api.deleteBackupTarget(target.id); await load() } catch { notify(t("genericError"), true) } }
  return <div className="page"><PageHeader title={t("backups")} description={t("backupIntro")} action={<Button variant="primary" icon={<Plus />} label={t("newBackupTarget")} onClick={() => setEditing("new")} />} /><section className="backup-target-grid">{targets.map((target) => <article className="panel backup-card" key={target.id}><div className="panel-heading"><div className="title-cell"><span className="row-icon"><CloudUpload /></span><div><h2>{target.displayName}</h2><small>{target.config.kind}</small></div></div><StatusPill value={target.enabled ? target.lastStatus || "active" : "disabled"} /></div><dl><div><dt>{t("endpoint")}</dt><dd>{target.config.settings.endpoint}</dd></div><div><dt>{t("nextRun")}</dt><dd>{target.nextRunAt ? formatDate(target.nextRunAt) : "—"}</dd></div><div><dt>{t("lastRun")}</dt><dd>{target.lastRunAt ? formatDate(target.lastRunAt) : t("never")}</dd></div></dl><div className="card-actions"><Button size="sm" variant="secondary" label={t("runNow")} isLoading={busyId === target.id} onClick={() => void run(target)} /><Button size="sm" variant="ghost" label={t("testTarget")} onClick={() => void test(target)} /><IconButton variant="ghost" label={t("edit")} icon={<Pencil />} onClick={() => setEditing(target)} /><IconButton variant="ghost" label={t("delete")} icon={<Trash2 />} onClick={() => void remove(target)} /></div></article>)}</section>{!targets.length && <DataPanel empty onEmptyAction={() => setEditing("new")} />}
    <section className="panel history-panel"><h2>{t("backupHistory")}</h2>{jobs.length ? <table><thead><tr><th>{t("backupTarget")}</th><th>{t("status")}</th><th>{t("lastRun")}</th><th>{t("actions")}</th></tr></thead><tbody>{jobs.map((job) => <tr key={job.id}><td><code>{job.objectKey}</code></td><td><StatusPill value={job.status} /></td><td>{formatDate(job.createdAt)}</td><td>{job.triggerKind === "manual" ? t("manual") : t("scheduled")}</td></tr>)}</tbody></table> : <p className="muted">{t("emptyBackupHistory")}</p>}</section>{editing && <BackupDialog item={editing === "new" ? null : editing} onClose={() => setEditing(null)} onSave={save} />}</div>
}

interface BackupForm { kind: "S3" | "WEBDAV"; displayName: string; endpoint: string; region: string; bucket: string; prefix: string; pathStyle: boolean; accessKeyId: string; secretAccessKey: string; sessionToken: string; username: string; password: string; enabled: boolean; scheduleEnabled: boolean; intervalHours: string }
function BackupDialog({ item, onClose, onSave }: { item: BackupTarget | null; onClose: () => void; onSave: (input: unknown) => Promise<void> }) {
  const { t } = useI18n(); const current = item?.config; const [form, setForm] = useState<BackupForm>({ kind: current?.kind || "S3", displayName: item?.displayName || "", endpoint: current?.settings.endpoint || "", region: current?.kind === "S3" ? current.settings.region : "us-east-1", bucket: current?.kind === "S3" ? current.settings.bucket : "", prefix: current?.settings.prefix || "", pathStyle: current?.kind === "S3" ? current.settings.pathStyle : true, accessKeyId: "", secretAccessKey: "", sessionToken: "", username: "", password: "", enabled: item?.enabled ?? true, scheduleEnabled: item?.scheduleEnabled ?? false, intervalHours: String(item?.intervalHours || 24) }); const [busy, setBusy] = useState(false)
  function set<K extends keyof BackupForm>(key: K, value: BackupForm[K]) { setForm((current) => ({ ...current, [key]: value })) }
  async function submit(event: FormEvent) { event.preventDefault(); setBusy(true); const config: BackupConfig = form.kind === "S3" ? { kind: "S3", settings: { endpoint: form.endpoint, region: form.region, bucket: form.bucket, prefix: form.prefix, pathStyle: form.pathStyle } } : { kind: "WEBDAV", settings: { endpoint: form.endpoint, prefix: form.prefix } }; const credentials = form.kind === "S3" ? { kind: "S3", values: { accessKeyId: form.accessKeyId, secretAccessKey: form.secretAccessKey, sessionToken: form.sessionToken || null } } : { kind: "WEBDAV", values: { username: form.username, password: form.password } }; await onSave({ displayName: form.displayName, config, credentials: item && !form.accessKeyId && !form.username ? null : credentials, enabled: form.enabled, scheduleEnabled: form.scheduleEnabled, intervalHours: Number(form.intervalHours) }); setBusy(false) }
  const credentialReady = item || (form.kind === "S3" ? form.accessKeyId && form.secretAccessKey : form.username && form.password)
  return <Modal title={item ? t("editBackupTarget") : t("newBackupTarget")} onClose={onClose} wide><form className="form-stack" onSubmit={submit}><div className="form-grid"><Field label={t("targetName")} value={form.displayName} onChange={(value) => set("displayName", value)} autoFocus /><SelectField label={t("targetType")} value={form.kind} disabled={Boolean(item)} onChange={(value) => set("kind", value as BackupForm["kind"])} options={[{ value: "S3", label: "S3" }, { value: "WEBDAV", label: "WebDAV" }]} /></div><Field label={t("endpoint")} value={form.endpoint} onChange={(value) => set("endpoint", value)} placeholder="https://…" />{form.kind === "S3" ? <><div className="form-grid"><Field label={t("region")} value={form.region} onChange={(value) => set("region", value)} /><Field label={t("bucket")} value={form.bucket} onChange={(value) => set("bucket", value)} /></div><Field label={t("accessKeyId")} value={form.accessKeyId} onChange={(value) => set("accessKeyId", value)} placeholder={item ? "••••••••" : ""} /><Field label={t("secretAccessKey")} value={form.secretAccessKey} onChange={(value) => set("secretAccessKey", value)} type="password" placeholder={item ? "••••••••" : ""} /><Field label={t("sessionToken")} value={form.sessionToken} onChange={(value) => set("sessionToken", value)} type="password" /><CheckField label={t("pathStyle")} checked={form.pathStyle} onChange={(value) => set("pathStyle", value)} /></> : <><Field label={t("webdavUsername")} value={form.username} onChange={(value) => set("username", value)} placeholder={item ? "••••••••" : ""} /><Field label={t("webdavPassword")} value={form.password} onChange={(value) => set("password", value)} type="password" placeholder={item ? "••••••••" : ""} /></>}<Field label={t("prefix")} value={form.prefix} onChange={(value) => set("prefix", value)} /><div className="form-grid"><CheckField label={t("enable")} checked={form.enabled} onChange={(value) => set("enabled", value)} /><CheckField label={t("scheduled")} checked={form.scheduleEnabled} onChange={(value) => set("scheduleEnabled", value)} /></div>{form.scheduleEnabled && <Field label={t("intervalHours")} value={form.intervalHours} onChange={(value) => set("intervalHours", value)} type="number" />}<DialogActions onClose={onClose} busy={busy} disabled={!form.displayName || !form.endpoint || !credentialReady} /></form></Modal>
}

function TransferPage({ notify }: { notify: (text: string, error?: boolean) => void }) {
  const { t } = useI18n(); const [exportPassphrase, setExportPassphrase] = useState(""); const [importPassphrase, setImportPassphrase] = useState(""); const [file, setFile] = useState<File | null>(null); const [busy, setBusy] = useState<"export" | "import" | null>(null)
  async function exportData(event: FormEvent) { event.preventDefault(); setBusy("export"); try { const blob = await api.export(exportPassphrase); const url = URL.createObjectURL(blob); const anchor = document.createElement("a"); anchor.href = url; anchor.download = `lightbws-${new Date().toISOString().slice(0, 10)}.lightbws`; anchor.click(); URL.revokeObjectURL(url); notify(t("exportComplete")) } catch { notify(t("genericError"), true) } finally { setBusy(null) } }
  async function importData(event: FormEvent) { event.preventDefault(); if (!file) return; setBusy("import"); try { const dataBase64 = await fileBase64(file); await api.import(importPassphrase, dataBase64); notify(t("importComplete")); setFile(null) } catch { notify(t("genericError"), true) } finally { setBusy(null) } }
  return <div className="page"><PageHeader title={t("transfer")} /><section className="transfer-grid"><article className="panel transfer-card"><span className="panel-icon"><FileDown /></span><h2>{t("exportTitle")}</h2><p>{t("exportText")}</p><form className="form-stack" onSubmit={exportData}><Field label={t("passphrase")} value={exportPassphrase} onChange={setExportPassphrase} type="password" hint={t("passphraseHint")} /><Button type="submit" variant="primary" icon={<FileDown />} label={t("downloadExport")} isLoading={busy === "export"} isDisabled={exportPassphrase.length < 12} /></form></article><article className="panel transfer-card"><span className="panel-icon"><FileUp /></span><h2>{t("importTitle")}</h2><p>{t("importText")}</p><form className="form-stack" onSubmit={importData}><label className="field"><span>{t("chooseFile")}</span><input type="file" accept=".lightbws,application/vnd.lightbws.backup" onChange={(event) => setFile(event.target.files?.[0] || null)} /></label><Field label={t("passphrase")} value={importPassphrase} onChange={setImportPassphrase} type="password" hint={t("passphraseHint")} /><Button type="submit" variant="primary" icon={<FileUp />} label={t("importFile")} isLoading={busy === "import"} isDisabled={!file || importPassphrase.length < 12} /></form></article></section></div>
}

function SettingsPage() {
  const { locale, setLocale, t } = useI18n(); const { mode, setMode } = useTheme()
  const modes: Array<{ value: ThemeMode; icon: ReactNode; label: MessageKey }> = [{ value: "system", icon: <Monitor />, label: "system" }, { value: "light", icon: <Sun />, label: "light" }, { value: "dark", icon: <Moon />, label: "dark" }]
  return <div className="page"><PageHeader title={t("settings")} description={t("appearanceText")} /><section className="settings-stack"><article className="panel settings-panel"><div><p className="eyebrow">ASTRYX</p><h2>{t("themeMode")}</h2></div><div className="choice-grid">{modes.map((choice) => <button key={choice.value} className="choice-card" data-selected={mode === choice.value} onClick={() => setMode(choice.value)}>{choice.icon}<span>{t(choice.label)}</span>{mode === choice.value && <Check />}</button>)}</div></article><article className="panel settings-panel"><div><p className="eyebrow">I18N</p><h2>{t("language")}</h2></div><div className="choice-grid two"><button className="choice-card" data-selected={locale === "zh-CN"} onClick={() => setLocale("zh-CN")}><Globe2 /><span>{t("chinese")}</span>{locale === "zh-CN" && <Check />}</button><button className="choice-card" data-selected={locale === "en"} onClick={() => setLocale("en")}><Languages /><span>{t("english")}</span>{locale === "en" && <Check />}</button></div></article></section></div>
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

function NameDialog({ title, label, type = "text", onClose, onSave }: { title: string; label: string; type?: string; onClose: () => void; onSave: (value: string) => Promise<void> }) { const [value, setValue] = useState(""); const [busy, setBusy] = useState(false); async function submit(event: FormEvent) { event.preventDefault(); setBusy(true); await onSave(value); setBusy(false) } return <Modal title={title} onClose={onClose}><form className="form-stack" onSubmit={submit}><Field label={label} value={value} onChange={setValue} type={type} autoFocus /><DialogActions onClose={onClose} busy={busy} disabled={!value.trim() || (type === "password" && value.length < 8)} /></form></Modal> }

function Field({ label, value, onChange, type = "text", placeholder, autoComplete, autoFocus, disabled, hint }: { label: string; value: string; onChange: (value: string) => void; type?: string; placeholder?: string; autoComplete?: string; autoFocus?: boolean; disabled?: boolean; hint?: string }) { return <label className="field"><span>{label}</span><input {...{ type, value, placeholder, autoComplete, autoFocus, disabled }} onChange={(event) => onChange(event.target.value)} />{hint && <small>{hint}</small>}</label> }
function TextArea({ label, value, onChange }: { label: string; value: string; onChange: (value: string) => void }) { return <label className="field"><span>{label}</span><textarea value={value} onChange={(event) => onChange(event.target.value)} rows={3} /></label> }
function SelectField({ label, value, onChange, options, disabled }: { label: string; value: string; onChange: (value: string) => void; options: Array<{ value: string; label: string }>; disabled?: boolean }) { return <label className="field"><span>{label}</span><select value={value} disabled={disabled} onChange={(event) => onChange(event.target.value)}>{options.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}</select></label> }
function CheckField({ label, checked, onChange, disabled }: { label: string; checked: boolean; onChange: (value: boolean) => void; disabled?: boolean }) { return <label className="check-field"><input type="checkbox" checked={checked} disabled={disabled} onChange={(event) => onChange(event.target.checked)} /><span>{label}</span></label> }
function RowActions({ children }: { children: ReactNode }) { return <div className="row-actions">{children}</div> }
function StatusPill({ value }: { value: string }) { const { t } = useI18n(); const key = (["active", "disabled", "revoked", "succeeded", "failed", "running"] as MessageKey[]).includes(value as MessageKey) ? value as MessageKey : "active"; return <span className="status-pill" data-status={value}><i />{t(key)}</span> }

function formatDate(value: number) { return new Intl.DateTimeFormat(document.documentElement.lang || "en", { dateStyle: "medium", timeStyle: "short" }).format(new Date(value * 1000)) }
function validPage(value: string): Page | null { return (["dashboard", "projects", "secrets", "machines", "users", "trash", "integrations", "backups", "transfer", "settings"] as Page[]).includes(value as Page) ? value as Page : null }
function fileBase64(file: File): Promise<string> { return new Promise((resolve, reject) => { const reader = new FileReader(); reader.onerror = () => reject(reader.error); reader.onload = () => resolve(String(reader.result).split(",")[1] || ""); reader.readAsDataURL(file) }) }
