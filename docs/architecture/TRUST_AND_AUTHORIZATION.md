# Trust and authorization

## Trust domains

Cresix has five distinct trust domains. Credentials from one domain are not
accepted merely because they reached another.

| Domain | Principal and proof | Authoritative decision |
| --- | --- | --- |
| Local browser | C6 peer, `c6_session` cookie plus bound CSRF token | Live local session/device/peer state and workspace role |
| Local machine client | C6 peer, scoped CLI Bearer or Git Basic secret | Token type/scope/restriction plus live local role |
| Cresix Cloud browser | Cloud account, Cloud session plus bound CSRF token | Cloud membership and resource ownership |
| Connector | Installation registration, connector secret | Active in-memory session identity, revocation, route binding, protocol state |
| Relayed local C6 client | Local C6 cookie, Bearer, or Git credential transported through target relay origin | The same live local C6 checks used by standalone ingress |

A Cloud session never authenticates the local browser, and a connector secret
never authenticates a person. Mixed browser cookies and Authorization headers
fail instead of choosing one opportunistically.

The connector's configured local API credential reads project metadata for
catalog publication only. It does not authenticate relayed users. Relayed
client credentials remain in the local C6 trust domain; the same-origin
dogfood path strips cookies, so browser authentication through the relay is not
available until isolated installation origins exist.

## Local enrollment and authorization

**Implemented:** first claim consumes a one-time, 256-bit bootstrap proof and
creates the immutable server administrator. Invitations consume one-time
proofs and can create workspace membership. Plaintext proofs are returned or
written once; only SHA-256 verifiers remain in SQLite.

Unsafe browser mutations require all of:

1. an unexpired, unrevoked peer/device/session chain;
2. exact `Origin` equality with the configured public URL;
3. a readable CSRF cookie matching both the request header and session-bound
   verifier; and
4. current role or explicit server-administrator capability.

CLI and Git tokens are high-entropy, expiring, scoped, optionally restricted,
and independently revocable. Bearer requests avoid CSRF but remain subject to
live role checks. Git accepts only username `c6` and a `git:read` token in the
implemented phase.

## Known local identity limitations

The submitted device public key is unverified metadata. There is no passkey,
password, key challenge, re-login proof, account recovery, or administrator
transfer. Losing the only administrator session can permanently lock global
administration. These limitations make current enrollment appropriate for
evaluation, not a durable company identity system.

## Cloud and relay trust

**Dogfood:** Cloud bootstrap is loopback-only. Browser sessions are host-only
and mutations require exact Origin and bound CSRF. Installation creation
reveals the connector credential once; Cloud stores its verifier. A new
authenticated relay session receives an opaque, internal in-memory identity
that replaces and fences the previous live session. The wire
`ServerReady.generation` is currently the constant placeholder `1`, not the
fencing mechanism, credential rotation, or catalog revision. Revocation
terminates relay access.

Cloud's TLS edge and relay can observe proxied headers, cookies, Git source,
request bodies, and response bodies. The target is managed trusted ingress,
not relay-blind end-to-end encryption. A compromised connector can observe or
modify the same local HTTP traffic and possesses its separately configured
local catalog credential.

The dogfood same-origin relay strips `Cookie` and `Set-Cookie`; this avoids
collapsing Cloud and local browser session domains but means it is not the
target browser experience.

## Target production identity

Production Cloud requires a reviewed passkey or OIDC adapter, immutable
provider-subject linking, recovery, recent-authentication gates, enrollment and
login rate limits, notifications, abuse response, and incident operations.
Local C6 still needs an explicit SSO design before Cloud identity can produce a
local session. The design must define provisioning, offboarding, role mapping,
recovery, audit provenance, and behavior during Cloud outage; none is implied
today.

## Authorization invariant

All protected operations use server-derived actor identity and live
authorization. The effective authority for a future workload is:

```text
declared requirement
  intersect consumer grant
  intersect caller's live local role
  intersect operator policy
  intersect enforcement available on this host
```

If the entire declared requirement is not grantable and enforceable, C6 fails
closed. Requirements, capabilities documents, catalog records, manifest text,
IP addresses, and URLs are never bearer grants.

See the [native trust model](../TRUST_MODEL.md),
[authorization matrix](../AUTHORIZATION.md), and
[threat model](../THREAT_MODEL.md) for implementation-level controls.
