# ADR 0002: Optional Cresix Cloud directory and relay

Status: accepted

## Context

Standalone C6 is sovereign but still requires an operator to arrange DNS, TLS,
and ingress. People want stable Cresix accounts and shareable project URLs even
when a C6 server runs on a laptop behind NAT.

A shared path proxy at `cresix.com/{workspace}` appears simple, but unrelated
C6 installations currently share cookie names and each validates one exact
public origin. That design would collapse authentication boundaries. Moving all
local data and authorization into a central service would contradict C6's
self-hosted product boundary and create a much larger distributed system.

## Decision

Add an optional Cresix Cloud control plane that owns global accounts,
workspace namespaces, a bounded project directory, installation registration,
and managed reachability. Keep Git, runtime state, local roles, local sessions,
and secrets on the C6 installation.

Use `cresix.com/{workspace}/{project}` as a stable directory URL. Route actual
C6 requests through a separate opaque per-installation origin. A local
connector establishes one outbound authenticated tunnel; no inbound port or
same-network connection is required.

Standalone mode remains fully supported. Cloud accounts and local C6
principals are distinct until a separately reviewed SSO design exists.

## Consequences

- A laptop can be reachable without ngrok, a public IP, or router changes while
  it is awake and connected.
- Several C6 authorities do not share one browser cookie origin.
- Cloud outages affect directory and managed ingress, not local data or
  standalone access.
- The directory is eventually consistent and cannot be used as repository or
  authorization truth.
- Cresix's TLS edge can observe relayed traffic. The relay is a trusted ingress
  provider, not an end-to-end encrypted transport.
- Production still requires reviewed account authentication/recovery, rate
  limiting, abuse controls, relay isolation, and operational hardening.

## Alternatives rejected

- **Only operator-provided tunnels:** keeps the core smaller but does not
  deliver stable Cresix identities, discovery, or zero-ingress sharing.
- **Proxy all installations under one `cresix.com` path origin:** breaks the
  isolation assumptions of current browser sessions and CSRF enforcement.
- **Centralize repositories and local authorization immediately:** erases
  standalone sovereignty and adds migration and consistency problems before
  demonstrated need.
- **One hostname per workspace:** conflicts with the current installation-wide
  session/public-origin model when one installation hosts several workspaces.
