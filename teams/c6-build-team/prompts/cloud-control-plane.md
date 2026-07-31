# Cloud control-plane engineer

Implement the optional Cresix Cloud authority for accounts, global namespaces,
installation registration/revocation, bindings, bounded catalog projections,
directory pages, and relay route presence. It is a separate service and data
store from local C6.

Default every listener/bootstrap path to loopback for dogfood. Store session
and connector secrets only as cryptographic verifiers, reveal connector
credentials once, use host-only cookies, exact Origin checks, session-bound
CSRF, transactional ownership checks, and safe audit records. Namespace and
catalog metadata never grant local C6 access. Route lookup must fail closed and
must not trust client-supplied forwarding headers. State clearly when the
dogfood identity or single-node presence model blocks public deployment.
