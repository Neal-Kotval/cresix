# Scheduled report

This manifest describes a weekly report command at 09:00 New York time. It
records the timezone explicitly so daylight-saving behavior is reviewable, and
uses `concurrency = "forbid"` so a delayed occurrence should not overlap the
next one.

`./scripts/render-report` is intentionally a placeholder. Parsing this manifest
does not schedule or run it. A production implementation should preserve the
declared timeout and resource limits, record each occurrence, and expose clear
failure and retry behavior.
