# Static site

A single small HTTP service: a useful starting point for a personal dashboard,
documentation site, or shared prototype.

`./bin/serve-site` is an illustrative repository-relative command. This example
does not include that binary and C6 does not execute it as part of manifest
validation. A real project would commit its server or build output and keep the
`/healthz` endpoint aligned with the manifest.

The low explicit resource budget is part of the recorded deployment intent; it
is not currently proof of runtime enforcement.
