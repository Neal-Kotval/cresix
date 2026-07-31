//! Deterministic scheduling primitives for the C6 control plane.
//!
//! This crate intentionally does not own persistence, a daemon, or job execution. The control
//! plane supplies durable scheduling state to [`Scheduler::tick`] and atomically persists the
//! returned checkpoint and any enqueued occurrence. That boundary keeps restart behavior
//! explicit and makes the scheduling policy testable without wall-clock sleeps.

use std::str::FromStr;

use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use cron::Schedule;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// User-provided scheduling configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleDefinition {
    /// Stable schedule identifier. It is part of every occurrence's idempotency key.
    pub id: String,
    /// A standard five-field cron expression: minute, hour, day-of-month, month, weekday.
    pub cron: String,
    /// An IANA timezone name such as `UTC` or `America/New_York`.
    pub timezone: String,
    /// What to do with an occurrence found during the first tick after a restart.
    pub missed_run_policy: MissedRunPolicy,
}

/// Bounded restart behavior. Neither policy can enqueue more than one occurrence per tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissedRunPolicy {
    /// Advance the checkpoint without enqueueing an occurrence missed while the scheduler was down.
    Skip,
    /// Enqueue the first missed occurrence once, then advance past the rest of the backlog.
    #[default]
    RunOnce,
}

/// A validated schedule ready for repeated evaluation.
#[derive(Clone, Debug)]
pub struct ValidatedSchedule {
    definition: ScheduleDefinition,
    timezone: Tz,
    parsed: Schedule,
}

impl ValidatedSchedule {
    /// Validate and compile a schedule definition.
    pub fn new(definition: ScheduleDefinition) -> Result<Self, ScheduleError> {
        if definition.id.trim().is_empty() {
            return Err(ScheduleError::EmptyId);
        }

        let fields: Vec<_> = definition.cron.split_whitespace().collect();
        if fields.len() != 5 {
            return Err(ScheduleError::InvalidFieldCount {
                actual: fields.len(),
            });
        }

        let timezone = Tz::from_str(&definition.timezone)
            .map_err(|_| ScheduleError::InvalidTimezone(definition.timezone.clone()))?;

        // `cron` accepts seconds and an optional year. Pin seconds to zero and all years so the
        // public contract remains the familiar, portable five-field format.
        let expanded = format!("0 {} *", fields.join(" "));
        let parsed = Schedule::from_str(&expanded)
            .map_err(|error| ScheduleError::InvalidCron(error.to_string()))?;

        Ok(Self {
            definition,
            timezone,
            parsed,
        })
    }

    pub fn definition(&self) -> &ScheduleDefinition {
        &self.definition
    }

    /// Return the first occurrence strictly after `instant`.
    ///
    /// During a spring-forward gap, nonexistent local times are skipped. During a fall-back fold,
    /// C6 selects the earlier of the two matching instants and runs that local occurrence once.
    pub fn next_after(&self, instant: DateTime<Utc>) -> Result<DateTime<Utc>, ScheduleError> {
        let local = instant.with_timezone(&self.timezone);
        self.parsed
            .after(&local)
            .next()
            .map(|next| next.with_timezone(&Utc))
            .ok_or(ScheduleError::NoFutureOccurrence)
    }
}

/// Durable idempotency key for an individual scheduled run.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OccurrenceKey {
    pub schedule_id: String,
    pub occurrence_at: DateTime<Utc>,
}

impl OccurrenceKey {
    pub fn new(schedule_id: impl Into<String>, occurrence_at: DateTime<Utc>) -> Self {
        Self {
            schedule_id: schedule_id.into(),
            occurrence_at,
        }
    }
}

/// Indicates whether this is an ordinary tick or the first tick after restoring durable state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TickMode {
    #[default]
    Live,
    Recovery,
}

/// Durable inputs read by the control plane before evaluating a tick.
#[derive(Clone, Copy, Debug)]
pub struct TickState<'a> {
    /// Exclusive lower bound already evaluated by a previous transaction.
    pub evaluated_through: DateTime<Utc>,
    pub mode: TickMode,
    /// Any currently non-terminal occurrence for this schedule.
    pub active_occurrence: Option<&'a OccurrenceKey>,
    /// Occurrences already claimed by a durable transaction.
    pub claimed_occurrences: &'a [OccurrenceKey],
}

/// A single bounded scheduling decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TickDecision {
    NoOccurrenceDue,
    Enqueue(OccurrenceKey),
    MissedOccurrenceSkipped(OccurrenceKey),
    AlreadyClaimed(OccurrenceKey),
    BlockedByActiveRun {
        due: OccurrenceKey,
        active: OccurrenceKey,
    },
}

/// The decision and the checkpoint that must be committed with it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TickOutcome {
    /// Persist this value even when a run is skipped or blocked. This deliberately drops any
    /// remaining backlog and prevents unbounded catch-up after downtime.
    pub evaluated_through: DateTime<Utc>,
    pub decision: TickDecision,
}

/// Injected time source. Production code can use [`SystemClock`]; tests can remain deterministic.
pub trait Clock {
    fn now(&self) -> DateTime<Utc>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// Pure scheduler engine with an injected clock.
#[derive(Clone, Debug)]
pub struct Scheduler<C> {
    clock: C,
}

impl<C: Clock> Scheduler<C> {
    pub fn new(clock: C) -> Self {
        Self { clock }
    }

    pub fn tick(
        &self,
        schedule: &ValidatedSchedule,
        state: TickState<'_>,
    ) -> Result<TickOutcome, ScheduleError> {
        evaluate(schedule, state, self.clock.now())
    }
}

/// Evaluate one tick at an explicit time. This function performs no I/O.
pub fn evaluate(
    schedule: &ValidatedSchedule,
    state: TickState<'_>,
    now: DateTime<Utc>,
) -> Result<TickOutcome, ScheduleError> {
    if now < state.evaluated_through {
        return Err(ScheduleError::ClockMovedBackwards {
            checkpoint: state.evaluated_through,
            now,
        });
    }

    let Some(occurrence_at) = schedule
        .next_after(state.evaluated_through)
        .map(Some)
        .or_else(|error| match error {
            ScheduleError::NoFutureOccurrence => Ok(None),
            other => Err(other),
        })?
    else {
        return Ok(TickOutcome {
            evaluated_through: now,
            decision: TickDecision::NoOccurrenceDue,
        });
    };

    if occurrence_at > now {
        return Ok(TickOutcome {
            evaluated_through: now,
            decision: TickDecision::NoOccurrenceDue,
        });
    }

    let due = OccurrenceKey::new(schedule.definition.id.clone(), occurrence_at);
    let decision = if state.mode == TickMode::Recovery
        && schedule.definition.missed_run_policy == MissedRunPolicy::Skip
    {
        TickDecision::MissedOccurrenceSkipped(due)
    } else if state.claimed_occurrences.contains(&due) {
        TickDecision::AlreadyClaimed(due)
    } else if let Some(active) = state.active_occurrence {
        // The MVP supports only `forbid`: overlap is denied and the due occurrence is not retried.
        TickDecision::BlockedByActiveRun {
            due,
            active: active.clone(),
        }
    } else {
        TickDecision::Enqueue(due)
    };

    Ok(TickOutcome {
        evaluated_through: now,
        decision,
    })
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ScheduleError {
    #[error("schedule id must not be empty")]
    EmptyId,
    #[error("cron expression must contain exactly five fields, got {actual}")]
    InvalidFieldCount { actual: usize },
    #[error("invalid cron expression: {0}")]
    InvalidCron(String),
    #[error("unknown IANA timezone: {0}")]
    InvalidTimezone(String),
    #[error("schedule has no future occurrence")]
    NoFutureOccurrence,
    #[error("clock moved backwards from checkpoint {checkpoint} to {now}")]
    ClockMovedBackwards {
        checkpoint: DateTime<Utc>,
        now: DateTime<Utc>,
    },
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;

    fn at(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, hour, minute, 0)
            .unwrap()
    }

    fn schedule(cron: &str, timezone: &str, policy: MissedRunPolicy) -> ValidatedSchedule {
        ValidatedSchedule::new(ScheduleDefinition {
            id: "nightly".into(),
            cron: cron.into(),
            timezone: timezone.into(),
            missed_run_policy: policy,
        })
        .unwrap()
    }

    fn state<'a>(checkpoint: DateTime<Utc>) -> TickState<'a> {
        TickState {
            evaluated_through: checkpoint,
            mode: TickMode::Live,
            active_occurrence: None,
            claimed_occurrences: &[],
        }
    }

    #[test]
    fn rejects_nonstandard_field_counts_and_unknown_timezones() {
        let mut definition = ScheduleDefinition {
            id: "x".into(),
            cron: "0 9 * *".into(),
            timezone: "UTC".into(),
            missed_run_policy: MissedRunPolicy::RunOnce,
        };
        assert_eq!(
            ValidatedSchedule::new(definition.clone()).unwrap_err(),
            ScheduleError::InvalidFieldCount { actual: 4 }
        );

        definition.cron = "0 9 * * *".into();
        definition.timezone = "New_York-ish".into();
        assert_eq!(
            ValidatedSchedule::new(definition).unwrap_err(),
            ScheduleError::InvalidTimezone("New_York-ish".into())
        );
    }

    #[test]
    fn rejects_invalid_values_and_empty_ids() {
        let invalid = ScheduleDefinition {
            id: "x".into(),
            cron: "99 9 * * *".into(),
            timezone: "UTC".into(),
            missed_run_policy: MissedRunPolicy::RunOnce,
        };
        assert!(matches!(
            ValidatedSchedule::new(invalid),
            Err(ScheduleError::InvalidCron(_))
        ));

        let empty_id = ScheduleDefinition {
            id: "  ".into(),
            cron: "0 9 * * *".into(),
            timezone: "UTC".into(),
            missed_run_policy: MissedRunPolicy::RunOnce,
        };
        assert_eq!(
            ValidatedSchedule::new(empty_id).unwrap_err(),
            ScheduleError::EmptyId
        );
    }

    #[test]
    fn timezone_schedule_returns_utc_occurrence() {
        let schedule = schedule("0 9 * * *", "America/New_York", MissedRunPolicy::RunOnce);
        assert_eq!(
            schedule.next_after(at(2025, 1, 1, 0, 0)).unwrap(),
            at(2025, 1, 1, 14, 0)
        );
        assert_eq!(
            schedule.next_after(at(2025, 7, 1, 0, 0)).unwrap(),
            at(2025, 7, 1, 13, 0)
        );
    }

    #[test]
    fn spring_forward_nonexistent_local_time_is_skipped() {
        let schedule = schedule("30 2 * * *", "America/New_York", MissedRunPolicy::RunOnce);
        assert_eq!(
            schedule.next_after(at(2025, 3, 8, 7, 30)).unwrap(),
            at(2025, 3, 10, 6, 30)
        );
    }

    #[test]
    fn fall_back_ambiguous_local_time_runs_once_at_earlier_instant() {
        let schedule = schedule("30 1 * * *", "America/New_York", MissedRunPolicy::RunOnce);
        let first = schedule.next_after(at(2025, 11, 2, 5, 0)).unwrap();
        assert_eq!(first, at(2025, 11, 2, 5, 30));
        // A new evaluation after the selected occurrence advances to the next local day rather
        // than running the repeated wall-clock time a second time.
        assert_eq!(schedule.next_after(first).unwrap(), at(2025, 11, 3, 6, 30));
    }

    #[test]
    fn no_due_occurrence_advances_checkpoint() {
        let schedule = schedule("0 * * * *", "UTC", MissedRunPolicy::RunOnce);
        let now = at(2025, 1, 1, 10, 30);
        let outcome = evaluate(&schedule, state(at(2025, 1, 1, 10, 5)), now).unwrap();
        assert_eq!(outcome.evaluated_through, now);
        assert_eq!(outcome.decision, TickDecision::NoOccurrenceDue);
    }

    #[test]
    fn tick_enqueues_only_first_due_occurrence_and_drops_backlog() {
        let schedule = schedule("* * * * *", "UTC", MissedRunPolicy::RunOnce);
        let now = at(2025, 1, 1, 12, 0);
        let outcome = evaluate(&schedule, state(at(2025, 1, 1, 8, 0)), now).unwrap();
        assert_eq!(
            outcome.decision,
            TickDecision::Enqueue(OccurrenceKey::new("nightly", at(2025, 1, 1, 8, 1)))
        );
        assert_eq!(outcome.evaluated_through, now);

        let next = evaluate(&schedule, state(outcome.evaluated_through), now).unwrap();
        assert_eq!(next.decision, TickDecision::NoOccurrenceDue);
    }

    #[test]
    fn recovery_skip_never_enqueues_a_missed_run() {
        let schedule = schedule("0 * * * *", "UTC", MissedRunPolicy::Skip);
        let mut restored = state(at(2025, 1, 1, 8, 30));
        restored.mode = TickMode::Recovery;
        let outcome = evaluate(&schedule, restored, at(2025, 1, 1, 12, 30)).unwrap();
        assert_eq!(
            outcome.decision,
            TickDecision::MissedOccurrenceSkipped(OccurrenceKey::new(
                "nightly",
                at(2025, 1, 1, 9, 0)
            ))
        );
    }

    #[test]
    fn recovery_run_once_enqueues_exactly_one_then_advances_past_backlog() {
        let schedule = schedule("0 * * * *", "UTC", MissedRunPolicy::RunOnce);
        let mut restored = state(at(2025, 1, 1, 8, 30));
        restored.mode = TickMode::Recovery;
        let now = at(2025, 1, 1, 12, 30);
        let outcome = evaluate(&schedule, restored, now).unwrap();
        assert!(matches!(outcome.decision, TickDecision::Enqueue(_)));

        let subsequent = evaluate(&schedule, state(outcome.evaluated_through), now).unwrap();
        assert_eq!(subsequent.decision, TickDecision::NoOccurrenceDue);
    }

    #[test]
    fn durable_claim_suppresses_duplicate_after_restart() {
        let schedule = schedule("0 9 * * *", "UTC", MissedRunPolicy::RunOnce);
        let due = OccurrenceKey::new("nightly", at(2025, 1, 1, 9, 0));
        let claimed = [due.clone()];
        let restored = TickState {
            evaluated_through: at(2025, 1, 1, 8, 0),
            mode: TickMode::Recovery,
            active_occurrence: None,
            claimed_occurrences: &claimed,
        };
        let outcome = evaluate(&schedule, restored, at(2025, 1, 1, 10, 0)).unwrap();
        assert_eq!(outcome.decision, TickDecision::AlreadyClaimed(due));
    }

    #[test]
    fn repeated_uncommitted_tick_produces_the_same_occurrence_key() {
        let schedule = schedule("0 9 * * *", "UTC", MissedRunPolicy::RunOnce);
        let checkpoint = at(2025, 1, 1, 8, 0);
        let now = at(2025, 1, 1, 10, 0);
        let first = evaluate(&schedule, state(checkpoint), now).unwrap();
        let retried = evaluate(&schedule, state(checkpoint), now).unwrap();
        assert_eq!(first, retried);
    }

    #[test]
    fn occurrence_key_serialization_preserves_both_identity_components() {
        let key = OccurrenceKey::new("nightly", at(2025, 1, 1, 9, 0));
        let json = serde_json::to_string(&key).unwrap();
        assert!(json.contains("nightly"));
        assert!(json.contains("2025-01-01T09:00:00Z"));
        assert_eq!(serde_json::from_str::<OccurrenceKey>(&json).unwrap(), key);
    }

    #[test]
    fn forbid_concurrency_blocks_and_does_not_retry_due_occurrence() {
        let schedule = schedule("0 * * * *", "UTC", MissedRunPolicy::RunOnce);
        let active = OccurrenceKey::new("nightly", at(2025, 1, 1, 8, 0));
        let tick = TickState {
            evaluated_through: at(2025, 1, 1, 8, 30),
            mode: TickMode::Live,
            active_occurrence: Some(&active),
            claimed_occurrences: &[],
        };
        let now = at(2025, 1, 1, 9, 30);
        let outcome = evaluate(&schedule, tick, now).unwrap();
        assert_eq!(
            outcome.decision,
            TickDecision::BlockedByActiveRun {
                due: OccurrenceKey::new("nightly", at(2025, 1, 1, 9, 0)),
                active,
            }
        );
        assert_eq!(outcome.evaluated_through, now);
    }

    #[test]
    fn backwards_clock_is_rejected_without_a_decision() {
        let schedule = schedule("0 * * * *", "UTC", MissedRunPolicy::RunOnce);
        let error = evaluate(
            &schedule,
            state(at(2025, 1, 1, 10, 0)),
            at(2025, 1, 1, 9, 59),
        )
        .unwrap_err();
        assert!(matches!(error, ScheduleError::ClockMovedBackwards { .. }));
    }

    #[derive(Clone, Copy)]
    struct FixedClock(DateTime<Utc>);

    impl Clock for FixedClock {
        fn now(&self) -> DateTime<Utc> {
            self.0
        }
    }

    #[test]
    fn injected_clock_makes_engine_deterministic() {
        let now = at(2025, 1, 1, 10, 5);
        let engine = Scheduler::new(FixedClock(now));
        let schedule = schedule("0 10 * * *", "UTC", MissedRunPolicy::RunOnce);
        let outcome = engine.tick(&schedule, state(at(2025, 1, 1, 9, 0))).unwrap();
        assert_eq!(
            outcome.decision,
            TickDecision::Enqueue(OccurrenceKey::new("nightly", at(2025, 1, 1, 10, 0)))
        );
    }
}
