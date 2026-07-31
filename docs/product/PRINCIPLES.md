# Product principles

These principles constrain the product. They are more durable than individual
features and should be cited when roadmap choices conflict.

## 1. The installation is sovereign

A C6 installation owns local identity, authorization, Git, collaboration
metadata, and operational records. Standalone use must not require a Cresix
account. Optional Cloud services may improve discovery and reachability without
silently becoming the local authority.

## 2. Centralize only what must be global

Account subjects, globally unique account handles, account-scoped workspace
slugs, directory entries, and managed route presence belong in the target
Cresix Cloud boundary. The current preview instead uses one globally unique
workspace namespace inside one database and has no public account handle. Local
repository contents, roles, sessions, schedules, runs, and secret values do not belong in
Cloud. Centralization must solve a demonstrated coordination problem, not merely
make the diagram familiar.

## 3. Remote-first is not peer-authority

Collaborators may be anywhere; same Wi-Fi, IP address, and physical proximity
are never identity. Each project still has one write authority. Git distributes
source work at the edge; C6 does not attempt multi-writer control-plane
federation without evidence that its conflict and revocation costs are needed.

## 4. Git owns source

Commits, trees, blobs, and refs belong in Git. SQLite stores control-plane
metadata, not duplicate source truth. Standard Git clients remain useful, and
future C6R provenance resolves to immutable Git revisions and content digests.

## 5. Intent is not execution

A manifest, schedule, run row, deployment record, button, or protocol type is
not proof that a workload ran. Unsupported execution states must be explicit in
the UI and API. New runtime claims require an implemented adapter and end-to-end
verification.

## 6. Agents use narrow, shared interfaces

Humans and agents should act against the same authorization decisions. The web
provides oversight; the CLI provides composition and automation; stable HTTP
and a thin MCP adapter can later provide polling and tool access. None may
bypass the authoritative API or query the database as a privileged shortcut.

## 7. Review before activation

Reusable material is pinned and inspectable before it becomes active. Passive
content can arrive earlier than commands, services, MCP servers, or applications.
Active capability must pass runtime, provenance, network, secret, permission,
and audit gates.

## 8. Secure defaults beat convenient ambiguity

Trust boundaries deny by default. Tokens are distinct by purpose, roles are
checked live, paths are bounded, origins are exact, and credentials are never
inferred from network location. Convenience that widens authority must be an
explicit, reviewable choice.

## 9. One small operational unit first

SQLite, bare Git repositories, local files, one server, and replaceable ingress
are the default. External databases, object stores, registries, Kubernetes,
federation, and generic plugin systems must earn their complexity through real
use.

## 10. Portability is an availability feature

The installation should have a declared data boundary, verified backup and
restore, and a comprehensible move from laptop to server. C6 must not imply
high availability when the real behavior is one authority that may be offline.

## 11. Product truth is a feature

Documentation and UI must distinguish implemented behavior, simulation,
recorded intent, and future design. “Now” requires code and regression evidence.
Known production blockers remain visible instead of being hidden by polished UI.

## 12. Extensions preserve the core boundary

Ingress providers, identity adapters, runners, secret backends, and C6R kinds
should attach through narrow typed seams. Extensions must not create a second
data authority, broaden credentials implicitly, or make standalone mode depend
on an optional service.
