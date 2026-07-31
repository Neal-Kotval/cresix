# Security policy

Please report suspected vulnerabilities privately to the repository owner. Do
not open a public issue containing exploit details, credentials, private source,
or data from a C6 installation.

## Current scope

The current release is a development preview and is not suitable for hostile
multi-tenant or public-internet deployment. Containers are an accident boundary
for trusted team code, not a promise that malicious workloads cannot escape.

Security-sensitive implementation must preserve these boundaries:

- The web/control-plane process never receives the Docker socket.
- A project cannot reach another project's filesystem, database, bucket, or
  network namespace.
- Secrets are individually granted and never copied into Git history or forks.
- Gateway identity headers are stripped and regenerated at the trusted edge.
- Agents never write directly to a protected/default branch.
- Unknown runner outcomes are not automatically retried.

