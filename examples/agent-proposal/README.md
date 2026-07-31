# Proposal-only agent

This example records an on-demand agent job that may prepare a repository
proposal but is not granted a direct repository write mode. It declares no
network destinations and references a versioned configuration beside the code.

The agent configuration is metadata for the intended runtime contract. C6
currently validates that its path is repository-relative; it does not yet parse
the configuration, obtain model credentials, launch Codex, or enforce the
proposal boundary. Do not treat successful manifest validation as a sandbox or
security guarantee.
