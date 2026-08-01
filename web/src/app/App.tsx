import { Banner } from "@astryxdesign/core/Banner"
import { AlertDialog } from "@astryxdesign/core/AlertDialog"
import { Button } from "@astryxdesign/core/Button"
import { Card } from "@astryxdesign/core/Card"
import { IconButton as BaseIconButton } from "@astryxdesign/core/IconButton"
import { Skeleton } from "@astryxdesign/core/Skeleton"
import { Tooltip } from "@astryxdesign/core/Tooltip"
import { SpotlightTour, useSpotlight, type SpotlightStep } from "react-tourlight"
import {
  ArchiveRestore, BookOpen, Boxes, Check, ChevronRight, CircleGauge, Clock3, CloudUpload, Copy, DatabaseBackup,
  Eye, EyeOff, ExternalLink, FileDown, FileUp, FolderKanban, Globe2, Info, KeyRound, Languages, LogOut, Menu,
  Monitor, Moon, Network, Pencil, Plus, RefreshCw, ScrollText, Search, ServerCog, Settings, ShieldCheck,
  Sun, Trash2, UserCog, UsersRound, X,
} from "lucide-react"
import { useCallback, useEffect, useId, useMemo, useRef, useState, type ComponentProps, type FormEvent, type ReactNode } from "react"

import { ApiError, api } from "./api"
import type {
  AccessPolicy, AuditEvent, AuditSettings, BackupCapabilities, BackupConfig, BackupEncryption, BackupJob, BackupScopes, BackupTarget, GrantInput, Group,
  IssuedMachineAccessToken, IssuedMachineAccount, Locale, MachineAccessToken, MachineAccount, NamedGrant, Overview, Project, Role,
  Secret, Session, ThemeMode, User,
} from "./types"
import { useI18n } from "../i18n/I18nProvider"
import type { MessageKey } from "../i18n/messages"
import type { AstryxThemeName } from "../theme/astryxThemes"
import { ThemePicker } from "../theme/ThemePicker"
import { useTheme } from "../theme/ThemeProvider"

type Page = "dashboard" | "projects" | "secrets" | "machines" | "users" | "groups" | "audit" | "trash" | "integrations" | "help" | "backups" | "transfer" | "settings"
type Notice = { text: string; error?: boolean } | null
const adminPages = new Set<Page>(["machines", "users", "groups", "audit", "backups", "transfer"])
type AccessResource = { kind: "project" | "secret" | "machine"; id: string; name: string }
type GrantBucketKey = "users" | "groups" | "machines" | "projects"
type GrantLevel = "none" | "read" | "write"
type GrantBuckets<T> = Record<GrantBucketKey, T[]>
type AccessSection = { key: GrantBucketKey; label: string; items: Array<{ id: string; name: string; detail?: string }> }

type IconButtonProps = ComponentProps<typeof BaseIconButton>
function IconButton({ label, tooltip = label, ...props }: IconButtonProps) {
  return <BaseIconButton {...props} label={label} tooltip={tooltip} />
}

export function App() {
  const [session, setSession] = useState<Session | null>(null)
  const [loading, setLoading] = useState(true)
  const postLoginRoute = useRef(location.pathname === "/login" ? "/" : `${location.pathname}${location.search}${location.hash}`)

  useEffect(() => {
    api.session()
      .then((current) => {
        setSession(current)
        if (location.pathname === "/login") history.replaceState(null, "", "/")
      })
      .catch(() => {
        setSession(null)
        if (location.pathname !== "/login") history.replaceState(null, "", "/login")
      })
      .finally(() => setLoading(false))
  }, [])

  function authenticated(current: Session) {
    history.replaceState(null, "", postLoginRoute.current)
    setSession(current)
  }

  function signedOut() {
    postLoginRoute.current = `${location.pathname}${location.search}${location.hash}`
    history.replaceState(null, "", "/login")
    setSession(null)
  }

  if (loading) return <LoadingScreen />
  if (!session) return <LoginPage onAuthenticated={authenticated} />
  return <Workspace session={session} onSignedOut={signedOut} />
}

function LoadingScreen() {
  const { t } = useI18n()
  return <main className="loading-screen" role="status" aria-label={t("loading")}><div className="boot-skeleton"><Skeleton width={52} height={52} radius={4} /><Skeleton width={160} height={22} index={1} /><Skeleton width={240} height={14} index={2} /></div></main>
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
  const mobileMenuButton = useRef<HTMLButtonElement>(null)
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
  const navigate = useCallback((next: Page) => {
    location.hash = next
    setPage(next)
    setMobileOpen(false)
  }, [])
  const closeMobileNavigation = useCallback(() => {
    setMobileOpen(false)
    mobileMenuButton.current?.focus()
  }, [])
  async function logout() {
    await api.logout().catch(() => undefined)
    onSignedOut()
  }
  const pageProps = { notify }

  return (
    <div className="workspace">
      <WorkspaceTour onNavigate={navigate} />
      <aside id="workspace-navigation" className="sidebar" data-open={mobileOpen}>
        <div className="sidebar-head"><Brand /><IconButton className="mobile-only" variant="ghost" label={t("mobileClose")} icon={<X />} onClick={closeMobileNavigation} /></div>
        <nav aria-label={t("menu")}>
          <NavSectionLabel label={t("navWorkspace")} />
          <NavItem page="dashboard" current={page} icon={<CircleGauge />} label={t("dashboard")} onClick={navigate} />
          <NavItem page="projects" current={page} icon={<FolderKanban />} label={t("projects")} onClick={navigate} />
          <NavItem page="secrets" current={page} icon={<KeyRound />} label={t("secrets")} onClick={navigate} />
          <NavItem page="trash" current={page} icon={<Trash2 />} label={t("trash")} onClick={navigate} />
          {isAdmin && <NavSectionLabel label={t("navAccess")} />}
          {isAdmin && <NavItem page="machines" current={page} icon={<ServerCog />} label={t("machines")} onClick={navigate} />}
          {isAdmin && <NavItem page="users" current={page} icon={<UsersRound />} label={t("users")} onClick={navigate} />}
          {isAdmin && <NavItem page="groups" current={page} icon={<Boxes />} label={t("groups")} onClick={navigate} />}
          {isAdmin && <NavItem page="audit" current={page} icon={<ShieldCheck />} label={t("auditLog")} onClick={navigate} />}
          <NavSectionLabel label={t("navSupport")} />
          <NavItem page="integrations" current={page} icon={<Network />} label={t("integrations")} onClick={navigate} />
          <NavItem page="help" current={page} icon={<BookOpen />} label={t("help")} onClick={navigate} />
          <NavSectionLabel label={t("navOperations")} />
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
      {mobileOpen && <button className="sidebar-scrim" aria-label={t("mobileClose")} onClick={closeMobileNavigation} />}
      <main className="main-canvas">
        <div className="mobile-bar"><IconButton ref={mobileMenuButton} variant="ghost" label={t("menu")} icon={<Menu />} aria-controls="workspace-navigation" aria-expanded={mobileOpen} onClick={() => setMobileOpen(true)} /><Brand compact /></div>
        {page === "dashboard" && <DashboardPage onNavigate={navigate} {...pageProps} />}
        {page === "projects" && <ProjectsPage isAdmin={isAdmin} {...pageProps} />}
        {page === "secrets" && <SecretsPage isAdmin={isAdmin} {...pageProps} />}
        {page === "machines" && isAdmin && <MachinesPage {...pageProps} />}
        {page === "users" && isAdmin && <UsersPage currentUser={session.user} {...pageProps} />}
        {page === "groups" && isAdmin && <GroupsPage onNavigate={navigate} {...pageProps} />}
        {page === "audit" && isAdmin && <AuditPage {...pageProps} />}
        {page === "trash" && <TrashPage isAdmin={isAdmin} {...pageProps} />}
        {page === "integrations" && <IntegrationsPage {...pageProps} />}
        {page === "help" && <HelpPage {...pageProps} />}
        {page === "backups" && isAdmin && <BackupsPage {...pageProps} />}
        {page === "transfer" && isAdmin && <TransferPage {...pageProps} />}
        {page === "settings" && <SettingsPage />}
      </main>
      {notice && <div className="toast" data-error={notice.error || undefined} role={notice.error ? "alert" : "status"}>{notice.error ? <X /> : <Check />}<span>{notice.text}</span></div>}
    </div>
  )
}

function WorkspaceTour({ onNavigate }: { onNavigate: (page: Page) => void }) {
  const { t } = useI18n()
  const finish = useCallback(() => {
    try { localStorage.setItem("lightbws-onboarding-v1", "done") } catch { /* storage may be blocked */ }
    onNavigate("dashboard")
  }, [onNavigate])
  const steps = useMemo<SpotlightStep[]>(() => [
    { target: "#tour-dashboard-header", route: "#dashboard", title: t("tourWelcomeTitle"), content: t("tourWelcomeText"), placement: "bottom" },
    { target: "#tour-service-endpoint", route: "#dashboard", title: t("tourEndpointTitle"), content: t("tourEndpointText"), placement: "bottom" },
    { target: "#tour-new-secret", route: "#secrets", title: t("tourSecretTitle"), content: t("tourSecretText"), placement: "bottom" },
    { target: "#tour-new-machine", route: "#machines", title: t("tourMachineTitle"), content: t("tourMachineText"), placement: "bottom" },
    { target: "#tour-new-user", route: "#users", title: t("tourUsersTitle"), content: t("tourUsersText"), placement: "bottom" },
  ], [t])
  return <SpotlightTour id="workspace-onboarding" onComplete={finish} onSkip={finish} steps={steps} />
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

function NavSectionLabel({ label }: { label: string }) {
  return <p className="nav-section-label">{label}</p>
}

function PageHeader({ title, description, action }: { title: string; description?: string; action?: ReactNode }) {
  return <header className="page-header"><div><h1>{title}</h1>{description && <p>{description}</p>}</div>{action}</header>
}

type PageSkeletonVariant = "table" | "dashboard" | "cards"
const skeletonRows = ["alpha", "beta", "gamma", "delta"] as const

function PageSkeleton({ variant = "table" }: { variant?: PageSkeletonVariant }) {
  const { t } = useI18n()
  if (variant === "dashboard") {
    return <div className="page-skeleton" role="status" aria-label={t("loading")} data-testid="page-skeleton"><div className="skeleton-metrics">{skeletonRows.slice(0, 3).map((key, index) => <Skeleton key={key} width="100%" height={104} index={index} />)}</div><div className="skeleton-cards">{skeletonRows.slice(0, 2).map((key, index) => <Skeleton key={key} width="100%" height={210} index={index + 3} />)}</div></div>
  }
  if (variant === "cards") {
    return <div className="page-skeleton" role="status" aria-label={t("loading")} data-testid="page-skeleton"><div className="skeleton-cards">{skeletonRows.map((key, index) => <Skeleton key={key} width="100%" height={220} index={index} />)}</div></div>
  }
  return <div className="page-skeleton" role="status" aria-label={t("loading")} data-testid="page-skeleton"><Skeleton width="100%" height={44} radius={2} /><div className="skeleton-table"><Skeleton width="100%" height={46} radius={1} index={1} />{skeletonRows.map((key, index) => <div className="skeleton-row" key={key}><Skeleton width="34%" height={18} index={index + 2} /><Skeleton width="18%" height={18} index={index + 3} /><Skeleton width="24%" height={18} index={index + 4} /></div>)}</div></div>
}

function DashboardPage({ notify, onNavigate }: { notify: (text: string, error?: boolean) => void; onNavigate: (page: Page) => void }) {
  const { t } = useI18n()
  const { start } = useSpotlight()
  const [overview, setOverview] = useState<Overview | null>(null)
  const [loading, setLoading] = useState(true)
  const started = useRef(false)
  useEffect(() => { api.overview().then(setOverview).catch(() => notify(t("genericError"), true)).finally(() => setLoading(false)) }, [notify, t])
  const needsOnboarding = overview?.secrets === 0
  useEffect(() => {
    if (!needsOnboarding || started.current) return
    try { if (localStorage.getItem("lightbws-onboarding-v1") === "done") return } catch { /* storage may be blocked */ }
    started.current = true
    const timer = setTimeout(() => start("workspace-onboarding"), 250)
    return () => clearTimeout(timer)
  }, [needsOnboarding, start])
  return <div className="page"><div id="tour-dashboard-header"><PageHeader title={t("dashboard")} description={t("appTagline")} /></div>
    {loading ? <PageSkeleton variant="dashboard" /> : <>{needsOnboarding && <section className="getting-started" id="tour-getting-started">
      <div className="getting-started-copy" id="tour-getting-started-copy"><span className="step-number">01</span><div><h2>{t("gettingStartedTitle")}</h2><p>{t("gettingStartedText")}</p></div></div>
      <div className="getting-started-actions"><Button variant="primary" icon={<KeyRound />} label={t("createFirstSecret")} onClick={() => onNavigate("secrets")} /><Button variant="ghost" label={t("startTour")} onClick={() => start("workspace-onboarding")} /></div>
      <ServiceEndpoint notify={notify} id="tour-service-endpoint" />
      <ol className="quick-path"><li><strong>{t("quickSecretTitle")}</strong><span>{t("quickSecretText")}</span></li><li><strong>{t("quickProjectTitle")}</strong><span>{t("quickProjectText")}</span></li><li><strong>{t("quickAccessTitle")}</strong><span>{t("quickAccessText")}</span></li></ol>
    </section>}
    <section className="metric-grid">
      <Metric icon={<FolderKanban />} label={t("projectsCount")} value={overview?.projects} />
      <Metric icon={<KeyRound />} label={t("secretsCount")} value={overview?.secrets} />
      <Metric icon={<Trash2 />} label={t("trashCount")} value={overview?.trash} />
    </section>
    <section className="dashboard-grid">
      <article className="panel callout-panel"><span className="panel-icon"><ShieldCheck /></span><div><h2>{t("securityBoundary")}</h2><p>{t("securityBoundaryText")}</p></div></article>
      <article className="panel"><div className="panel-heading"><div><p className="eyebrow">SYSTEM</p><h2>{t("recentActivity")}</h2></div><span className="status-dot" /></div><p>{t("healthReady")}</p><code>GET /health · 200 OK</code></article>
    </section>
    </>}
  </div>
}

function Metric({ icon, label, value }: { icon: ReactNode; label: string; value?: number }) {
  return <article className="metric"><span>{icon}</span><div><strong>{value ?? "—"}</strong><small>{label}</small></div></article>
}

function ProjectsPage({ isAdmin, notify }: { isAdmin: boolean; notify: (text: string, error?: boolean) => void }) {
  const { t } = useI18n()
  const [items, setItems] = useState<Project[]>([])
  const [query, setQuery] = useState("")
  const [loading, setLoading] = useState(true)
  const [editing, setEditing] = useState<Project | "new" | null>(null)
  const [accessItem, setAccessItem] = useState<Project | null>(null)
  const load = useCallback(() => api.projects().then(setItems).catch(() => notify(t("genericError"), true)).finally(() => setLoading(false)), [notify, t])
  useEffect(() => { void load() }, [load])
  const filtered = items.filter((item) => item.name.toLowerCase().includes(query.toLowerCase()))
  async function save(name: string) {
    editing === "new" ? await api.createProject(name) : editing && await api.updateProject(editing.id, name)
    setEditing(null); notify(t("changesSaved")); await load()
  }
  async function remove(item: Project) { try { await api.trashProject(item.id); notify(t("movedToTrash")); await load() } catch (error) { notify(apiErrorText(error, t), true) } }
  return <div className="page"><PageHeader title={t("projects")} action={isAdmin ? <Button variant="primary" icon={<Plus />} label={`${t("new")} ${t("project")}`} onClick={() => setEditing("new")} /> : undefined} />
    {loading ? <PageSkeleton /> : <><ListToolbar query={query} onQuery={setQuery} resultCount={filtered.length} totalCount={items.length} />
    <DataPanel empty={!filtered.length} emptyTitle={query ? t("noResultsTitle") : undefined} emptyDescription={query ? t("noResultsDescription") : undefined} onEmptyAction={isAdmin && !query ? () => setEditing("new") : undefined}>
      <table><thead><tr><th>{t("projectName")}</th><th>{t("permission")}</th><th>{t("lastUpdated")}</th><th className="action-column">{t("actions")}</th></tr></thead>
        <tbody>{filtered.map((item) => <tr key={item.id}><td><div className="title-cell"><span className="row-icon"><FolderKanban /></span><div><strong>{item.name}</strong></div></div></td><td><StatusPill value={item.permissions.write ? "readWrite" : "readOnly"} /></td><td>{formatDate(item.updatedAt)}</td><td><RowActions>{isAdmin && <IconButton variant="ghost" label={t("manageAccess")} icon={<ShieldCheck />} onClick={() => setAccessItem(item)} />}{isAdmin && <IconButton variant="ghost" label={t("edit")} icon={<Pencil />} onClick={() => setEditing(item)} />}{isAdmin && <IconButton variant="ghost" label={t("delete")} icon={<Trash2 />} onClick={() => void remove(item)} />}</RowActions></td></tr>)}</tbody>
      </table>
    </DataPanel></>}
    {isAdmin && editing && <ProjectDialog item={editing === "new" ? null : editing} onClose={() => setEditing(null)} onSave={save} />}
    {isAdmin && accessItem && <ResourceAccessDialog resource={{ kind: "project", id: accessItem.id, name: accessItem.name }} onClose={() => setAccessItem(null)} notify={notify} />}
  </div>
}

function ProjectDialog({ item, onClose, onSave }: { item: Project | null; onClose: () => void; onSave: (name: string) => Promise<void> }) {
  const { t } = useI18n(); const [name, setName] = useState(item?.name || ""); const [busy, setBusy] = useState(false); const [touched, setTouched] = useState(false); const [formError, setFormError] = useState<string | null>(null); const error = textValidation(name, 500, "projectNameRequirement", t)
  async function submit(event: FormEvent) { event.preventDefault(); if (busy) return; setTouched(true); if (error) return; setBusy(true); setFormError(null); try { await onSave(name.trim()) } catch (caught) { setFormError(apiErrorText(caught, t)) } finally { setBusy(false) } }
  return <Modal title={`${item ? t("edit") : t("new")} ${t("project")}`} onClose={onClose}><form className="form-stack" onSubmit={submit} noValidate>{!item && <Banner status="info" title={t("newProjectHint")} />}{formError && <Banner status="error" title={formError} />}<Field label={t("projectName")} value={name} onChange={setName} onBlur={() => setTouched(true)} autoFocus required hint={t("projectNameRequirement")} error={touched ? error : undefined} /><DialogActions onClose={onClose} busy={busy} disabled={Boolean(error)} /></form></Modal>
}

function SecretsPage({ isAdmin, notify }: { isAdmin: boolean; notify: (text: string, error?: boolean) => void }) {
  const { t } = useI18n()
  const [items, setItems] = useState<Secret[]>([])
  const [projects, setProjects] = useState<Project[]>([])
  const [loading, setLoading] = useState(true)
  const [query, setQuery] = useState("")
  const [editing, setEditing] = useState<Secret | "new" | null>(null)
  const [accessItem, setAccessItem] = useState<Secret | null>(null)
  const [revealed, setRevealed] = useState<Set<string>>(new Set())
  const load = useCallback(() => Promise.all([api.secrets(), api.projects()]).then(([secrets, projects]) => { setItems(secrets); setProjects(projects) }).catch(() => notify(t("genericError"), true)).finally(() => setLoading(false)), [notify, t])
  useEffect(() => { void load() }, [load])
  const filtered = items.filter((item) => `${item.key} ${item.note}`.toLowerCase().includes(query.toLowerCase()))
  const canCreate = isAdmin || projects.some((project) => project.permissions.write)
  async function save(input: { key: string; value: string; note: string; projectId: string | null }) {
    editing === "new" ? await api.createSecret(input) : editing && await api.updateSecret(editing.id, input)
    setEditing(null); notify(t("changesSaved")); await load()
  }
  async function remove(item: Secret) { try { await api.trashSecret(item.id); notify(t("movedToTrash")); await load() } catch (error) { notify(apiErrorText(error, t), true) } }
  async function copy(value: string) { await navigator.clipboard.writeText(value); notify(t("copied")) }
  return <div className="page"><PageHeader title={t("secrets")} description={t("secretsIntro")} action={canCreate ? <span id="tour-new-secret"><Button variant="primary" icon={<Plus />} label={`${t("new")} ${t("secret")}`} onClick={() => setEditing("new")} /></span> : undefined} />
    {loading ? <PageSkeleton /> : <><ListToolbar query={query} onQuery={setQuery} resultCount={filtered.length} totalCount={items.length} />
    <DataPanel empty={!filtered.length} emptyIcon={<KeyRound />} emptyTitle={query ? t("noResultsTitle") : t("emptySecretsTitle")} emptyDescription={query ? t("noResultsDescription") : t("emptySecretsText")} emptyActionLabel={t("createFirstSecret")} onEmptyAction={canCreate && !query ? () => setEditing("new") : undefined}>
      <table><thead><tr><th>{t("secretKey")}</th><th>{t("secretValue")}</th><th>{t("project")}</th><th>{t("permission")}</th><th>{t("lastUpdated")}</th><th className="action-column">{t("actions")}</th></tr></thead>
        <tbody>{filtered.map((item) => { const isRevealed = revealed.has(item.id); return <tr key={item.id}><td><div className="title-cell"><span className="row-icon"><KeyRound /></span><div className="copyable-cell"><strong>{item.key}</strong></div><IconButton variant="ghost" label={t("copySecretKey")} icon={<Copy />} onClick={() => void copy(item.key)} /></div></td><td><div className="copyable-value"><code className="secret-value">{item.value === null ? "••••••••" : isRevealed ? item.value : "••••••••••••"}</code>{item.value !== null && <IconButton variant="ghost" label={t("copySecretValue")} icon={<Copy />} onClick={() => void copy(item.value || "")} />}</div></td><td>{projects.find((project) => project.id === item.projectId)?.name || t("noProject")}</td><td><StatusPill value={item.permissions.write ? "readWrite" : "readOnly"} /></td><td>{formatDate(item.updatedAt)}</td><td><RowActions>{item.value !== null && <IconButton variant="ghost" label={t(isRevealed ? "hideSecretValue" : "revealSecretValue")} icon={<KeyRound />} onClick={() => setRevealed((current) => { const next = new Set(current); next.has(item.id) ? next.delete(item.id) : next.add(item.id); return next })} />}{isAdmin && <IconButton variant="ghost" label={t("manageAccess")} icon={<ShieldCheck />} onClick={() => setAccessItem(item)} />}{item.permissions.write && <IconButton variant="ghost" label={t("edit")} icon={<Pencil />} onClick={() => setEditing(item)} />}{item.permissions.write && <IconButton variant="ghost" label={t("delete")} icon={<Trash2 />} onClick={() => void remove(item)} />}</RowActions></td></tr> })}</tbody>
      </table>
    </DataPanel></>}
    {editing && <SecretDialog item={editing === "new" ? null : editing} projects={projects} allowUnassigned={isAdmin || (editing !== "new" && editing.projectId === null)} onClose={() => setEditing(null)} onSave={save} />}
    {isAdmin && accessItem && <ResourceAccessDialog resource={{ kind: "secret", id: accessItem.id, name: accessItem.key }} onClose={() => setAccessItem(null)} notify={notify} />}
  </div>
}

function SecretDialog({ item, projects, allowUnassigned, onClose, onSave }: { item: Secret | null; projects: Project[]; allowUnassigned: boolean; onClose: () => void; onSave: (input: { key: string; value: string; note: string; projectId: string | null }) => Promise<void> }) {
  const { t } = useI18n(); const allowedProjects = projects.filter((project) => project.permissions.write || project.id === item?.projectId); const [key, setKey] = useState(item?.key || ""); const [value, setValue] = useState(item?.value || ""); const [note, setNote] = useState(item?.note || ""); const [projectId, setProjectId] = useState(item?.projectId || (allowUnassigned ? "" : allowedProjects[0]?.id || "")); const [busy, setBusy] = useState(false); const [touched, setTouched] = useState(false); const [formError, setFormError] = useState<string | null>(null); const keyError = textValidation(key, 500, "secretKeyRequirement", t); const projectError = !projectId && !allowUnassigned ? t("requiredField") : undefined
  async function submit(event: FormEvent) { event.preventDefault(); if (busy) return; setTouched(true); if (keyError || projectError) return; setBusy(true); setFormError(null); try { await onSave({ key: key.trim(), value, note, projectId: projectId || null }) } catch (caught) { setFormError(apiErrorText(caught, t)) } finally { setBusy(false) } }
  const projectOptions = [...(allowUnassigned ? [{ value: "", label: t("noProject") }] : []), ...allowedProjects.map((project) => ({ value: project.id, label: project.name }))]
  const selectedProject = allowedProjects.find((project) => project.id === projectId)
  const storageHint = selectedProject ? t("projectSecretHint") : t("unassignedSecretHint")
  return <Modal title={`${item ? t("edit") : t("new")} ${t("secret")}`} onClose={onClose} wide><form className="form-stack" onSubmit={submit} noValidate>{formError && <Banner status="error" title={formError} />}<div className="form-grid"><Field label={t("secretKey")} value={key} onChange={setKey} onBlur={() => setTouched(true)} autoFocus required hint={t("secretKeyRequirement")} error={touched ? keyError : undefined} /><Field label={t("secretValue")} value={value} onChange={setValue} /></div><TextArea label={t("note")} value={note} onChange={setNote} /><SelectField label={t("project")} value={projectId} onChange={setProjectId} options={projectOptions} /><p className="field-hint" data-error={Boolean(touched && projectError) || undefined}>{touched && projectError ? projectError : storageHint}</p><DialogActions onClose={onClose} busy={busy} disabled={Boolean(keyError || projectError)} /></form></Modal>
}

function MachinesPage({ notify }: { notify: (text: string, error?: boolean) => void }) {
  const { t } = useI18n(); const [items, setItems] = useState<MachineAccount[]>([]); const [query, setQuery] = useState(""); const [loading, setLoading] = useState(true); const [creating, setCreating] = useState(false); const [issued, setIssued] = useState<IssuedMachineAccount | null>(null); const [accessItem, setAccessItem] = useState<MachineAccount | null>(null); const [operationsItem, setOperationsItem] = useState<MachineAccount | null>(null); const [deleting, setDeleting] = useState<MachineAccount | null>(null); const [deleteBusy, setDeleteBusy] = useState(false)
  const load = useCallback(() => api.machines().then(setItems).catch(() => notify(t("genericError"), true)).finally(() => setLoading(false)), [notify, t]); useEffect(() => { void load() }, [load])
  const filtered = items.filter((item) => `${item.name} ${item.clientId}`.toLowerCase().includes(query.trim().toLowerCase()))
  async function create(name: string) { const value = await api.createMachine(name); setCreating(false); setIssued(value); await load() }
  async function toggle(item: MachineAccount) { try { await api.setMachineRevoked(item.id, !item.revokedAt); notify(t("machineStatusUpdated")); await load() } catch (error) { notify(apiErrorText(error, t), true) } }
  async function remove() { if (!deleting) return; setDeleteBusy(true); try { await api.deleteMachine(deleting.id); setDeleting(null); notify(t("machineDeleted")); await load() } catch (error) { notify(apiErrorText(error, t), true) } finally { setDeleteBusy(false) } }
  return <div className="page"><PageHeader title={t("machines")} action={<span id="tour-new-machine"><Button variant="primary" icon={<Plus />} label={`${t("new")} ${t("machineAccount")}`} onClick={() => setCreating(true)} /></span>} />{loading ? <PageSkeleton /> : <><ListToolbar query={query} onQuery={setQuery} resultCount={filtered.length} totalCount={items.length} /><DataPanel empty={!filtered.length} emptyTitle={query ? t("noResultsTitle") : undefined} emptyDescription={query ? t("noResultsDescription") : undefined} onEmptyAction={!query ? () => setCreating(true) : undefined}><table><thead><tr><th>{t("machineName")}</th><th>{t("clientId")}</th><th>{t("lastUsed")}</th><th>{t("status")}</th><th className="action-column">{t("actions")}</th></tr></thead><tbody>{filtered.map((item) => <tr key={item.id}><td><div className="title-cell"><span className="row-icon"><ServerCog /></span><div><strong>{item.name}</strong>{item.compatibilityAccount && <small>{t("compatibilityAccount")}</small>}</div></div></td><td><code>{item.clientId}</code></td><td>{item.lastUsedAt ? formatDate(item.lastUsedAt) : t("never")}</td><td><StatusPill value={item.revokedAt ? "revoked" : "active"} /></td><td><RowActions>{!item.compatibilityAccount && <IconButton variant="ghost" label={t("credentialsAndEvents")} icon={<KeyRound />} onClick={() => setOperationsItem(item)} />}{!item.compatibilityAccount && <IconButton variant="ghost" label={t("manageAccess")} icon={<ShieldCheck />} onClick={() => setAccessItem(item)} />}<Button size="sm" variant="ghost" label={item.revokedAt ? t("enable") : t("revoke")} onClick={() => void toggle(item)} />{!item.compatibilityAccount && <IconButton variant="destructive" label={t("delete")} icon={<Trash2 />} onClick={() => setDeleting(item)} />}</RowActions></td></tr>)}</tbody></table></DataPanel></>}{creating && <NameDialog title={`${t("new")} ${t("machineAccount")}`} label={t("machineName")} onClose={() => setCreating(false)} onSave={create} />}{issued && <TokenDialog item={issued} onClose={() => setIssued(null)} notify={notify} />}{accessItem && <ResourceAccessDialog resource={{ kind: "machine", id: accessItem.id, name: accessItem.name }} onClose={() => setAccessItem(null)} notify={notify} />}{operationsItem && <MachineOperationsDialog machine={operationsItem} onClose={() => setOperationsItem(null)} notify={notify} />}{deleting && <ConfirmActionDialog title={t("confirmDeleteMachineTitle")} description={`${deleting.name} · ${t("confirmDeleteMachineText")}`} actionLabel={t("delete")} busy={deleteBusy} onAction={() => void remove()} onClose={() => setDeleting(null)} />}</div>
}

function TokenDialog({ item, onClose, notify }: { item: IssuedMachineAccount | IssuedMachineAccessToken; onClose: () => void; notify: (text: string) => void }) {
  const { t } = useI18n(); async function copy() { await navigator.clipboard.writeText(item.accessToken); notify(t("copied")) }
  return <Modal title={t("accessToken")} onClose={onClose} wide><Banner status="warning" title={t("tokenOneTime")} /><div className="token-box"><code>{item.accessToken}</code><IconButton variant="ghost" label={t("copy")} icon={<Copy />} onClick={() => void copy()} /></div><div className="dialog-actions"><Button variant="primary" label={t("close")} onClick={onClose} /></div></Modal>
}

function MachineOperationsDialog({ machine, onClose, notify }: { machine: MachineAccount; onClose: () => void; notify: (text: string, error?: boolean) => void }) {
  const { t } = useI18n()
  const [tab, setTab] = useState<"tokens" | "events">("tokens")
  const [tokens, setTokens] = useState<MachineAccessToken[]>([])
  const [events, setEvents] = useState<AuditEvent[]>([])
  const [loading, setLoading] = useState(true)
  const [creating, setCreating] = useState(false)
  const [issued, setIssued] = useState<IssuedMachineAccessToken | null>(null)
  const [revoking, setRevoking] = useState<MachineAccessToken | null>(null)
  const [revokeBusy, setRevokeBusy] = useState(false)
  const load = useCallback(async () => {
    setLoading(true)
    try {
      const [nextTokens, nextEvents] = await Promise.all([api.machineTokens(machine.id), api.machineEvents(machine.id)])
      setTokens(nextTokens); setEvents(nextEvents)
    } catch { notify(t("genericError"), true) }
    finally { setLoading(false) }
  }, [machine.id, notify, t])
  useEffect(() => { void load() }, [load])
  async function create(name: string, expiresAt: number | null) {
    const value = await api.createMachineToken(machine.id, name, expiresAt); setCreating(false); setIssued(value); await load()
  }
  async function revoke() {
    if (!revoking) return
    setRevokeBusy(true)
    try { await api.revokeMachineToken(machine.id, revoking.id); setRevoking(null); notify(t("tokenRevoked")); await load() }
    catch (error) { notify(apiErrorText(error, t), true) }
    finally { setRevokeBusy(false) }
  }
  return <>
    <Modal title={`${t("credentialsAndEvents")} · ${machine.name}`} onClose={onClose} wide>
      <div className="dialog-tabs" role="tablist" aria-label={t("credentialsAndEvents")}>
        <button type="button" role="tab" aria-selected={tab === "tokens"} data-active={tab === "tokens"} onClick={() => setTab("tokens")}><KeyRound />{t("accessTokens")}</button>
        <button type="button" role="tab" aria-selected={tab === "events"} data-active={tab === "events"} onClick={() => setTab("events")}><ScrollText />{t("eventLogs")}</button>
      </div>
      {loading ? <div className="access-loading"><RefreshCw /><span>{t("loading")}</span></div> : tab === "tokens" ? <section className="machine-operation-panel" role="tabpanel">
        <div className="panel-heading"><div><h3>{t("accessTokens")}</h3><p>{t("accessTokensHint")}</p></div><Button variant="secondary" icon={<Plus />} label={t("createAccessToken")} onClick={() => setCreating(true)} /></div>
        {tokens.length ? <div className="table-scroll"><table><thead><tr><th>{t("tokenName")}</th><th>{t("expires")}</th><th>{t("lastUsed")}</th><th>{t("status")}</th><th className="action-column">{t("actions")}</th></tr></thead><tbody>{tokens.map((token) => { const expired = token.expiresAt !== null && token.expiresAt <= Math.floor(Date.now() / 1000); return <tr key={token.id}><td><strong>{token.name}</strong></td><td>{token.expiresAt ? formatDate(token.expiresAt) : t("never")}</td><td>{token.lastUsedAt ? formatDate(token.lastUsedAt) : t("never")}</td><td><StatusPill value={token.revokedAt ? "revoked" : expired ? "expired" : "active"} /></td><td><RowActions>{!token.revokedAt && <Button size="sm" variant="destructive" label={t("revoke")} onClick={() => setRevoking(token)} />}</RowActions></td></tr> })}</tbody></table></div> : <div className="compact-empty"><KeyRound /><p>{t("emptyAccessTokens")}</p></div>}
      </section> : <section className="machine-operation-panel" role="tabpanel">
        <div className="panel-heading"><div><h3>{t("eventLogs")}</h3><p>{t("eventLogsHint")}</p></div><IconButton variant="ghost" label={t("refresh")} icon={<RefreshCw />} onClick={() => void load()} /></div>
        {events.length ? <div className="table-scroll"><table><thead><tr><th>{t("auditAction")}</th><th>{t("resourceId")}</th><th>{t("outcome")}</th><th>{t("time")}</th></tr></thead><tbody>{events.map((event) => <tr key={event.id}><td><code>{event.action}</code></td><td><code>{event.resourceId || "—"}</code></td><td><StatusPill value={event.outcome} /></td><td>{formatDate(event.createdAt)}</td></tr>)}</tbody></table></div> : <div className="compact-empty"><ScrollText /><p>{t("emptyMachineEvents")}</p></div>}
      </section>}
    </Modal>
    {creating && <CreateAccessTokenDialog onClose={() => setCreating(false)} onSave={create} />}
    {issued && <TokenDialog item={issued} onClose={() => setIssued(null)} notify={notify} />}
    {revoking && <ConfirmActionDialog title={t("confirmRevokeTokenTitle")} description={`${revoking.name} · ${t("confirmRevokeTokenText")}`} actionLabel={t("revoke")} busy={revokeBusy} onAction={() => void revoke()} onClose={() => setRevoking(null)} />}
  </>
}

function CreateAccessTokenDialog({ onClose, onSave }: { onClose: () => void; onSave: (name: string, expiresAt: number | null) => Promise<void> }) {
  const { t } = useI18n()
  const [name, setName] = useState("")
  const [expiry, setExpiry] = useState("never")
  const [customExpiry, setCustomExpiry] = useState("")
  const [busy, setBusy] = useState(false)
  const [touched, setTouched] = useState(false)
  const [formError, setFormError] = useState<string | null>(null)
  const nameError = textValidation(name, 128, "nameRequirement", t)
  const expiryError = expiry === "custom" && (!customExpiry || !Number.isFinite(new Date(customExpiry).getTime()) || new Date(customExpiry).getTime() <= Date.now()) ? t("futureDateRequired") : undefined
  async function submit(event: FormEvent) {
    event.preventDefault()
    if (busy) return
    setTouched(true)
    if (nameError || expiryError) return
    const expiresAt = expiry === "never" ? null : expiry === "custom" ? Math.floor(new Date(customExpiry).getTime() / 1000) : Math.floor(Date.now() / 1000) + Number(expiry) * 24 * 60 * 60
    setBusy(true); setFormError(null); try { await onSave(name.trim(), expiresAt) } catch (caught) { setFormError(apiErrorText(caught, t)) } finally { setBusy(false) }
  }
  return <Modal title={t("createAccessToken")} onClose={onClose}><form className="form-stack" onSubmit={submit} noValidate>{formError && <Banner status="error" title={formError} />}<Field label={t("tokenName")} value={name} onChange={setName} onBlur={() => setTouched(true)} autoFocus required hint={t("nameRequirement")} error={touched ? nameError : undefined} /><SelectField label={t("expires")} value={expiry} onChange={setExpiry} options={[{ value: "never", label: t("never") }, { value: "7", label: t("sevenDays") }, { value: "30", label: t("thirtyDays") }, { value: "60", label: t("sixtyDays") }, { value: "custom", label: t("customExpiry") }]} />{expiry === "custom" && <Field label={t("customExpiry")} type="datetime-local" value={customExpiry} onChange={setCustomExpiry} onBlur={() => setTouched(true)} min={minimumLocalDateTime()} required error={touched ? expiryError : undefined} />}<DialogActions onClose={onClose} busy={busy} disabled={Boolean(nameError || expiryError)} /></form></Modal>
}

function UsersPage({ currentUser, notify }: { currentUser: User; notify: (text: string, error?: boolean) => void }) {
  const { t } = useI18n(); const [items, setItems] = useState<User[]>([]); const [query, setQuery] = useState(""); const [loading, setLoading] = useState(true); const [editing, setEditing] = useState<User | "new" | null>(null); const [passwordUser, setPasswordUser] = useState<User | null>(null)
  const load = useCallback(() => api.users().then(setItems).catch(() => notify(t("genericError"), true)).finally(() => setLoading(false)), [notify, t]); useEffect(() => { void load() }, [load])
  const filtered = items.filter((item) => `${item.displayName} ${item.username} ${item.role} ${item.disabled ? "disabled" : "active"}`.toLowerCase().includes(query.trim().toLowerCase()))
  async function save(input: { username: string; displayName: string; role: Role; password: string; disabled: boolean }) {
    if (editing === "new") await api.createUser({ username: input.username, displayName: input.displayName, role: input.role, password: input.password })
    else if (editing) await api.updateUser(editing.id, { displayName: input.displayName, role: input.role, disabled: input.disabled })
    setEditing(null); notify(t("changesSaved")); await load()
  }
  async function password(value: string) { if (!passwordUser) return; await api.resetPassword(passwordUser.id, value); setPasswordUser(null); notify(t("passwordResetComplete")) }
  return <div className="page"><PageHeader title={t("users")} description={t("usersIntro")} action={<span id="tour-new-user"><Button variant="primary" icon={<Plus />} label={`${t("new")} ${t("user")}`} onClick={() => setEditing("new")} /></span>} />{loading ? <PageSkeleton /> : <><ListToolbar query={query} onQuery={setQuery} resultCount={filtered.length} totalCount={items.length} /><DataPanel empty={!filtered.length} emptyIcon={<UsersRound />} emptyTitle={query ? t("noResultsTitle") : t("emptyUsersTitle")} emptyDescription={query ? t("noResultsDescription") : t("emptyUsersText")} emptyActionLabel={`${t("new")} ${t("user")}`} onEmptyAction={!query ? () => setEditing("new") : undefined}><table><thead><tr><th>{t("displayName")}</th><th>{t("username")}</th><th>{t("role")}</th><th>{t("lastUsed")}</th><th>{t("status")}</th><th className="action-column">{t("actions")}</th></tr></thead><tbody>{filtered.map((item) => <tr key={item.id}><td><div className="title-cell"><span className="avatar small">{item.displayName.slice(0, 2).toUpperCase()}</span><div><strong>{item.displayName}</strong>{item.id === currentUser.id && <small>{t("currentUser")}</small>}</div></div></td><td>{item.username}</td><td>{item.role === "admin" ? t("administrator") : t("member")}</td><td>{item.lastLoginAt ? formatDate(item.lastLoginAt) : t("never")}</td><td><StatusPill value={item.disabled ? "disabled" : "active"} /></td><td><RowActions><IconButton variant="ghost" label={t("edit")} icon={<UserCog />} onClick={() => setEditing(item)} /><Button size="sm" variant="ghost" label={t("resetPassword")} onClick={() => setPasswordUser(item)} /></RowActions></td></tr>)}</tbody></table></DataPanel></>}{editing && <UserDialog item={editing === "new" ? null : editing} currentUser={currentUser} onClose={() => setEditing(null)} onSave={save} />}{passwordUser && <NameDialog title={t("resetPassword")} label={t("newPassword")} type="password" onClose={() => setPasswordUser(null)} onSave={password} />}</div>
}

function UserDialog({ item, currentUser, onClose, onSave }: { item: User | null; currentUser: User; onClose: () => void; onSave: (input: { username: string; displayName: string; role: Role; password: string; disabled: boolean }) => Promise<void> }) {
  const { t } = useI18n(); const [username, setUsername] = useState(item?.username || ""); const [displayName, setDisplayName] = useState(item?.displayName || ""); const [password, setPassword] = useState(""); const [role, setRole] = useState<Role>(item?.role || "user"); const [disabled, setDisabled] = useState(item?.disabled || false); const [busy, setBusy] = useState(false); const [touched, setTouched] = useState({ username: false, displayName: false, password: false }); const [formError, setFormError] = useState<string | null>(null); const self = item?.id === currentUser.id
  const usernameError = textValidation(username, 128, "nameRequirement", t); const displayNameError = textValidation(displayName, 128, "nameRequirement", t); const passwordError = item ? undefined : passwordValidation(password, t); const accessChanged = Boolean(item && !self && (role !== item.role || disabled !== item.disabled))
  async function submit(event: FormEvent) { event.preventDefault(); if (busy) return; setTouched({ username: true, displayName: true, password: true }); if (usernameError || displayNameError || passwordError) return; setBusy(true); setFormError(null); try { await onSave({ username: username.trim(), displayName: displayName.trim(), password, role, disabled }) } catch (caught) { setFormError(apiErrorText(caught, t)) } finally { setBusy(false) } }
  return <Modal title={`${item ? t("edit") : t("new")} ${t("user")}`} onClose={onClose}><form className="form-stack" onSubmit={submit} noValidate>{formError && <Banner status="error" title={formError} />}<Field label={t("username")} value={username} onChange={setUsername} onBlur={() => setTouched((value) => ({ ...value, username: true }))} disabled={Boolean(item)} autoComplete="username" autoFocus required hint={t("nameRequirement")} error={touched.username ? usernameError : undefined} /><Field label={t("displayName")} value={displayName} onChange={setDisplayName} onBlur={() => setTouched((value) => ({ ...value, displayName: true }))} required hint={t("nameRequirement")} error={touched.displayName ? displayNameError : undefined} />{!item && <Field label={t("password")} value={password} onChange={setPassword} onBlur={() => setTouched((value) => ({ ...value, password: true }))} type="password" autoComplete="new-password" required hint={t("passwordRequirement")} error={touched.password ? passwordError : undefined} />}<SelectField label={t("role")} value={role} onChange={(value) => setRole(value as Role)} disabled={self} options={[{ value: "user", label: t("member") }, { value: "admin", label: t("administrator") }]} />{item && <CheckField label={t("disabled")} checked={disabled} onChange={setDisabled} disabled={self} />}{accessChanged && <Banner status="warning" title={t("userAccessChangeWarning")} />}<DialogActions onClose={onClose} busy={busy} disabled={Boolean(usernameError || displayNameError || passwordError)} /></form></Modal>
}

function GroupsPage({ notify, onNavigate }: { notify: (text: string, error?: boolean) => void; onNavigate: (page: Page) => void }) {
  const { t } = useI18n()
  const [groups, setGroups] = useState<Group[]>([])
  const [users, setUsers] = useState<User[]>([])
  const [query, setQuery] = useState("")
  const [loading, setLoading] = useState(true)
  const [editing, setEditing] = useState<Group | "new" | null>(null)
  const [membersGroup, setMembersGroup] = useState<Group | null>(null)
  const [deleting, setDeleting] = useState<Group | null>(null)
  const [deleteBusy, setDeleteBusy] = useState(false)
  const load = useCallback(() => Promise.all([api.groups(), api.users()]).then(([nextGroups, nextUsers]) => { setGroups(nextGroups); setUsers(nextUsers) }).catch(() => notify(t("genericError"), true)).finally(() => setLoading(false)), [notify, t])
  useEffect(() => { void load() }, [load])
  async function save(name: string) { editing === "new" ? await api.createGroup(name) : editing && await api.updateGroup(editing.id, name); setEditing(null); notify(t("changesSaved")); await load() }
  async function remove() { if (!deleting) return; setDeleteBusy(true); try { await api.deleteGroup(deleting.id); setDeleting(null); notify(t("changesSaved")); await load() } catch (error) { notify(apiErrorText(error, t), true) } finally { setDeleteBusy(false) } }
  async function saveMembers(memberIds: string[]) { if (!membersGroup) return; await api.replaceGroupMembers(membersGroup.id, memberIds); setMembersGroup(null); notify(t("groupMembersSaved")); await load() }
  const needsMoreUsers = users.length <= 1 && groups.length === 0
  const filtered = groups.filter((group) => group.name.toLowerCase().includes(query.trim().toLowerCase()))
  const emptyAction = needsMoreUsers ? () => onNavigate("users") : () => setEditing("new")
  return <div className="page"><PageHeader title={t("groups")} description={t("groupsIntro")} action={loading ? undefined : needsMoreUsers ? <Button variant="primary" icon={<Plus />} label={`${t("new")} ${t("user")}`} onClick={() => onNavigate("users")} /> : <Button variant="primary" icon={<Plus />} label={`${t("new")} ${t("group")}`} onClick={() => setEditing("new")} />} />{loading ? <PageSkeleton /> : <>{groups.length > 0 && <ListToolbar query={query} onQuery={setQuery} resultCount={filtered.length} totalCount={groups.length} />}<DataPanel empty={!filtered.length} emptyIcon={<Boxes />} emptyTitle={query ? t("noResultsTitle") : t(needsMoreUsers ? "groupsNeedUsersTitle" : "emptyGroupsTitle")} emptyDescription={query ? t("noResultsDescription") : t(needsMoreUsers ? "groupsNeedUsersText" : "emptyGroupsText")} emptyActionLabel={t(needsMoreUsers ? "addUserFirst" : "createGroup")} onEmptyAction={!query ? emptyAction : undefined}><table><thead><tr><th>{t("groupName")}</th><th>{t("members")}</th><th>{t("lastUpdated")}</th><th className="action-column">{t("actions")}</th></tr></thead><tbody>{filtered.map((group) => <tr key={group.id}><td><div className="title-cell"><span className="row-icon"><Boxes /></span><strong>{group.name}</strong></div></td><td>{group.memberIds.length}</td><td>{formatDate(group.updatedAt)}</td><td><RowActions><Button size="sm" variant="ghost" label={t("manageMembers")} onClick={() => setMembersGroup(group)} /><IconButton variant="ghost" label={t("edit")} icon={<Pencil />} onClick={() => setEditing(group)} /><IconButton variant="destructive" label={t("delete")} icon={<Trash2 />} onClick={() => setDeleting(group)} /></RowActions></td></tr>)}</tbody></table></DataPanel></>}{editing && <NameDialog title={`${editing === "new" ? t("new") : t("edit")} ${t("group")}`} label={t("groupName")} initialValue={editing === "new" ? "" : editing.name} onClose={() => setEditing(null)} onSave={save} />}{membersGroup && <GroupMembersDialog group={membersGroup} users={users} onClose={() => setMembersGroup(null)} onSave={saveMembers} />}{deleting && <ConfirmActionDialog title={`${t("delete")} ${t("group")}?`} description={`${deleting.name} · ${t("confirmDeleteGroup")}`} actionLabel={t("delete")} busy={deleteBusy} onAction={() => void remove()} onClose={() => setDeleting(null)} />}</div>
}

function GroupMembersDialog({ group, users, onClose, onSave }: { group: Group; users: User[]; onClose: () => void; onSave: (memberIds: string[]) => Promise<void> }) {
  const { t } = useI18n(); const [selected, setSelected] = useState(() => new Set(group.memberIds)); const [busy, setBusy] = useState(false); const [formError, setFormError] = useState<string | null>(null)
  async function submit(event: FormEvent) { event.preventDefault(); if (busy) return; setBusy(true); setFormError(null); try { await onSave([...selected]) } catch (caught) { setFormError(apiErrorText(caught, t)) } finally { setBusy(false) } }
  return <Modal title={`${t("manageMembers")} · ${group.name}`} onClose={onClose} wide><form className="form-stack" onSubmit={submit}>{formError && <Banner status="error" title={formError} />}<div className="member-picker">{users.map((user) => <label className="member-option" key={user.id}><input type="checkbox" checked={selected.has(user.id)} onChange={(event) => setSelected((current) => { const next = new Set(current); event.target.checked ? next.add(user.id) : next.delete(user.id); return next })} /><span className="avatar small">{user.displayName.slice(0, 2).toUpperCase()}</span><span><strong>{user.displayName}</strong><small>{user.username}</small></span></label>)}</div><DialogActions onClose={onClose} busy={busy} /></form></Modal>
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
  const [loading, setLoading] = useState(true)
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
  }).catch(() => notify(t("genericError"), true)).finally(() => setLoading(false)), [notify, t])
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
    <PageHeader title={t("auditLog")} description={t("auditIntro")} action={loading ? undefined : <Button variant="ghost" icon={<Trash2 />} label={t("clearAudit")} isLoading={busy === "clear"} isDisabled={!events.length} onClick={() => void clear()} />} />
    {loading ? <PageSkeleton /> : <><article className="panel settings-panel audit-settings-panel">
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
    </>}
  </div>
}

function TrashPage({ isAdmin, notify }: { isAdmin: boolean; notify: (text: string, error?: boolean) => void }) {
  const { t } = useI18n(); const [projects, setProjects] = useState<Project[]>([]); const [secrets, setSecrets] = useState<Secret[]>([]); const [loading, setLoading] = useState(true)
  const load = useCallback(() => Promise.all([api.projects(true), api.secrets(true)]).then(([projects, secrets]) => { setProjects(projects); setSecrets(secrets) }).catch(() => notify(t("genericError"), true)).finally(() => setLoading(false)), [notify, t]); useEffect(() => { void load() }, [load])
  async function act(kind: "project" | "secret", id: string, purge: boolean) { if (purge && !confirm(t("confirmDelete"))) return; try { if (kind === "project") purge ? await api.purgeProject(id) : await api.restoreProject(id); else purge ? await api.purgeSecret(id) : await api.restoreSecret(id); await load() } catch { notify(t("genericError"), true) } }
  const empty = !projects.length && !secrets.length
  return <div className="page"><PageHeader title={t("trash")} description={t("dangerousAction")} />{loading ? <PageSkeleton /> : <DataPanel empty={empty}><div className="trash-groups">{projects.length > 0 && <section><h2>{t("projects")}</h2>{projects.map((item) => <TrashRow key={item.id} name={item.name} canAct={isAdmin} onRestore={() => void act("project", item.id, false)} onPurge={() => void act("project", item.id, true)} />)}</section>}{secrets.length > 0 && <section><h2>{t("secrets")}</h2>{secrets.map((item) => <TrashRow key={item.id} name={item.key} canAct={item.permissions.write} onRestore={() => void act("secret", item.id, false)} onPurge={() => void act("secret", item.id, true)} />)}</section>}</div></DataPanel>}</div>
}

function TrashRow({ name, canAct, onRestore, onPurge }: { name: string; canAct: boolean; onRestore: () => void; onPurge: () => void }) { const { t } = useI18n(); return <div className="trash-row"><strong>{name}</strong>{canAct ? <RowActions><Button variant="ghost" size="sm" icon={<RefreshCw />} label={t("restore")} onClick={onRestore} /><Button variant="ghost" size="sm" icon={<Trash2 />} label={t("purge")} onClick={onPurge} /></RowActions> : <StatusPill value="readOnly" />}</div> }

function ServiceEndpoint({ notify, id }: { notify: (text: string, error?: boolean) => void; id?: string }) {
  const { t } = useI18n()
  const endpoint = location.origin
  async function copy() {
    await navigator.clipboard.writeText(endpoint)
    notify(t("endpointCopied"))
  }
  return <div className="service-endpoint" id={id}><span className="panel-icon"><Globe2 /></span><div><strong>{t("serviceEndpoint")}</strong><code>{endpoint}</code><small>{t("serviceEndpointHint")}</small></div><IconButton variant="ghost" label={t("copyEndpoint")} icon={<Copy />} onClick={() => void copy()} /></div>
}

function IntegrationsPage({ notify }: { notify: (text: string, error?: boolean) => void }) {
  const { t } = useI18n()
  const links: Array<{ title: MessageKey; href: string; detail: string }> = [
    { title: "officialSdk", href: "https://github.com/bitwarden/sdk-sm", detail: "Rust · JavaScript · Python · C#" },
    { title: "bwsCli", href: "https://github.com/bitwarden/sdk-sm/tree/main/crates/bws", detail: "bws --server-url <LIGHTBWS_URL>" },
    { title: "fnox", href: "https://fnox.jdx.dev/providers/bitwarden-sm", detail: "provider = \"bitwarden-sm\"" },
    { title: "bitwardenHelp", href: "https://bitwarden.com/help/secrets-manager-overview/", detail: "Concepts · SDK · Machine access" },
  ]
  return <div className="page"><PageHeader title={t("integrations")} description={t("integrationsIntro")} /><ServiceEndpoint notify={notify} /><section className="integration-grid">{links.map((link) => <a className="integration-card" key={link.href} href={link.href} target="_blank" rel="noreferrer"><span className="panel-icon"><Network /></span><div><h2>{t(link.title)}</h2><code>{link.detail}</code><span>{t("openLink")} <ExternalLink /></span></div></a>)}</section></div>
}

function HelpPage({ notify }: { notify: (text: string, error?: boolean) => void }) {
  const { t } = useI18n()
  const endpoint = location.origin
  const helpSections: Array<[string, MessageKey]> = [
    ["help-start", "helpStartTitle"],
    ["help-model", "helpModelTitle"],
    ["help-automation", "helpAutomationTitle"],
    ["help-people", "helpPeopleTitle"],
    ["help-operations", "helpOperationsTitle"],
  ]
  function scrollToSection(id: string) {
    document.getElementById(id)?.scrollIntoView({ behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth", block: "start" })
  }
  return <div className="page help-page"><PageHeader title={t("help")} description={t("helpIntro")} />
    <ServiceEndpoint notify={notify} />
    <nav className="help-toc" aria-label={t("helpContents")}>
      <strong>{t("helpContents")}</strong>
      {helpSections.map(([id, label]) => <button type="button" key={id} onClick={() => scrollToSection(id)}>{t(label)}</button>)}
    </nav>
    <section className="help-section" id="help-start"><div className="help-section-heading"><span>01</span><div><p className="eyebrow">{t("helpMinimumLabel")}</p><h2>{t("helpStartTitle")}</h2><p>{t("helpStartIntro")}</p></div></div>
      <ol className="help-steps"><li><strong>{t("helpStartStep1Title")}</strong><p>{t("helpStartStep1Text")}</p></li><li><strong>{t("helpStartStep2Title")}</strong><p>{t("helpStartStep2Text")}</p></li><li><strong>{t("helpStartStep3Title")}</strong><p>{t("helpStartStep3Text")}</p></li></ol>
      <div className="help-note"><Info /><p><strong>{t("helpWebSecretTitle")}</strong>{t("helpWebSecretText")}</p></div>
    </section>
    <section className="help-section" id="help-model"><div className="help-section-heading"><span>02</span><div><p className="eyebrow">{t("helpConceptsLabel")}</p><h2>{t("helpModelTitle")}</h2><p>{t("helpModelIntro")}</p></div></div>
      <div className="help-card-grid"><article><KeyRound /><h3>{t("secret")}</h3><p>{t("helpSecretConcept")}</p></article><article><FolderKanban /><h3>{t("project")}</h3><p>{t("helpProjectConcept")}</p></article><article><ServerCog /><h3>{t("machineAccount")}</h3><p>{t("helpMachineConcept")}</p></article><article><UsersRound /><h3>{t("usersAndGroups")}</h3><p>{t("helpPeopleConcept")}</p></article></div>
      <div className="help-note"><ShieldCheck /><p><strong>{t("helpPermissionTitle")}</strong>{t("helpPermissionText")}</p></div>
    </section>
    <section className="help-section" id="help-automation"><div className="help-section-heading"><span>03</span><div><p className="eyebrow">{t("helpScenarioLabel")}</p><h2>{t("helpAutomationTitle")}</h2><p>{t("helpAutomationIntro")}</p></div></div>
      <ol className="help-steps"><li><strong>{t("helpAutomationStep1Title")}</strong><p>{t("helpAutomationStep1Text")}</p></li><li><strong>{t("helpAutomationStep2Title")}</strong><p>{t("helpAutomationStep2Text")}</p></li><li><strong>{t("helpAutomationStep3Title")}</strong><p>{t("helpAutomationStep3Text")}</p></li><li><strong>{t("helpAutomationStep4Title")}</strong><p>{t("helpAutomationStep4Text")}</p></li></ol>
      <pre className="help-code" role="region" tabIndex={0} aria-label={t("helpCodeLabel")}><code>{`export BWS_ACCESS_TOKEN='<access-token>'\nexport BWS_SERVER_URL='${endpoint}'\nbws project list\nbws secret list\nfnox get DATABASE_URL`}</code></pre>
      <div className="help-note"><Clock3 /><p><strong>{t("helpTokenTitle")}</strong>{t("helpTokenText")}</p></div>
    </section>
    <section className="help-section" id="help-people"><div className="help-section-heading"><span>04</span><div><p className="eyebrow">{t("helpScenarioLabel")}</p><h2>{t("helpPeopleTitle")}</h2><p>{t("helpPeopleIntro")}</p></div></div>
      <ol className="help-steps"><li><strong>{t("helpPeopleStep1Title")}</strong><p>{t("helpPeopleStep1Text")}</p></li><li><strong>{t("helpPeopleStep2Title")}</strong><p>{t("helpPeopleStep2Text")}</p></li><li><strong>{t("helpPeopleStep3Title")}</strong><p>{t("helpPeopleStep3Text")}</p></li></ol>
    </section>
    <section className="help-section" id="help-operations"><div className="help-section-heading"><span>05</span><div><p className="eyebrow">{t("helpOperationsLabel")}</p><h2>{t("helpOperationsTitle")}</h2><p>{t("helpOperationsIntro")}</p></div></div>
      <div className="help-card-grid"><article><ScrollText /><h3>{t("auditLog")}</h3><p>{t("helpAuditText")}</p></article><article><Trash2 /><h3>{t("trash")}</h3><p>{t("helpTrashText")}</p></article><article><DatabaseBackup /><h3>{t("backups")}</h3><p>{t("helpBackupText")}</p></article><article><ArchiveRestore /><h3>{t("transfer")}</h3><p>{t("helpTransferText")}</p></article></div>
      <div className="help-note"><Info /><p><strong>{t("helpTroubleshootTitle")}</strong>{t("helpTroubleshootText")}</p></div>
    </section>
  </div>
}

function BackupsPage({ notify }: { notify: (text: string, error?: boolean) => void }) {
  const { t } = useI18n(); const [targets, setTargets] = useState<BackupTarget[]>([]); const [jobs, setJobs] = useState<BackupJob[]>([]); const [capabilities, setCapabilities] = useState<BackupCapabilities>({ plaintextAllowed: false }); const [loading, setLoading] = useState(true); const [editing, setEditing] = useState<BackupTarget | "new" | null>(null); const [busyId, setBusyId] = useState<string | null>(null)
  const load = useCallback(() => Promise.all([api.backupTargets(), api.backupJobs(), api.backupCapabilities()]).then(([targets, jobs, capabilities]) => { setTargets(targets); setJobs(jobs); setCapabilities(capabilities) }).catch(() => notify(t("genericError"), true)).finally(() => setLoading(false)), [notify, t]); useEffect(() => { void load() }, [load])
  async function save(input: unknown) { try { editing === "new" ? await api.createBackupTarget(input) : editing && await api.updateBackupTarget(editing.id, input); setEditing(null); await load() } catch { notify(t("genericError"), true) } }
  async function run(target: BackupTarget) { setBusyId(target.id); try { const job = await api.runBackup(target.id); notify(job.status === "succeeded" ? t("succeeded") : t("failed"), job.status !== "succeeded"); await load() } catch { notify(t("genericError"), true) } finally { setBusyId(null) } }
  async function test(target: BackupTarget) { setBusyId(target.id); try { await api.testBackupTarget(target.id); notify(t("succeeded")) } catch { notify(t("genericError"), true) } finally { setBusyId(null) } }
  async function remove(target: BackupTarget) { if (!confirm(t("confirmDelete"))) return; try { await api.deleteBackupTarget(target.id); await load() } catch { notify(t("genericError"), true) } }
  return <div className="page"><PageHeader title={t("backups")} description={t("backupIntro")} action={loading ? undefined : <Button variant="primary" icon={<Plus />} label={t("newBackupTarget")} onClick={() => setEditing("new")} />} />{loading ? <PageSkeleton variant="cards" /> : <><section className="backup-target-grid">{targets.map((target) => <article className="panel backup-card" key={target.id}><div className="panel-heading"><div className="title-cell"><span className="row-icon"><CloudUpload /></span><div><h2>{target.displayName}</h2><small>{target.config.kind}</small></div></div><StatusPill value={target.enabled ? target.lastStatus || "active" : "disabled"} /></div><dl><div><dt>{t("endpoint")}</dt><dd>{target.config.settings.endpoint}</dd></div><div><dt>{t("backupEncryption")}</dt><dd>{t(target.encryption === "plaintext" ? "plaintext" : "masterKeyEncrypted")}</dd></div><div><dt>{t("backupScope")}</dt><dd>{scopeLabel(target.scopes, t)}</dd></div><div><dt>{t("nextRun")}</dt><dd>{target.nextRunAt ? formatDate(target.nextRunAt) : "—"}</dd></div><div><dt>{t("lastRun")}</dt><dd>{target.lastRunAt ? formatDate(target.lastRunAt) : t("never")}</dd></div></dl><div className="card-actions"><Button size="sm" variant="secondary" label={t("runNow")} isLoading={busyId === target.id} onClick={() => void run(target)} /><Button size="sm" variant="ghost" label={t("testTarget")} onClick={() => void test(target)} /><IconButton variant="ghost" label={t("edit")} icon={<Pencil />} onClick={() => setEditing(target)} /><IconButton variant="ghost" label={t("delete")} icon={<Trash2 />} onClick={() => void remove(target)} /></div></article>)}</section>{!targets.length && <DataPanel empty onEmptyAction={() => setEditing("new")} />}
    <section className="panel history-panel"><h2>{t("backupHistory")}</h2>{jobs.length ? <table><thead><tr><th>{t("backupTarget")}</th><th>{t("status")}</th><th>{t("lastRun")}</th><th>{t("actions")}</th></tr></thead><tbody>{jobs.map((job) => <tr key={job.id}><td><code>{job.objectKey}</code></td><td><StatusPill value={job.status} /></td><td>{formatDate(job.createdAt)}</td><td>{job.triggerKind === "manual" ? t("manual") : t("scheduled")}</td></tr>)}</tbody></table> : <p className="muted">{t("emptyBackupHistory")}</p>}</section></>}{editing && <BackupDialog item={editing === "new" ? null : editing} plaintextAllowed={capabilities.plaintextAllowed} onClose={() => setEditing(null)} onSave={save} />}</div>
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
  const { t } = useI18n(); const [capabilities, setCapabilities] = useState<BackupCapabilities>({ plaintextAllowed: false }); const [loading, setLoading] = useState(true); const [exportPassphrase, setExportPassphrase] = useState(""); const [exportScopes, setExportScopes] = useState(defaultBackupScopes()); const [plaintext, setPlaintext] = useState(false); const [confirmPlaintext, setConfirmPlaintext] = useState(false); const [importPassphrase, setImportPassphrase] = useState(""); const [masterKey, setMasterKey] = useState(""); const [file, setFile] = useState<File | null>(null); const [archiveKind, setArchiveKind] = useState<ArchiveKind | null>(null); const [replace, setReplace] = useState(false); const [busy, setBusy] = useState<"export" | "import" | null>(null)
  useEffect(() => { api.backupCapabilities().then(setCapabilities).catch(() => notify(t("genericError"), true)).finally(() => setLoading(false)) }, [notify, t])
  async function chooseArchive(selected: File | null) { setFile(selected); setArchiveKind(selected ? await detectArchiveKind(selected) : null); setImportPassphrase(""); setMasterKey("") }
  async function exportData(event: FormEvent) { event.preventDefault(); setBusy("export"); try { const blob = await api.export({ passphrase: plaintext ? undefined : exportPassphrase, scopes: exportScopes, plaintext, confirmPlaintext }); const url = URL.createObjectURL(blob); const anchor = document.createElement("a"); anchor.href = url; anchor.download = `lightbws-${new Date().toISOString().slice(0, 10)}${plaintext ? ".plain" : ""}.lightbws`; anchor.click(); URL.revokeObjectURL(url); notify(t("exportComplete")) } catch { notify(t("genericError"), true) } finally { setBusy(null) } }
  async function importData(event: FormEvent) { event.preventDefault(); if (!file) return; if (replace && !confirm(t("confirmReplaceImport"))) return; setBusy("import"); try { const dataBase64 = await fileBase64(file); await api.import({ passphrase: archiveKind === "passphrase" ? importPassphrase : undefined, masterKey: archiveKind === "masterKey" ? masterKey : undefined, dataBase64, replace }); notify(t("importComplete")); setFile(null); setArchiveKind(null) } catch { notify(t("genericError"), true) } finally { setBusy(null) } }
  const importCredentialReady = archiveKind === "plaintext" || (archiveKind === "passphrase" && importPassphrase.length >= 12) || (archiveKind === "masterKey" && masterKey.length > 0)
  return <div className="page"><PageHeader title={t("transfer")} />{loading ? <PageSkeleton variant="cards" /> : <section className="transfer-grid"><article className="panel transfer-card"><span className="panel-icon"><FileDown /></span><h2>{t("exportTitle")}</h2><p>{t("exportText")}</p><form className="form-stack" onSubmit={exportData}><ScopePicker scopes={exportScopes} onChange={(key, checked) => setExportScopes(normalizeScopes({ ...exportScopes, [key]: checked }, key, checked))} onPreset={setExportScopes} /><LabelWithTip label={t("backupEncryption")} tip={t("manualEncryptionTip")} /><div className="choice-grid two"><button type="button" className="choice-card" data-selected={!plaintext} onClick={() => { setPlaintext(false); setConfirmPlaintext(false) }}><KeyRound /><span>{t("passphraseEncrypted")}</span>{!plaintext && <Check className="selection-check" />}</button>{capabilities.plaintextAllowed && <button type="button" className="choice-card danger-choice" data-selected={plaintext} onClick={() => setPlaintext(true)}><ArchiveRestore /><span>{t("plaintext")}</span>{plaintext && <Check className="selection-check" />}</button>}</div>{plaintext ? <><Banner status="warning" title={t("plaintextWarning")} /><CheckField label={t("confirmPlaintext")} checked={confirmPlaintext} onChange={setConfirmPlaintext} /></> : <Field label={t("passphrase")} value={exportPassphrase} onChange={setExportPassphrase} type="password" hint={t("passphraseHint")} />}<Button type="submit" variant="primary" icon={<FileDown />} label={t("downloadExport")} isLoading={busy === "export"} isDisabled={plaintext ? !confirmPlaintext : exportPassphrase.length < 12} /></form></article><article className="panel transfer-card"><span className="panel-icon"><FileUp /></span><h2>{t("importTitle")}</h2><p>{t("importText")}</p><form className="form-stack" onSubmit={importData}><label className="field"><span>{t("chooseFile")}</span><input type="file" accept=".lightbws,application/vnd.lightbws.backup" onChange={(event) => void chooseArchive(event.target.files?.[0] || null)} /></label>{archiveKind && <Banner status={archiveKind === "plaintext" ? "warning" : "info"} title={t(archiveKind === "passphrase" ? "passphraseArchive" : archiveKind === "masterKey" ? "automaticArchive" : archiveKind === "plaintext" ? "plaintextArchive" : "invalidArchive")} />}{archiveKind === "passphrase" && <Field label={t("passphrase")} value={importPassphrase} onChange={setImportPassphrase} type="password" hint={t("passphraseHint")} />}{archiveKind === "masterKey" && <><LabelWithTip label={t("oldMasterKey")} tip={t("oldMasterKeyTip")} /><Field label={t("oldMasterKey")} value={masterKey} onChange={setMasterKey} type="password" /><label className="field"><span>{t("chooseMasterKeyFile")}</span><input type="file" accept=".key,text/plain" onChange={(event) => void readMasterKey(event.target.files?.[0] || null).then(setMasterKey)} /></label></>}<CheckField label={t("replaceDatabase")} checked={replace} onChange={setReplace} />{replace && <Banner status="warning" title={t("replaceDatabaseWarning")} />}<Button type="submit" variant="primary" icon={<FileUp />} label={t("importFile")} isLoading={busy === "import"} isDisabled={!file || archiveKind === "unknown" || !importCredentialReady} /></form></article></section>}</div>
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

function ListToolbar({ query, onQuery, resultCount, totalCount }: { query: string; onQuery: (value: string) => void; resultCount?: number; totalCount?: number }) { const { t } = useI18n(); return <div className="list-toolbar"><Search /><input aria-label={t("search")} placeholder={`${t("search")}…`} value={query} onChange={(event) => onQuery(event.target.value)} />{resultCount !== undefined && totalCount !== undefined && <span aria-live="polite">{resultCount}/{totalCount} {t("results")}</span>}</div> }

function DataPanel({ empty, emptyIcon, emptyTitle, emptyDescription, emptyActionLabel, onEmptyAction, children }: { empty: boolean; emptyIcon?: ReactNode; emptyTitle?: string; emptyDescription?: string; emptyActionLabel?: string; onEmptyAction?: () => void; children?: ReactNode }) {
  const { t } = useI18n(); return <section className="data-panel">{empty ? <div className="empty-state"><span>{emptyIcon || <Boxes />}</span><h2>{emptyTitle || t("emptyTitle")}</h2><p>{emptyDescription || t("emptyDescription")}</p>{onEmptyAction && <Button variant="secondary" icon={<Plus />} label={emptyActionLabel || t("create")} onClick={onEmptyAction} />}</div> : <div className="table-scroll">{children}</div>}</section>
}

function Modal({ title, onClose, wide = false, children }: { title: string; onClose: () => void; wide?: boolean; children: ReactNode }) {
  const backdrop = useRef<HTMLDivElement>(null)
  const dialog = useRef<HTMLElement>(null)
  const previousFocus = useRef<HTMLElement | null>(document.activeElement instanceof HTMLElement ? document.activeElement : null)
  const titleId = useId()
  useEffect(() => {
    const frame = requestAnimationFrame(() => {
      const node = dialog.current
      if (node && !node.contains(document.activeElement)) focusableElements(node)[0]?.focus()
    })
    const key = (event: KeyboardEvent) => {
      const layers = document.querySelectorAll(".modal-backdrop")
      if (layers.item(layers.length - 1) !== backdrop.current) return
      if (event.key === "Escape") { event.preventDefault(); onClose(); return }
      if (event.key !== "Tab" || !dialog.current) return
      const elements = focusableElements(dialog.current)
      if (!elements.length) return
      const first = elements[0]
      const last = elements[elements.length - 1]
      if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus() }
      else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus() }
    }
    addEventListener("keydown", key)
    return () => { cancelAnimationFrame(frame); removeEventListener("keydown", key); previousFocus.current?.focus() }
  }, [onClose])
  const { t } = useI18n()
  return <div ref={backdrop} className="modal-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose() }}><section ref={dialog} className="modal" data-wide={wide} role="dialog" aria-modal="true" aria-labelledby={titleId}><header><h2 id={titleId}>{title}</h2><IconButton variant="ghost" label={t("close")} icon={<X />} onClick={onClose} /></header>{children}</section></div>
}

function DialogActions({ onClose, busy, disabled }: { onClose: () => void; busy: boolean; disabled?: boolean }) { const { t } = useI18n(); return <div className="dialog-actions"><Button variant="ghost" label={t("cancel")} onClick={onClose} isDisabled={busy} /><Button type="submit" variant="primary" label={t("save")} isLoading={busy} isDisabled={disabled} /></div> }

function NameDialog({ title, label, type = "text", initialValue = "", onClose, onSave }: { title: string; label: string; type?: string; initialValue?: string; onClose: () => void; onSave: (value: string) => Promise<void> }) {
  const { t } = useI18n(); const [value, setValue] = useState(initialValue); const [busy, setBusy] = useState(false); const [touched, setTouched] = useState(false); const [formError, setFormError] = useState<string | null>(null)
  const error = type === "password" ? passwordValidation(value, t) : textValidation(value, 128, "nameRequirement", t)
  async function submit(event: FormEvent) { event.preventDefault(); if (busy) return; setTouched(true); if (error) return; setBusy(true); setFormError(null); try { await onSave(type === "password" ? value : value.trim()) } catch (caught) { setFormError(apiErrorText(caught, t)) } finally { setBusy(false) } }
  return <Modal title={title} onClose={onClose}><form className="form-stack" onSubmit={submit} noValidate>{formError && <Banner status="error" title={formError} />}<Field label={label} value={value} onChange={setValue} onBlur={() => setTouched(true)} type={type} autoComplete={type === "password" ? "new-password" : undefined} autoFocus required hint={type === "password" ? t("passwordRequirement") : t("nameRequirement")} error={touched ? error : undefined} /><DialogActions onClose={onClose} busy={busy} disabled={Boolean(error)} /></form></Modal>
}

interface FieldProps { label: string; value: string; onChange: (value: string) => void; type?: string; placeholder?: string; autoComplete?: string; autoFocus?: boolean; disabled?: boolean; hint?: string; error?: string; required?: boolean; onBlur?: () => void; min?: string; max?: string; inputMode?: "numeric" | "decimal" | "text" | "url" }
function Field({ label, value, onChange, type = "text", placeholder, autoComplete, autoFocus, disabled, hint, error, required, onBlur, min, max, inputMode }: FieldProps) {
  const { t } = useI18n(); const id = useId(); const messageId = `${id}-message`; const [passwordVisible, setPasswordVisible] = useState(false); const isPassword = type === "password"; const input = <input id={id} type={isPassword && passwordVisible ? "text" : type} value={value} placeholder={placeholder} autoComplete={autoComplete} autoFocus={autoFocus} disabled={disabled} aria-invalid={Boolean(error) || undefined} aria-describedby={(error || hint) ? messageId : undefined} aria-required={required || undefined} min={min} max={max} inputMode={inputMode} onBlur={onBlur} onChange={(event) => onChange(event.target.value)} />
  return <div className="field" data-error={Boolean(error) || undefined}><label htmlFor={id}>{label}{required && <span className="required-mark"> · {t("required")}</span>}</label>{isPassword ? <div className="password-input">{input}<IconButton variant="ghost" label={t(passwordVisible ? "hidePassword" : "showPassword")} icon={passwordVisible ? <EyeOff /> : <Eye />} onClick={() => setPasswordVisible((visible) => !visible)} /></div> : input}{(error || hint) && <small id={messageId} role={error ? "alert" : undefined}>{error || hint}</small>}</div>
}
function TextArea({ label, value, onChange }: { label: string; value: string; onChange: (value: string) => void }) { const id = useId(); return <div className="field"><label htmlFor={id}>{label}</label><textarea id={id} value={value} onChange={(event) => onChange(event.target.value)} rows={3} /></div> }
function SelectField({ label, value, onChange, options, disabled }: { label: string; value: string; onChange: (value: string) => void; options: Array<{ value: string; label: string }>; disabled?: boolean }) { const id = useId(); return <div className="field"><label htmlFor={id}>{label}</label><select id={id} value={value} disabled={disabled} onChange={(event) => onChange(event.target.value)}>{options.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}</select></div> }
function CheckField({ label, checked, onChange, disabled }: { label: string; checked: boolean; onChange: (value: boolean) => void; disabled?: boolean }) { return <label className="check-field"><input type="checkbox" checked={checked} disabled={disabled} onChange={(event) => onChange(event.target.checked)} /><span>{label}</span></label> }
function RowActions({ children }: { children: ReactNode }) { return <div className="row-actions">{children}</div> }
function StatusPill({ value }: { value: string }) { const { t } = useI18n(); const key = (["active", "disabled", "revoked", "expired", "succeeded", "failed", "running", "allowed", "denied", "changed", "readOnly", "readWrite"] as MessageKey[]).includes(value as MessageKey) ? value as MessageKey : "active"; return <span className="status-pill" data-status={value}><i />{t(key)}</span> }

function ConfirmActionDialog({ title, description, actionLabel, busy, onAction, onClose }: { title: string; description: string; actionLabel: string; busy: boolean; onAction: () => void; onClose: () => void }) {
  const { t } = useI18n()
  return <AlertDialog isOpen onOpenChange={(open) => { if (!open && !busy) onClose() }} title={title} description={description} cancelLabel={t("cancel")} actionLabel={actionLabel} actionVariant="destructive" isActionLoading={busy} onAction={onAction} />
}

function textValidation(value: string, max: number, requirement: MessageKey, t: (key: MessageKey) => string) {
  const trimmed = value.trim()
  if (!trimmed) return t("requiredField")
  if ([...trimmed].length > max || [...trimmed].some((character) => /[\u0000-\u001f\u007f-\u009f]/u.test(character))) return t(requirement)
  return undefined
}

function passwordValidation(value: string, t: (key: MessageKey) => string) {
  if (!value) return t("requiredField")
  if ([...value].length < 6 || [...value].length > 4096 || [...value].some((character) => /[\u0000-\u001f\u007f-\u009f]/u.test(character))) return t("passwordRequirement")
  return undefined
}

function apiErrorText(error: unknown, t: (key: MessageKey) => string) {
  if (error instanceof ApiError) {
    if (error.status === 409) return t("conflictError")
    if (error.status === 422) return t("validationError")
    if (error.status === 401 || error.status === 403) return t("permissionError")
  }
  return t("genericError")
}

function focusableElements(node: HTMLElement) {
  return [...node.querySelectorAll<HTMLElement>('button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), a[href], [tabindex]:not([tabindex="-1"])')].filter((element) => !element.hidden && element.getAttribute("aria-hidden") !== "true")
}

function formatDate(value: number) { return new Intl.DateTimeFormat(document.documentElement.lang || "en", { dateStyle: "medium", timeStyle: "short" }).format(new Date(value * 1000)) }
function minimumLocalDateTime() { const date = new Date(Date.now() + 60_000); date.setMinutes(date.getMinutes() - date.getTimezoneOffset()); return date.toISOString().slice(0, 16) }
function grantKey(bucket: GrantBucketKey, id: string) { return `${bucket}:${id}` }
function shortId(value: string | null) { return value ? value.slice(0, 8) : "—" }
function validPage(value: string): Page | null { return (["dashboard", "projects", "secrets", "machines", "users", "groups", "audit", "trash", "integrations", "help", "backups", "transfer", "settings"] as Page[]).includes(value as Page) ? value as Page : null }
function availablePage(value: string, isAdmin: boolean): Page {
  const page = validPage(value) || "dashboard"
  return !isAdmin && adminPages.has(page) ? "dashboard" : page
}
function fileBase64(file: File): Promise<string> { return new Promise((resolve, reject) => { const reader = new FileReader(); reader.onerror = () => reject(reader.error); reader.onload = () => resolve(String(reader.result).split(",")[1] || ""); reader.readAsDataURL(file) }) }
