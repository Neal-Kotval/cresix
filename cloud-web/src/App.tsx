import { useEffect, useMemo, useState, type FormEvent, type ReactNode } from "react";
import {
  ArrowLeft,
  ArrowUpRight,
  Check,
  CircleAlert,
  Clipboard,
  Clock3,
  GitBranch,
  Laptop,
  Link2,
  LockKeyhole,
  Plus,
  RotateCcw,
  Server,
  ShieldCheck,
  Unplug,
} from "lucide-react";
import { ApiError, cloudApi } from "./api";
import {
  AppShell,
  Brand,
  EmptyState,
  PageTitle,
  PreviewBanner,
  RepositoryMeta,
  RouteSeam,
  Status,
} from "./components";
import {
  previewDirectory,
  previewInstallations,
  previewSession,
  previewWorkspaces,
} from "./fixtures";
import type {
  DirectoryEntry,
  Installation,
  InstallationCreated,
  Session,
  Workspace,
} from "./types";

type AppRoute =
  | { kind: "claim" }
  | { kind: "dashboard" }
  | { kind: "new-workspace" }
  | { kind: "new-installation" }
  | { kind: "bind"; workspaceId: string }
  | { kind: "settings" }
  | { kind: "directory"; namespace: string; project: string }
  | { kind: "not-found" };

function currentRoute(pathname: string): AppRoute {
  const parts = pathname.split("/").filter(Boolean).map(decodeURIComponent);
  if (pathname === "/" || pathname === "/claim") return { kind: "claim" };
  if (pathname === "/app") return { kind: "dashboard" };
  if (pathname === "/app/workspaces/new") return { kind: "new-workspace" };
  if (pathname === "/app/installations/new") return { kind: "new-installation" };
  if (pathname === "/app/settings") return { kind: "settings" };
  if (parts[0] === "app" && parts[1] === "workspaces" && parts[3] === "bind") {
    return { kind: "bind", workspaceId: parts[2] };
  }
  if (parts.length === 2 && parts[0] !== "api") {
    return { kind: "directory", namespace: parts[0], project: parts[1] };
  }
  return { kind: "not-found" };
}

function previewEnabled() {
  const params = new URLSearchParams(window.location.search);
  return params.has("preview") || import.meta.env.VITE_CRESIX_PREVIEW === "true";
}

function friendlyTime(value?: string) {
  if (!value) return "Never connected";
  const date = new Date(value);
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(date);
}

function messageFrom(error: unknown) {
  if (error instanceof ApiError) return error.message;
  if (error instanceof Error) return error.message;
  return "The request could not be completed.";
}

export function App() {
  const preview = previewEnabled();
  const route = currentRoute(window.location.pathname);
  const [session, setSession] = useState<Session | null>(preview ? previewSession : null);
  const [loading, setLoading] = useState(!preview && route.kind !== "directory");

  useEffect(() => {
    if (preview || route.kind === "directory" || route.kind === "claim") return;
    cloudApi
      .session()
      .then(setSession)
      .catch(() => setSession(null))
      .finally(() => setLoading(false));
  }, [preview, route.kind]);

  if (route.kind === "directory") {
    return <DirectoryPage namespace={route.namespace} project={route.project} preview={preview} />;
  }
  if (route.kind === "claim") {
    return <ClaimPage preview={preview} onClaim={setSession} />;
  }
  if (loading) return <LoadingScreen />;
  if (!session) return <SignInRequired />;

  const signOut = async () => {
    if (!preview) await cloudApi.signOut(session.csrfToken).catch(() => undefined);
    window.location.assign(preview ? "/claim?preview=1" : "/claim");
  };

  return (
    <AppShell handle={session.account.handle} preview={preview} onSignOut={signOut}>
      {route.kind === "dashboard" && <Dashboard session={session} preview={preview} />}
      {route.kind === "new-workspace" && <NewWorkspace session={session} preview={preview} />}
      {route.kind === "new-installation" && (
        <NewInstallation session={session} preview={preview} />
      )}
      {route.kind === "bind" && (
        <BindWorkspace session={session} workspaceId={route.workspaceId} preview={preview} />
      )}
      {route.kind === "settings" && <AccountPage session={session} />}
      {route.kind === "not-found" && <NotFound />}
    </AppShell>
  );
}

function LoadingScreen() {
  return (
    <main className="centered-page" aria-busy="true">
      <Brand />
      <p>Loading your Cresix directory…</p>
    </main>
  );
}

function SignInRequired() {
  return (
    <main className="centered-page">
      <Brand />
      <LockKeyhole aria-hidden="true" />
      <h1>This directory needs an account</h1>
      <p>No active Cresix Cloud session was found.</p>
      <a className="button button-primary" href="/claim">
        Open account setup
      </a>
    </main>
  );
}

function ClaimPage({ preview, onClaim }: { preview: boolean; onClaim: (session: Session) => void }) {
  const [proof, setProof] = useState("");
  const [handle, setHandle] = useState(preview ? "neal" : "");
  const [displayName, setDisplayName] = useState(preview ? "Neal Kotval" : "");
  const [error, setError] = useState("");
  const [submitting, setSubmitting] = useState(false);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setSubmitting(true);
    setError("");
    try {
      const claimed = preview
        ? { ...previewSession, account: { ...previewSession.account, handle, displayName } }
        : await cloudApi.claim(proof, handle, displayName);
      onClaim(claimed);
      window.location.assign(preview ? "/app?preview=1" : "/app");
    } catch (nextError) {
      setError(messageFrom(nextError));
      setSubmitting(false);
    }
  };

  return (
    <div className="claim-page">
      {preview && <PreviewBanner />}
      <header className="claim-header">
        <Brand />
        <span className="dogfood-label">Dogfood control plane</span>
      </header>
      <main className="claim-layout">
        <section className="claim-thesis" aria-labelledby="claim-title">
          <p className="eyebrow">The global doorway to local C6</p>
          <h1 id="claim-title">Give small software a place people can find.</h1>
          <p className="claim-copy">
            Cresix reserves the name and keeps the route open. Your code, runs, credentials, and
            permissions stay on the C6 installation you control.
          </p>
          <RouteSeam
            account={handle || "you"}
            workspace="your-workspace"
            project="small-tool"
            route="route-••••"
          />
          <ul className="trust-list">
            <li>
              <ShieldCheck aria-hidden="true" /> Standalone C6 keeps working when Cloud disconnects.
            </li>
            <li>
              <Laptop aria-hidden="true" /> A laptop can connect outbound without opening a router port.
            </li>
            <li>
              <LockKeyhole aria-hidden="true" /> Local C6 authentication remains authoritative.
            </li>
          </ul>
        </section>
        <form className="claim-panel" onSubmit={submit} aria-label="Claim Cloud preview">
          <p className="eyebrow">First account</p>
          <h2>Claim this Cloud</h2>
          <p className="form-intro">
            This loopback-only setup creates the first owner. It is not the production cresix.com
            login flow.
          </p>
          {!preview && (
            <Field label="Bootstrap proof" hint="Read it from the Cloud server’s owner-only data file.">
              <input
                required
                autoComplete="off"
                name="proof"
                type="password"
                value={proof}
                onChange={(event) => setProof(event.target.value)}
              />
            </Field>
          )}
          <Field label="Account handle" hint="Lowercase letters, numbers, and hyphens.">
            <div className="prefixed-input">
              <span aria-hidden="true">@</span>
              <input
                required
                pattern="[a-z0-9](?:[a-z0-9-]{0,37}[a-z0-9])?"
                name="handle"
                autoComplete="username"
                value={handle}
                onChange={(event) => setHandle(event.target.value.toLowerCase())}
              />
            </div>
          </Field>
          <Field label="Display name">
            <input
              required
              name="displayName"
              autoComplete="name"
              value={displayName}
              onChange={(event) => setDisplayName(event.target.value)}
            />
          </Field>
          {error && <InlineError>{error}</InlineError>}
          <button className="button button-primary button-wide" disabled={submitting} type="submit">
            {submitting ? "Claiming…" : preview ? "Enter preview" : "Claim Cresix Cloud"}
          </button>
        </form>
      </main>
    </div>
  );
}

function Dashboard({ session, preview }: { session: Session; preview: boolean }) {
  const [workspaces, setWorkspaces] = useState<Workspace[]>(preview ? previewWorkspaces : []);
  const [installations, setInstallations] = useState<Installation[]>(
    preview ? previewInstallations : [],
  );
  const [error, setError] = useState("");

  useEffect(() => {
    if (preview) return;
    Promise.all([cloudApi.workspaces(), cloudApi.installations()])
      .then(([nextWorkspaces, nextInstallations]) => {
        setWorkspaces(nextWorkspaces);
        setInstallations(nextInstallations);
      })
      .catch((nextError) => setError(messageFrom(nextError)));
  }, [preview]);

  const installationById = useMemo(
    () => new Map(installations.map((installation) => [installation.id, installation])),
    [installations],
  );

  return (
    <>
      <PageTitle
        eyebrow={`@${session.account.handle}`}
        action={
          <a className="button button-primary" href={preview ? "/app/workspaces/new?preview=1" : "/app/workspaces/new"}>
            <Plus aria-hidden="true" /> New workspace
          </a>
        }
      >
        Workspaces
      </PageTitle>
      {error && <InlineError>{error}</InlineError>}
      {workspaces.length === 0 ? (
        <EmptyState title="Name your first workspace" href="/app/workspaces/new" action="New workspace">
          A workspace is the stable address people use to find projects on a C6 installation.
        </EmptyState>
      ) : (
        <section className="directory-list" aria-label="Workspaces">
          {workspaces.map((workspace) => {
            const installation = workspace.binding
              ? installationById.get(workspace.binding.installationId)
              : undefined;
            return (
              <article className="workspace-row" key={workspace.id}>
                <div className="row-main">
                  <div className="row-heading">
                    <a
                      className="workspace-name"
                      href={
                        workspace.projects[0]
                          ? `/${workspace.namespace}/${workspace.projects[0].slug}${preview ? "?preview=1" : ""}`
                          : undefined
                      }
                    >
                      {workspace.namespace}
                    </a>
                    <span className="role-label">{workspace.role}</span>
                  </div>
                  <p>{workspace.name}</p>
                  <RouteSeam
                    account={session.account.handle}
                    workspace={workspace.namespace}
                    route={installation?.routeId}
                  />
                </div>
                <div className="row-status">
                  {installation ? <Status state={installation.state} /> : <span>Not bound</span>}
                  <span>{workspace.projects.length} projects</span>
                  {!workspace.binding && (
                    <a href={`/app/workspaces/${workspace.id}/bind${preview ? "?preview=1" : ""}`}>
                      Bind installation
                    </a>
                  )}
                </div>
              </article>
            );
          })}
        </section>
      )}

      <div className="section-heading">
        <div>
          <p className="eyebrow">Outbound routes</p>
          <h2>Installations</h2>
        </div>
        <a className="text-link" href={preview ? "/app/installations/new?preview=1" : "/app/installations/new"}>
          Register installation <Plus aria-hidden="true" />
        </a>
      </div>
      <section className="installation-list" aria-label="Installations">
        {installations.map((installation) => (
          <article className="installation-row" key={installation.id}>
            <Server aria-hidden="true" />
            <div>
              <strong>{installation.label}</strong>
              <code>{installation.routeId}.relay.cresix.com</code>
            </div>
            <div>
              <Status state={installation.state} />
              <span className="last-seen">Last seen {friendlyTime(installation.lastSeenAt)}</span>
            </div>
          </article>
        ))}
      </section>
    </>
  );
}

function NewWorkspace({ session, preview }: { session: Session; preview: boolean }) {
  const [namespace, setNamespace] = useState("");
  const [name, setName] = useState("");
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setSaving(true);
    setError("");
    try {
      if (!preview) await cloudApi.createWorkspace(session.csrfToken, namespace, name);
      window.location.assign(preview ? "/app?preview=1" : "/app");
    } catch (nextError) {
      setError(messageFrom(nextError));
      setSaving(false);
    }
  };

  return (
    <FormPage title="Create a workspace" eyebrow="Global namespace" back="/app">
      <p className="form-intro">
        The namespace becomes <code>cresix.com/{namespace || "your-workspace"}/…</code>. Renaming is
        disabled during dogfood.
      </p>
      <form className="stack-form" onSubmit={submit}>
        <Field label="Namespace" hint="Globally unique. Lowercase letters, numbers, and hyphens.">
          <div className="suffixed-input">
            <span>cresix.com/</span>
            <input
              required
              pattern="[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?"
              autoFocus
              value={namespace}
              onChange={(event) => setNamespace(event.target.value.toLowerCase())}
            />
          </div>
        </Field>
        <Field label="Workspace name" hint="A human-readable label; it can change later.">
          <input required value={name} onChange={(event) => setName(event.target.value)} />
        </Field>
        <Callout icon={<Link2 aria-hidden="true" />}>
          Creating a workspace reserves its address. It does not grant access to any local C6 data.
        </Callout>
        {error && <InlineError>{error}</InlineError>}
        <FormActions saving={saving} action="Create workspace" preview={preview} />
      </form>
    </FormPage>
  );
}

function NewInstallation({ session, preview }: { session: Session; preview: boolean }) {
  const [label, setLabel] = useState("");
  const [serverId, setServerId] = useState("");
  const [created, setCreated] = useState<InstallationCreated | null>(null);
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setSaving(true);
    setError("");
    try {
      const result = preview
        ? {
            ...previewInstallations[0],
          id: "install-preview-new",
            label,
            localServerId: serverId,
            routeId: "route-k4d9",
            connectorCredential: "c6c_preview_7qD4wM3K-example-only",
          }
        : await cloudApi.createInstallation(session.csrfToken, label, serverId);
      setCreated(result);
    } catch (nextError) {
      setError(messageFrom(nextError));
    } finally {
      setSaving(false);
    }
  };

  const copyCredential = async () => {
    if (!created) return;
    await navigator.clipboard.writeText(created.connectorCredential);
    setCopied(true);
  };

  if (created) {
    return (
      <FormPage title="Save the connector credential" eyebrow="Shown once" back="/app">
        <div className="credential-warning">
          <LockKeyhole aria-hidden="true" />
          <div>
            <strong>This is the only reveal.</strong>
            <p>
              Cresix stores a verifier, not this credential. Save it to an owner-only file before
              leaving this page.
            </p>
          </div>
        </div>
        <div className="credential-box">
          <code data-testid="connector-credential">{created.connectorCredential}</code>
          <button className="button button-secondary" type="button" onClick={copyCredential}>
            {copied ? <Check aria-hidden="true" /> : <Clipboard aria-hidden="true" />}
            {copied ? "Copied" : "Copy"}
          </button>
        </div>
        <dl className="definition-list">
          <div>
            <dt>Route</dt>
            <dd>{created.routeId}.relay.cresix.com</dd>
          </div>
          <div>
            <dt>Local server ID</dt>
            <dd>{created.localServerId}</dd>
          </div>
        </dl>
        <Callout icon={<ShieldCheck aria-hidden="true" />}>
          The connector is limited to its configured loopback C6 origin. The relay still terminates
          TLS and can observe relayed traffic.
        </Callout>
        <a className="button button-primary" href={preview ? "/app?preview=1" : "/app"}>
          I saved the credential
        </a>
      </FormPage>
    );
  }

  return (
    <FormPage title="Register a C6 installation" eyebrow="One outbound route" back="/app">
      <p className="form-intro">
        One installation can serve several workspaces. Registration creates a stable opaque route;
        it does not copy repositories into Cloud.
      </p>
      <form className="stack-form" onSubmit={submit}>
        <Field label="Installation label" hint="A name you will recognize, such as “Studio mini”.">
          <input required autoFocus value={label} onChange={(event) => setLabel(event.target.value)} />
        </Field>
        <Field label="Local server ID" hint="Copy the immutable server ID shown by your local C6.">
          <input
            required
            pattern="[0-9a-fA-F-]{36}"
            placeholder="00000000-0000-0000-0000-000000000000"
            autoComplete="off"
            value={serverId}
            onChange={(event) => setServerId(event.target.value)}
          />
        </Field>
        {error && <InlineError>{error}</InlineError>}
        <FormActions saving={saving} action="Register installation" preview={preview} />
      </form>
    </FormPage>
  );
}

function BindWorkspace({
  session,
  workspaceId,
  preview,
}: {
  session: Session;
  workspaceId: string;
  preview: boolean;
}) {
  const [installations, setInstallations] = useState<Installation[]>(
    preview ? previewInstallations : [],
  );
  const [installationId, setInstallationId] = useState(previewInstallations[0].id);
  const [localWorkspaceId, setLocalWorkspaceId] = useState("");
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (preview) return;
    cloudApi
      .installations()
      .then((items) => {
        setInstallations(items);
        setInstallationId(items[0]?.id ?? "");
      })
      .catch((nextError) => setError(messageFrom(nextError)));
  }, [preview]);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setSaving(true);
    setError("");
    try {
      if (!preview) {
        await cloudApi.bindWorkspace(
          session.csrfToken,
          workspaceId,
          installationId,
          localWorkspaceId,
        );
      }
      window.location.assign(preview ? "/app?preview=1" : "/app");
    } catch (nextError) {
      setError(messageFrom(nextError));
      setSaving(false);
    }
  };

  return (
    <FormPage title="Bind the local workspace" eyebrow="Directory pointer" back="/app">
      <p className="form-intro">
        This records where the workspace lives. It does not synchronize Cloud membership into local
        C6 roles.
      </p>
      <form className="stack-form" onSubmit={submit}>
        <Field label="Installation">
          <select
            required
            value={installationId}
            onChange={(event) => setInstallationId(event.target.value)}
          >
            {installations.map((installation) => (
              <option key={installation.id} value={installation.id} disabled={installation.state === "revoked"}>
                {installation.label} · {installation.state}
              </option>
            ))}
          </select>
        </Field>
        <Field label="Local workspace UUID" hint="Use the UUID from C6, not its editable slug.">
          <input
            required
            pattern="[0-9a-fA-F-]{36}"
            placeholder="00000000-0000-0000-0000-000000000000"
            value={localWorkspaceId}
            onChange={(event) => setLocalWorkspaceId(event.target.value)}
          />
        </Field>
        <Callout icon={<Unplug aria-hidden="true" />}>
          Removing this binding later does not delete the local workspace or any repository.
        </Callout>
        {error && <InlineError>{error}</InlineError>}
        <FormActions saving={saving} action="Bind workspace" preview={preview} />
      </form>
    </FormPage>
  );
}

function DirectoryPage({ namespace, project, preview }: { namespace: string; project: string; preview: boolean }) {
  const forcedState = new URLSearchParams(window.location.search).get("state");
  const fixtureEntry = preview ? previewDirectory(namespace, project) : undefined;
  if (
    fixtureEntry &&
    (forcedState === "connected" || forcedState === "offline" || forcedState === "revoked")
  ) {
    fixtureEntry.installation.state = forcedState;
    fixtureEntry.relayUrl =
      forcedState === "connected"
        ? `https://${fixtureEntry.installation.routeId}.relay.cresix.com/projects/${fixtureEntry.project.slug}`
        : null;
  }
  const [entry, setEntry] = useState<DirectoryEntry | null>(
    fixtureEntry ?? null,
  );
  const [loading, setLoading] = useState(!preview);
  const [notFound, setNotFound] = useState(false);

  useEffect(() => {
    if (preview) return;
    cloudApi
      .directory(namespace, project)
      .then(setEntry)
      .catch(() => setNotFound(true))
      .finally(() => setLoading(false));
  }, [namespace, preview, project]);

  if (loading) return <LoadingScreen />;
  if (notFound || !entry) return <PublicNotFound namespace={namespace} project={project} preview={preview} />;
  const { installation } = entry;
  const relayTarget = entry.relayUrl ? new URL(entry.relayUrl, window.location.origin) : undefined;
  const pathRelayPreview = relayTarget?.origin === window.location.origin
    && relayTarget.pathname.startsWith("/relay/");
  return (
    <div className="directory-page">
      {preview && <PreviewBanner />}
      <header className="directory-header">
        <Brand />
        <a href={preview ? "/app?preview=1" : "/app"}>Your workspaces</a>
      </header>
      <main className="directory-main">
        <nav className="breadcrumbs" aria-label="Breadcrumb">
          <a href={`/${entry.workspace.namespace}/${entry.project.slug}${preview ? "?preview=1" : ""}`}>
            {entry.workspace.namespace}
          </a>
          <span aria-hidden="true">/</span>
          <strong>{entry.project.slug}</strong>
        </nav>
        <div className="directory-title">
          <div>
            <p className="eyebrow">Project doorway</p>
            <h1>{entry.project.name}</h1>
            <p>{entry.project.description}</p>
          </div>
          <Status state={installation.state} />
        </div>
        <RouteSeam
          workspace={entry.workspace.namespace}
          project={entry.project.slug}
          route={installation.routeId}
        />
        <section className="doorway-panel" aria-labelledby="doorway-title">
          <div className="doorway-project">
            <GitBranch aria-hidden="true" />
            <div>
              <h2 id="doorway-title">Repository on {installation.label}</h2>
              <RepositoryMeta branch={entry.project.defaultBranch} sha={entry.project.headSha} />
            </div>
          </div>
          {installation.state === "connected" && entry.relayUrl && pathRelayPreview ? (
            <>
              <div className="origin-notice">
                <ShieldCheck aria-hidden="true" />
                <p>
                  The loopback transport is connected. Browser opening is disabled because this
                  dogfood path shares the Cloud account origin and deliberately strips cookies.
                </p>
              </div>
              <button className="button button-primary" disabled>
                Isolated relay origin required
              </button>
            </>
          ) : installation.state === "connected" && entry.relayUrl ? (
            <>
              <div className="origin-notice">
                <ArrowUpRight aria-hidden="true" />
                <p>
                  You are leaving <strong>cresix.com</strong> for an isolated C6 origin. Sign-in and
                  permissions there are managed by that installation.
                </p>
              </div>
              <a className="button button-primary" href={entry.relayUrl} rel="noreferrer">
                Open on C6 <ArrowUpRight aria-hidden="true" />
              </a>
            </>
          ) : installation.state === "offline" ? (
            <StateMessage icon={<Clock3 aria-hidden="true" />} title="This installation is offline">
              The owner may have closed their laptop or stopped the connector. The directory entry
              remains, but Cresix will not guess or retry a write.
            </StateMessage>
          ) : (
            <StateMessage icon={<Unplug aria-hidden="true" />} title="This route was revoked">
              The installation owner disabled Cloud ingress. Ask them for a current way to reach the
              project.
            </StateMessage>
          )}
        </section>
        <p className="directory-footnote">
          Catalog revision from {friendlyTime(entry.project.updatedAt)}. Project metadata is a
          bounded directory projection; local C6 is authoritative.
        </p>
      </main>
    </div>
  );
}

function PublicNotFound({ namespace, project, preview }: { namespace: string; project: string; preview: boolean }) {
  return (
    <main className="centered-page">
      {preview && <PreviewBanner />}
      <Brand />
      <CircleAlert aria-hidden="true" />
      <h1>Project not found</h1>
      <p>
        There is no published directory entry for <code>{namespace}/{project}</code>.
      </p>
      <a href={preview ? "/app?preview=1" : "/app"}>Return to your workspaces</a>
    </main>
  );
}

function AccountPage({ session }: { session: Session }) {
  return (
    <>
      <PageTitle eyebrow="Cloud identity">Account</PageTitle>
      <dl className="definition-list settings-list">
        <div>
          <dt>Display name</dt>
          <dd>{session.account.displayName}</dd>
        </div>
        <div>
          <dt>Handle</dt>
          <dd>@{session.account.handle}</dd>
        </div>
        <div>
          <dt>Identity boundary</dt>
          <dd>This Cloud account does not sign you into local C6 installations.</dd>
        </div>
      </dl>
      <Callout icon={<CircleAlert aria-hidden="true" />}>
        The dogfood bootstrap has no production account recovery. Do not expose it to a hostile
        multi-tenant internet environment.
      </Callout>
    </>
  );
}

function NotFound() {
  return (
    <div className="empty-state">
      <CircleAlert aria-hidden="true" />
      <h1>Page not found</h1>
      <a className="button button-secondary" href="/app">
        Return to workspaces
      </a>
    </div>
  );
}

function FormPage({
  title,
  eyebrow,
  back,
  children,
}: {
  title: string;
  eyebrow: string;
  back: string;
  children: ReactNode;
}) {
  return (
    <div className="form-page">
      <a className="back-link" href={back}>
        <ArrowLeft aria-hidden="true" /> Back to workspaces
      </a>
      <PageTitle eyebrow={eyebrow}>{title}</PageTitle>
      {children}
    </div>
  );
}

function Field({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: ReactNode;
}) {
  const id = label.toLowerCase().replaceAll(/[^a-z0-9]+/g, "-");
  return (
    <label className="field">
      <span>{label}</span>
      {hint && <small id={`${id}-hint`}>{hint}</small>}
      <div className="field-control">{children}</div>
    </label>
  );
}

function FormActions({ saving, action, preview }: { saving: boolean; action: string; preview: boolean }) {
  return (
    <div className="form-actions">
      <button className="button button-primary" disabled={saving} type="submit">
        {saving ? "Saving…" : action}
      </button>
      {preview && <span>Preview only</span>}
    </div>
  );
}

function InlineError({ children }: { children: ReactNode }) {
  return (
    <div className="inline-error" role="alert">
      <CircleAlert aria-hidden="true" /> {children}
    </div>
  );
}

function Callout({ icon, children }: { icon: ReactNode; children: ReactNode }) {
  return (
    <div className="callout">
      {icon}
      <p>{children}</p>
    </div>
  );
}

function StateMessage({ icon, title, children }: { icon: ReactNode; title: string; children: ReactNode }) {
  return (
    <div className="state-message">
      {icon}
      <div>
        <h3>{title}</h3>
        <p>{children}</p>
      </div>
    </div>
  );
}
