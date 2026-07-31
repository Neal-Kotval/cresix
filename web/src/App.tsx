import {
  ArrowUpRight,
  BookOpen,
  Box,
  Braces,
  Check,
  ChevronDown,
  Clock3,
  Cloud,
  Code2,
  Copy,
  Database,
  GitBranch,
  GitFork,
  GitPullRequest,
  Globe2,
  History,
  MoreHorizontal,
  Play,
  Rocket,
  Search,
  Server,
  Share2,
  Sparkles,
  Users,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { api } from "./api";
import type { ProjectDetail, Run } from "./types";

type Tab = "overview" | "files" | "pulls" | "runs" | "settings";

const relative = new Intl.RelativeTimeFormat("en", { numeric: "auto" });

function relativeTime(value: string) {
  const minutes = Math.round((new Date(value).getTime() - Date.now()) / 60_000);
  if (Math.abs(minutes) < 60) return relative.format(minutes, "minute");
  const hours = Math.round(minutes / 60);
  if (Math.abs(hours) < 24) return relative.format(hours, "hour");
  return relative.format(Math.round(hours / 24), "day");
}

function shortSha(sha: string) {
  return sha.slice(0, 7);
}

export function App() {
  const [project, setProject] = useState<ProjectDetail>();
  const [tab, setTab] = useState<Tab>("overview");
  const [error, setError] = useState<string>();
  const [busy, setBusy] = useState<string>();
  const [notice, setNotice] = useState<string>();

  useEffect(() => {
    api.project("weeknote").then(setProject).catch((caught: Error) => setError(caught.message));
  }, []);

  const production = useMemo(
    () => project?.deployments.find((deployment) => deployment.environment === "production"),
    [project],
  );

  async function run(job: string, kind: Run["kind"]) {
    if (!project) return;
    setBusy(job);
    try {
      const created = await api.run(project.slug, job, kind);
      setProject({ ...project, runs: [created, ...project.runs] });
      setNotice(`${job} is queued.`);
    } catch (caught) {
      setNotice(caught instanceof Error ? caught.message : "The job could not be queued.");
    } finally {
      setBusy(undefined);
    }
  }

  async function publish() {
    if (!project) return;
    setBusy("publish");
    try {
      await api.publish(project.slug);
      setNotice(`Publishing ${shortSha(project.headSha)}.`);
    } catch (caught) {
      setNotice(caught instanceof Error ? caught.message : "The revision could not be published.");
    } finally {
      setBusy(undefined);
    }
  }

  if (error) {
    return <div className="center-state"><strong>C6 is not reachable.</strong><span>{error}</span><code>cargo run -p c6-server</code></div>;
  }
  if (!project) {
    return <div className="center-state"><span className="loader" />Loading your small software…</div>;
  }

  return (
    <div className="app-shell">
      <header className="topbar">
        <a className="brand" href="#" aria-label="C6 home">
          <span className="brand-mark"><Cloud size={18} strokeWidth={2.4} /></span>
          <span>C6</span>
        </a>
        <div className="global-search">
          <Search size={15} />
          <span>Find small software</span>
          <kbd>⌘ K</kbd>
        </div>
        <div className="top-actions">
          <span className="server-state"><span /> Laptop server</span>
          <button className="icon-button" aria-label="More options"><MoreHorizontal size={18} /></button>
          <button className="avatar" aria-label="Account menu">NK</button>
        </div>
      </header>

      <main>
        <section className="project-head">
          <div className="crumbs">
            <button>Paper Street</button><span>/</span><strong>{project.name}</strong>
          </div>
          <div className="project-title-row">
            <div>
              <div className="title-lockup">
                <span className="project-icon"><Braces size={24} /></span>
                <h1>{project.name}</h1>
                <span className="private-chip">Private</span>
              </div>
              <p>{project.description}</p>
            </div>
            <div className="project-actions">
              <button className="button secondary"><Share2 size={16} />Share</button>
              <button className="button secondary"><GitFork size={16} />Fork</button>
              <a className="button primary" href={project.appUrl} target="_blank" rel="noreferrer">
                Open app<ArrowUpRight size={16} />
              </a>
            </div>
          </div>
          <nav className="tabs" aria-label="Project sections">
            {([
              ["overview", BookOpen, "Overview"],
              ["files", Code2, "Files"],
              ["pulls", GitPullRequest, "Pull requests"],
              ["runs", Play, "Runs"],
              ["settings", Server, "Settings"],
            ] as const).map(([value, Icon, label]) => (
              <button key={value} className={tab === value ? "active" : ""} onClick={() => setTab(value)}>
                <Icon size={16} />{label}
                {value === "pulls" && <span className="count">{project.pullRequests.length}</span>}
              </button>
            ))}
          </nav>
        </section>

        {notice && <button className="notice" onClick={() => setNotice(undefined)}>{notice}<span>Dismiss</span></button>}

        <section className="content-grid">
          <div className="main-column">
            <div className="software-spine" aria-label="Software lifecycle">
              <LifecycleStep icon={Code2} label="Source" value={project.defaultBranch} detail={shortSha(project.headSha)} tone="ink" />
              <LifecycleStep icon={GitPullRequest} label="Review" value={`${project.pullRequests.length} open`} detail="preview ready" tone="blue" />
              <LifecycleStep icon={Rocket} label="Published" value={shortSha(project.publishedSha ?? "—")} detail="1 day ago" tone="green" />
              <LifecycleStep icon={Clock3} label="Next run" value="Fri 16:00" detail="friday-notes" tone="orange" last />
            </div>

            {tab === "overview" && (
              <>
                <section className="panel readme-panel">
                  <div className="panel-heading">
                    <span><BookOpen size={16} />README.md</span>
                    <button className="text-button">Edit</button>
                  </div>
                  <div className="readme-content">
                    <h2>Weeknote</h2>
                    <p>A small private app for making the weekly update less painful. It gathers activity, proposes a summary, and lets the team edit before sharing.</p>
                    <h3>What runs</h3>
                    <ul>
                      <li><code>web</code> serves the shared editor</li>
                      <li><code>sync-activity</code> refreshes source data hourly</li>
                      <li><code>friday-notes</code> uses Codex to propose the weekly summary and opens a pull request</li>
                    </ul>
                  </div>
                </section>

                <section className="panel">
                  <div className="panel-heading">
                    <span><History size={16} />Recent revisions</span>
                    <button className="text-button">View all</button>
                  </div>
                  <div className="list">
                    {project.revisions.map((revision) => (
                      <div className="revision-row" key={revision.sha}>
                        <span className="commit-node" />
                        <div><strong>{revision.message}</strong><small>{revision.author.handle} · {relativeTime(revision.createdAt)}</small></div>
                        <code>{shortSha(revision.sha)}</code>
                      </div>
                    ))}
                  </div>
                </section>
              </>
            )}

            {tab === "pulls" && <Pulls project={project} />}
            {tab === "runs" && <Runs project={project} onRun={run} busy={busy} />}
            {tab === "files" && <Files />}
            {tab === "settings" && <Settings />}
          </div>

          <aside className="side-column">
            <section className="side-section deploy-card">
              <div className="side-heading"><span>Production</span><span className="healthy"><Check size={12} />Live</span></div>
              <div className="deploy-visual"><Globe2 size={23} /><span /><Rocket size={19} /></div>
              <a href={production?.url} target="_blank" rel="noreferrer">weeknote.c6.local<ArrowUpRight size={14} /></a>
              <dl>
                <div><dt>Revision</dt><dd><code>{shortSha(production?.revisionSha ?? "—")}</code></dd></div>
                <div><dt>Published</dt><dd>1 day ago</dd></div>
                <div><dt>Access</dt><dd>6 people</dd></div>
              </dl>
              {project.headSha !== project.publishedSha && (
                <button className="button publish" onClick={publish} disabled={busy === "publish"}>
                  <Rocket size={15} />{busy === "publish" ? "Publishing…" : `Publish ${shortSha(project.headSha)}`}
                </button>
              )}
            </section>

            <section className="side-section">
              <div className="side-heading"><span>Software</span><button><MoreHorizontal size={16} /></button></div>
              <div className="resource-list">
                <Resource icon={Box} name="web" detail="Web · healthy" state="live" />
                <Resource icon={Clock3} name="sync-activity" detail="Every hour" />
                <Resource icon={Sparkles} name="friday-notes" detail="Agent · Fridays" state="agent" />
                <Resource icon={Database} name="postgres" detail="18 MB" />
              </div>
            </section>

            <section className="side-section share-card">
              <div className="side-heading"><span>People</span><Users size={15} /></div>
              <div className="people"><span>NK</span><span>AM</span><span>JP</span><span>+3</span></div>
              <p>Only invited people can use or inspect this software.</p>
              <button className="button secondary wide"><Share2 size={15} />Manage access</button>
            </section>

            <section className="clone-box">
              <label>Clone this software</label>
              <div><code>git@laptop:paper-street/weeknote.git</code><button aria-label="Copy clone URL"><Copy size={14} /></button></div>
            </section>
          </aside>
        </section>
      </main>
    </div>
  );
}

function LifecycleStep({ icon: Icon, label, value, detail, tone, last }: { icon: typeof Code2; label: string; value: string; detail: string; tone: string; last?: boolean }) {
  return <div className={`life-step ${tone}`}><span className="life-icon"><Icon size={17} /></span><div><small>{label}</small><strong>{value}</strong><em>{detail}</em></div>{!last && <span className="life-line" />}</div>;
}

function Resource({ icon: Icon, name, detail, state }: { icon: typeof Box; name: string; detail: string; state?: string }) {
  return <div className="resource"><span className={`resource-icon ${state ?? ""}`}><Icon size={16} /></span><div><strong>{name}</strong><small>{detail}</small></div><ChevronDown size={14} /></div>;
}

function Pulls({ project }: { project: ProjectDetail }) {
  return <section className="panel"><div className="panel-heading"><span><GitPullRequest size={16} />Pull requests</span><button className="button compact">New pull request</button></div><div className="list">{project.pullRequests.map((pull) => <div className="pull-row" key={pull.number}><span className="pull-icon"><GitPullRequest size={17} /></span><div><strong>{pull.title}</strong><small>#{pull.number} by {pull.author.displayName} · {pull.sourceBranch} → {pull.targetBranch}</small></div><a href={pull.preview?.url} target="_blank" rel="noreferrer">Preview<ArrowUpRight size={13} /></a></div>)}</div></section>;
}

function Runs({ project, onRun, busy }: { project: ProjectDetail; onRun: (job: string, kind: Run["kind"]) => void; busy?: string }) {
  return <><section className="run-actions"><div><Sparkles size={18} /><span><strong>friday-notes</strong><small>Codex agent · proposes changes through a pull request</small></span></div><button className="button primary" onClick={() => onRun("friday-notes", "agent")} disabled={busy === "friday-notes"}><Play size={14} />{busy === "friday-notes" ? "Queuing…" : "Run now"}</button></section><section className="panel"><div className="panel-heading"><span><History size={16} />Run history</span></div><div className="list">{project.runs.map((run) => <div className="run-row" key={run.id}><span className={`run-dot ${run.status}`} /><div><strong>{run.job}</strong><small>{run.trigger}</small></div><span className="kind-chip">{run.kind}</span><code>{shortSha(run.revisionSha)}</code><time>{relativeTime(run.startedAt)}</time></div>)}</div></section></>;
}

function Files() {
  const files = [["src", "folder"], ["agents", "folder"], ["c6.toml", "file"], ["README.md", "file"], ["Cargo.toml", "file"]];
  return <section className="panel"><div className="panel-heading"><span><GitBranch size={16} />main</span><button className="button compact">Add file</button></div><div className="file-list">{files.map(([name, kind]) => <div key={name}><span>{kind === "folder" ? "↳" : "·"}</span><strong>{name}</strong><small>{kind === "folder" ? "Browse folder" : "Updated in 7c1a840"}</small></div>)}</div></section>;
}

function Settings() {
  return <section className="panel settings-panel"><div className="panel-heading"><span><Server size={16} />Project settings</span></div><div className="setting"><div><strong>Built-in access gate</strong><p>Only invited C6 accounts can open the app.</p></div><button className="toggle" aria-label="Toggle access gate"><span /></button></div><div className="setting"><div><strong>Default branch</strong><p>Pull requests merge into this branch.</p></div><code>main</code></div><div className="setting"><div><strong>Agent repository writes</strong><p>Agents may only propose changes through branches and pull requests.</p></div><span className="policy-chip">Proposal only</span></div></section>;
}
