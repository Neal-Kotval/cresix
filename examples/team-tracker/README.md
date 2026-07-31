# Team tracker

A small shared workflow tool with a web service and weekday summary job. The
manifest declares that the application expects PostgreSQL-compatible storage,
file storage, and one secret value supplied outside Git.

Only the secret name and purpose appear here. Never commit the signing key. The
commands are placeholders, and the current manifest parser does not provision a
database, mount file storage, inject a secret, or start either runtime.
