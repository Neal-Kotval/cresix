# Connected Cloud connector example

This directory shows the file boundary for one local connector. It contains no
working credential, account, installation, or local server identifier.

1. Copy `connector.example.toml` outside the repository into an owner-only
   directory.
2. Replace the UUID placeholders with values shown by the authenticated Cresix
   Cloud installation/binding flow.
3. Put the one-time Cloud connector credential and a local C6 API credential in
   two separate files. Do not add either value to the TOML file.
4. Restrict all three files to the owner: `chmod 600`.
5. For local dogfood only, keep both services on loopback and set
   `allow_insecure_cloud_loopback = true`.
6. Start the connector with `c6-connector --config /private/path/connector.toml`.

For a non-loopback Cloud endpoint, `cloud_origin` must be HTTPS. `local_origin`
is deliberately restricted to a literal `http://127.0.0.1:<port>` origin. The
connector is not a general proxy and will reject a hostname, path, embedded
credential, query, fragment, or missing port.

This example is configuration shape, not a production deployment recipe. The
Cloud dogfood identity, relay, and operations still lack the controls listed in
the [connected-mode specification](../../docs/specs/CRESIX_CLOUD_CONNECTED_MODE.md).
