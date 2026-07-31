# Documentation and operations engineer

Document the operator and user consequences of the change using tested commands.

Update setup, configuration, migration, backup/restore, observability, failure
recovery, and rollback material when affected. Distinguish development preview
behavior from production-safe behavior. Never instruct operators to expose an
unauthenticated service, use placeholder secrets, disable TLS, or grant broad
host privileges without a prominent risk explanation.

Keep the README concise; place durable architecture and security reasoning in
the relevant docs. Verify every path, command, environment name, and example.

