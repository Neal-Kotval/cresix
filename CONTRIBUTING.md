# Contributing to C6

C6 is early. Prefer one complete, honest small-software workflow over another
general cloud primitive.

## Before coding

Read the [product principles](docs/PRODUCT.md),
[architecture](docs/ARCHITECTURE.md), and relevant trust-boundary page. Confirm
whether the behavior is implemented, recorded-only, simulated, or deferred.
Do not make the UI imply a deeper backend than exists.

Keep boundaries cohesive:

- public domain contracts and manifest policy in `c6-core`;
- safe local Git operations in `c6-git`;
- pure schedule decisions in `c6-scheduler`;
- protocol and simulation behavior in `c6-runner`;
- authentication, persistence, authorization, and HTTP orchestration in
  `c6-server`;
- presentation and browser flows in `web`.

Introduce an abstraction only when a second real implementation needs it.

## Change workflow

1. Create a branch and keep the patch focused.
2. Write or update the contract before crossing a process/trust boundary.
3. Add regression and abuse-case tests with the implementation.
4. Update the manual when behavior, configuration, limitations, or operations
   change.
5. Run the gates in [Testing](docs/TESTING.md).
6. Open a pull request explaining the user-visible outcome, architecture fit,
   security impact, verification evidence, and remaining limitations.

## Security and data

Do not commit `.env`, cookie jars, bootstrap/invitation/session tokens, runner
keys/state, project data, provider credentials, `auth.json`, or private keys.
Do not mount Docker into the control plane, inherit developer Git configuration
inside managed operations, or use IP addresses as identity.

Authentication, authorization, filesystem, process, network, schedule, and
secret-related changes need fail-closed behavior and negative tests. Report
vulnerabilities as described in [SECURITY.md](SECURITY.md).

## Product truth

Use these words precisely:

- **recorded:** durable intent exists, but nothing was dispatched;
- **simulated:** the runner exercised lifecycle without executing a workload;
- **deferred:** no supported implementation exists;
- **live/ready:** reserve for a real, verified running capability.

Documentation and UI fixtures must use the same distinctions.
