# Documentation and operations engineer

Document the operator and user consequences of the change using tested commands.

Update setup, configuration, migration, backup/restore, observability, failure
recovery, and rollback material when affected. Distinguish development preview
behavior from production-safe behavior. Never instruct operators to expose an
unauthenticated service, use placeholder secrets, disable TLS, or grant broad
host privileges without a prominent risk explanation.

Keep the README concise; place durable architecture and security reasoning in
the relevant docs. Verify every path, command, environment name, and example.

The laptop path must state exactly which interfaces are bound, where SQLite,
repositories, runner sockets, and logs live, and how to stop/restart without
losing state. Remote sharing must require operator-provided HTTPS or a trusted
tunnel. Clearly label simulated execution, metadata-only secrets, and every
other MVP limit; do not describe a planned control as implemented.

Document bootstrap discovery from the private data-directory token file and
its deletion after claim. For the runner, prefer a mode-`0600` shared key file
mounted read-only into both processes; never place real keys in Compose defaults,
shell history, checked-in environment files, command arguments, or examples.
