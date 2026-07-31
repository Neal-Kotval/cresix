# Contributing to C6

C6 is early. Prefer changes that make a complete small-software workflow
simpler over adding general cloud primitives.

1. Create a branch.
2. Keep public contracts in `c6-core` and privileged execution in `c6-runner`.
3. Add negative authorization or isolation tests for trust-boundary changes.
4. Run the verification commands from the README.
5. Open a pull request describing the user-visible outcome and security impact.

Do not commit credentials, `.env` files, provider tokens, runner state, project
data, or generated secrets.

