# Extension points

## Extension philosophy

Cresix is modular through narrow typed seams, not arbitrary in-process plugins.
An extension must preserve one local authority, deny-by-default policy, explicit
versioning, and a recoverable storage boundary. Being open source does not make
loading unknown code into the control plane safe.

## Supported seams and intended adapters

| Seam | Contract | Examples | Guardrail |
| --- | --- | --- | --- |
| Ingress | HTTPS forwarding to one configured origin | Caddy, ngrok, Tailscale, Cloudflare Tunnel | Reachability only; never identity |
| Identity | External subject resolved to a local principal | Future OIDC or passkey | Immutable subject linking, local role checks, recovery ADR |
| Machine clients | Versioned JSON API and scoped credentials | CLI, future MCP, automation | No direct SQLite/Git access |
| Git transport | Local authorization then repository operation | Current read-only smart HTTP; future push | Expected refs, scoped credentials, no credential redirects |
| Runtime | `RuntimeAdapter` over resolved run plans | Future Docker, later microVM | No control-plane Docker socket; enforce every grant |
| Secrets | Native encrypted store or reference adapter | Future 1Password, Doppler | Opaque references, narrow provider auth, no plaintext reads |
| Events | Durable bounded event sequence | Long poll, later webhook/stream | Live visibility filtering and cursor retention semantics |
| Backup | Declared SQLite/Git and private-config boundary | Filesystem snapshot, future object target | Coherent quiesce/restore verification |
| C6R registry | Immutable release/artifact/provenance protocol | Cresix Cloud or self-hosted registry | Registry never becomes project policy authority |

## Module acceptance questions

Before adding an extension, answer:

1. Which existing authority makes the final decision?
2. What credential crosses the seam, and what is its least privilege?
3. Is the interface typed, bounded, versioned, and fail-closed?
4. Which state is durable, derived, or disposable, and who backs it up?
5. What happens on timeout, duplicate delivery, revocation, and downgrade?
6. Can the extension expose secrets, source, host paths, or runtime authority?
7. Can it be removed without stranding the installation or changing source
   truth?
8. Which negative and abuse-case tests prove the boundary?

## Explicitly rejected generic extension points

- dynamically loaded authentication or authorization code inside `c6-server`;
- a plugin with direct SQLite, Git-root, Docker-socket, or secret-store access;
- manifest-defined install hooks or lifecycle scripts during C6R resolution;
- an MCP adapter that mints broader tokens or skips human approval;
- runtime plugins that translate absent enforcement into a reduced grant;
- Cloud integrations that silently synchronize local roles or rewrite locks.

## Compatibility and lifecycle

Adapters declare their protocol and capability versions. Unsupported versions
fail closed. Removal must enumerate durable references and offer a deliberate
orphan, migrate, or delete decision. Startup must not silently rewrite user
manifests or locks; migration tooling produces a reviewable diff.

New seams require demonstrated use, threat analysis, and usually an ADR. This
keeps the core small while allowing companies to implement modules without
forking the fundamental authority model.
