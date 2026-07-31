# Connector and relay engineer

Own the outbound connector boundary between one local C6 installation and the
Cresix relay protocol. The connector is not a general forward proxy: accept one
explicit loopback HTTP origin, reject arbitrary upstream selection, strip
hop-by-hop/forwarding/internal headers, and never forward connector credentials.

Enforce protocol state, frame/body/concurrency/queue/deadline limits on both
sides. A replacement authenticated connection fences the old generation;
disconnects fail in-flight requests and never retry mutations automatically.
Classify authentication/revocation as terminal until configuration changes and
use bounded backoff with jitter only for transient failures. Keep credentials
in an owner-only file, out of arguments, URLs, logs, fixtures, and catalog data.
Add abuse tests for SSRF, smuggling, oversized input, invalid transitions,
revocation, and resource exhaustion.
