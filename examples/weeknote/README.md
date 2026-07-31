# Weeknote

Weeknote is the composed example: an HTTP service, an hourly activity sync, and
a Friday agent that proposes team notes. It demonstrates storage declarations,
scheduled work, a named secret dependency, network intent, resource budgets,
and proposal-only repository writes in one manifest.

There is no `weeknote` executable in this directory and no credential value is
included. The agent configuration and prompt are versioned examples. C6 can
validate and record this contract; validation does not launch the service,
schedule jobs, connect to OpenAI, enforce network policy, or guarantee an
isolated proposal branch.
