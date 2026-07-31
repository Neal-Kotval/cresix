export type Role =
  | "consumer"
  | "reader"
  | "runner"
  | "contributor"
  | "maintainer"
  | "owner";

export interface User {
  id: string;
  handle: string;
  displayName: string;
}

export interface Deployment {
  id: string;
  revisionSha: string;
  environment: "preview" | "production";
  status: "queued" | "building" | "ready" | "failed" | "superseded";
  url?: string;
  createdAt: string;
}

export interface Revision {
  sha: string;
  message: string;
  author: User;
  createdAt: string;
}

export interface PullRequest {
  number: number;
  title: string;
  sourceBranch: string;
  targetBranch: string;
  author: User;
  status: "open" | "merged" | "closed";
  preview?: Deployment;
  updatedAt: string;
}

export interface Run {
  id: string;
  job: string;
  kind: "command" | "cron" | "agent";
  revisionSha: string;
  status: "queued" | "running" | "succeeded" | "failed" | "interrupted" | "cancelled";
  trigger: string;
  startedAt: string;
  finishedAt?: string;
}

export interface ProjectDetail {
  id: string;
  workspaceId: string;
  slug: string;
  name: string;
  description: string;
  defaultBranch: string;
  headSha: string;
  publishedSha?: string;
  role: Role;
  appUrl?: string;
  updatedAt: string;
  readme: string;
  revisions: Revision[];
  pullRequests: PullRequest[];
  deployments: Deployment[];
  runs: Run[];
}

