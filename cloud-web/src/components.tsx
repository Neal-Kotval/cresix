import type { PropsWithChildren, ReactNode } from "react";
import {
  ArrowRight,
  Boxes,
  ChevronRight,
  Cloud,
  GitBranch,
  LogOut,
  Plus,
  Radio,
  Server,
  Settings,
} from "lucide-react";
import type { InstallationState } from "./types";

export function Brand({ compact = false }: { compact?: boolean }) {
  return (
    <a className="brand" href="/app" aria-label="Cresix Cloud home">
      <span className="brand-mark" aria-hidden="true">
        C6
      </span>
      {!compact && <span>Cresix</span>}
    </a>
  );
}

export function PreviewBanner() {
  return (
    <div className="preview-banner" role="status">
      <span>Preview data</span>
      <span aria-hidden="true">·</span>
      Changes are not saved or sent to a C6 installation.
    </div>
  );
}

export function Status({ state }: { state: InstallationState }) {
  return (
    <span className={`status status-${state}`}>
      <span className="status-dot" aria-hidden="true" />
      {state}
    </span>
  );
}

export function RouteSeam({
  account,
  workspace,
  project,
  route,
}: {
  account?: string;
  workspace: string;
  project?: string;
  route?: string;
}) {
  return (
    <div className="route-seam" aria-label="Cresix route">
      {account && (
        <>
          <span className="route-node">@{account}</span>
          <ChevronRight aria-hidden="true" />
        </>
      )}
      <span className="route-node">{workspace}</span>
      {project && (
        <>
          <ChevronRight aria-hidden="true" />
          <span className="route-node route-project">{project}</span>
        </>
      )}
      {route && (
        <>
          <ArrowRight className="route-arrow" aria-hidden="true" />
          <span className="route-port">{route}.relay.cresix.com</span>
        </>
      )}
    </div>
  );
}

export function AppShell({
  children,
  handle,
  preview,
  onSignOut,
}: PropsWithChildren<{ handle: string; preview: boolean; onSignOut: () => void }>) {
  return (
    <div className="app-shell">
      {preview && <PreviewBanner />}
      <header className="app-header">
        <Brand />
        <nav aria-label="Account">
          <span className="account-handle">@{handle}</span>
          <button className="icon-button" type="button" onClick={onSignOut} aria-label="Sign out">
            <LogOut aria-hidden="true" />
          </button>
        </nav>
      </header>
      <div className="app-layout">
        <aside className="sidebar">
          <nav aria-label="Cloud navigation">
            <a className="nav-item" href="/app">
              <Boxes aria-hidden="true" /> Workspaces
            </a>
            <a className="nav-item" href="/app/installations/new">
              <Server aria-hidden="true" /> Installations
            </a>
            <a className="nav-item" href="/app/settings">
              <Settings aria-hidden="true" /> Account
            </a>
          </nav>
          <p className="sidebar-note">
            Cresix routes to your C6. Your repositories and runs stay there.
          </p>
        </aside>
        <main id="main-content" className="app-main">
          {children}
        </main>
      </div>
    </div>
  );
}

export function PageTitle({
  eyebrow,
  children,
  action,
}: PropsWithChildren<{ eyebrow?: string; action?: ReactNode }>) {
  return (
    <div className="page-title">
      <div>
        {eyebrow && <p className="eyebrow">{eyebrow}</p>}
        <h1>{children}</h1>
      </div>
      {action}
    </div>
  );
}

export function RepositoryMeta({ branch, sha }: { branch: string; sha: string }) {
  return (
    <span className="repo-meta">
      <span>
        <GitBranch aria-hidden="true" /> {branch}
      </span>
      <code>{sha.slice(0, 8)}</code>
    </span>
  );
}

export function EmptyState({
  icon = "workspace",
  title,
  children,
  href,
  action,
}: PropsWithChildren<{
  icon?: "workspace" | "installation";
  title: string;
  href: string;
  action: string;
}>) {
  const Icon = icon === "workspace" ? Cloud : Radio;
  return (
    <div className="empty-state">
      <Icon aria-hidden="true" />
      <h2>{title}</h2>
      <p>{children}</p>
      <a className="button button-primary" href={href}>
        <Plus aria-hidden="true" /> {action}
      </a>
    </div>
  );
}
