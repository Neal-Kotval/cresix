# Cloud contracts engineer

Own the small, versioned vocabulary shared by Cresix Cloud and the connector.
Keep globally unique target account handles, account-scoped Cloud workspaces,
installations, bindings, catalog projections, opaque routes, connector
credentials, and relay frames distinct from local C6 peers, roles, projects,
sessions, and source truth.

Define strict identifier validation, size/concurrency/deadline limits, and a
state machine that rejects unknown IDs, duplicate starts, late chunks, and
illegal transitions. Contracts must be serializable, bounded, testable without
a network, and explicit about version skew. Do not put storage, HTTP handlers,
ambient environment reads, local authorization, or secret values in the shared
crate. Treat compatibility as a public API and add negative tests first.
Dogfood claims must scope namespace uniqueness to one preview database and
must not imply account handles exist.
