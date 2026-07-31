# Scheduler

`c6-scheduler` is a pure deterministic engine. The server uses it to validate
new schedule records and compute `nextOccurrenceAt`; no background daemon ticks
schedules or dispatches runs.

## Validated contract

- Standard five-field cron: minute, hour, day of month, month, weekday
- Explicit IANA timezone such as `UTC` or `America/New_York`
- `forbid` concurrency only in the server API
- Deterministic occurrence key: `(schedule_id, occurrence_at UTC)`
- Injected clock for repeatable tests

Expressions with seconds/year fields, invalid timezones, empty IDs, and
unsupported concurrency are rejected when the schedule is created.

## Time behavior

- `next_after` is strictly after the supplied instant.
- A nonexistent local time in a spring-forward gap is skipped.
- In a fall-back fold, C6 chooses the earlier matching instant and treats the
  local occurrence once.
- Clock movement behind the durable checkpoint is an error.

## Restart semantics in the engine

The engine accepts a durable checkpoint, claimed occurrence keys, active run,
and `Live` or `Recovery` tick mode. Recovery policies are bounded:

- `skip`: advance without enqueueing the missed occurrence.
- `run_once`: enqueue the first missed occurrence once and drop remaining
  backlog.

Every decision returns the checkpoint that must be committed atomically with
the claimed occurrence. Duplicate claims and active `forbid` runs are explicit
decisions, not silent retries.

## Product boundary

The server currently persists validated schedule metadata and the next instant,
returning `dispatchAvailable: false`. It does not persist scheduler checkpoints,
run a tick loop, or create occurrence-backed runs. Those are required before
the UI can truthfully say a cron will execute.
