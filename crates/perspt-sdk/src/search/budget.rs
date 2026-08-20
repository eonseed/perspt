//! Search limits, monotone usage, and the reservation discipline
//! (PSP-10 Definition 5, system 20, Gate AC).
//!
//! Every branch action reserves resources before it starts and always
//! consumes one nonrefundable action unit. Unused resource reservations
//! may be released; consumed usage never decreases — there is no public
//! decrement. A closed forest cannot reopen: closing consumes the budget
//! by type.

use serde::{Deserialize, Serialize};

use crate::error::{Result, SdkError};

/// The forest's finite limits (Definition 5). One set per forest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchLimits {
    pub actions: u32,
    pub model_turns: u32,
    pub tool_calls: u32,
    pub mutations: u32,
    pub verifier_runs: u32,
    pub tokens: u64,
    pub elapsed_secs: u64,
    pub result_bytes: u64,
    /// Cumulative eager-copy file reservations across every fork.
    pub workspace_files: u64,
    /// Cumulative eager-copy byte reservations across every fork.
    pub workspace_bytes: u64,
}

impl SearchLimits {
    /// A finite release-default limit set (system 20's starting posture).
    pub fn release_default() -> Self {
        Self {
            actions: 64,
            model_turns: 24,
            tool_calls: 192,
            mutations: 96,
            verifier_runs: 48,
            tokens: 400_000,
            elapsed_secs: 1_800,
            result_bytes: 16 * 1024 * 1024,
            workspace_files: 300_000,
            workspace_bytes: 4 * 1024 * 1024 * 1024,
        }
    }
}

/// One action's resource request, reserved before the action starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ReservationRequest {
    pub model_turns: u32,
    pub tool_calls: u32,
    pub mutations: u32,
    pub verifier_runs: u32,
    pub tokens: u64,
    pub result_bytes: u64,
    pub workspace_files: u64,
    pub workspace_bytes: u64,
}

/// Proof one reservation was taken; releasing unused amounts requires it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReservationTicket {
    request: ReservationRequest,
}

impl ReservationTicket {
    pub fn request(&self) -> &ReservationRequest {
        &self.request
    }
}

/// Monotone usage. All fields are reserved-or-consumed totals; nothing
/// public decrements except [`SearchUsage::release_unused`], which may only
/// return part of a held ticket — never below what was actually consumed.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SearchUsage {
    pub actions: u32,
    pub model_turns: u32,
    pub tool_calls: u32,
    pub mutations: u32,
    pub verifier_runs: u32,
    pub tokens: u64,
    /// Wall-clock seconds charged against `SearchLimits::elapsed_secs`
    /// (Definition 5's time dimension; monotone like every other field).
    #[serde(default)]
    pub elapsed_secs: u64,
    pub result_bytes: u64,
    pub workspace_files: u64,
    pub workspace_bytes: u64,
    closed: bool,
}

impl SearchUsage {
    /// Reserve one action's resources. Consumes one action unit — action
    /// units are never returned — and fails without reserving anything if
    /// any limit would be exceeded or the forest is closed.
    pub fn reserve(
        &mut self,
        limits: &SearchLimits,
        request: ReservationRequest,
    ) -> Result<ReservationTicket> {
        if self.closed {
            return Err(SdkError::Domain(
                "search_closed: a closed forest cannot act".into(),
            ));
        }
        let over = self.actions + 1 > limits.actions
            || self.model_turns + request.model_turns > limits.model_turns
            || self.tool_calls + request.tool_calls > limits.tool_calls
            || self.mutations + request.mutations > limits.mutations
            || self.verifier_runs + request.verifier_runs > limits.verifier_runs
            || self.tokens + request.tokens > limits.tokens
            || self.result_bytes + request.result_bytes > limits.result_bytes
            || self.workspace_files + request.workspace_files > limits.workspace_files
            || self.workspace_bytes + request.workspace_bytes > limits.workspace_bytes;
        if over {
            return Err(SdkError::Domain(
                "search budget exceeded; the forest must close".into(),
            ));
        }
        self.actions += 1;
        self.model_turns += request.model_turns;
        self.tool_calls += request.tool_calls;
        self.mutations += request.mutations;
        self.verifier_runs += request.verifier_runs;
        self.tokens += request.tokens;
        self.result_bytes += request.result_bytes;
        self.workspace_files += request.workspace_files;
        self.workspace_bytes += request.workspace_bytes;
        Ok(ReservationTicket { request })
    }

    /// Charge consumed actuals a reservation could not know in advance
    /// (observed turns, calls, mutations, verifier runs, tokens, bytes).
    /// Monotone; crossing any limit closes the forest by type — the error
    /// is terminal for further reservations.
    pub fn charge(&mut self, limits: &SearchLimits, consumed: ReservationRequest) -> Result<()> {
        self.model_turns += consumed.model_turns;
        self.tool_calls += consumed.tool_calls;
        self.mutations += consumed.mutations;
        self.verifier_runs += consumed.verifier_runs;
        self.tokens += consumed.tokens;
        self.result_bytes += consumed.result_bytes;
        self.workspace_files += consumed.workspace_files;
        self.workspace_bytes += consumed.workspace_bytes;
        self.ensure_within(limits)
    }

    /// Charge one wall-clock interval (Definition 5's time dimension).
    pub fn charge_elapsed(&mut self, limits: &SearchLimits, seconds: u64) -> Result<()> {
        self.elapsed_secs += seconds;
        self.ensure_within(limits)
    }

    fn ensure_within(&mut self, limits: &SearchLimits) -> Result<()> {
        let over = self.model_turns > limits.model_turns
            || self.tool_calls > limits.tool_calls
            || self.mutations > limits.mutations
            || self.verifier_runs > limits.verifier_runs
            || self.tokens > limits.tokens
            || self.elapsed_secs > limits.elapsed_secs
            || self.result_bytes > limits.result_bytes
            || self.workspace_files > limits.workspace_files
            || self.workspace_bytes > limits.workspace_bytes;
        if over {
            self.close();
            return Err(SdkError::Domain(
                "search budget exhausted by consumed actuals; the forest closed".into(),
            ));
        }
        Ok(())
    }

    /// Release the unused part of a held reservation. The action unit is
    /// never returned; each dimension may release at most what the ticket
    /// reserved.
    pub fn release_unused(&mut self, ticket: ReservationTicket, consumed: ReservationRequest) {
        let held = ticket.request;
        self.model_turns -= held
            .model_turns
            .saturating_sub(consumed.model_turns.min(held.model_turns));
        self.tool_calls -= held
            .tool_calls
            .saturating_sub(consumed.tool_calls.min(held.tool_calls));
        self.mutations -= held
            .mutations
            .saturating_sub(consumed.mutations.min(held.mutations));
        self.verifier_runs -= held
            .verifier_runs
            .saturating_sub(consumed.verifier_runs.min(held.verifier_runs));
        self.tokens -= held.tokens.saturating_sub(consumed.tokens.min(held.tokens));
        self.result_bytes -= held
            .result_bytes
            .saturating_sub(consumed.result_bytes.min(held.result_bytes));
        self.workspace_files -= held
            .workspace_files
            .saturating_sub(consumed.workspace_files.min(held.workspace_files));
        self.workspace_bytes -= held
            .workspace_bytes
            .saturating_sub(consumed.workspace_bytes.min(held.workspace_bytes));
    }

    /// Close the forest. Terminal: no reservation succeeds afterwards.
    pub fn close(&mut self) {
        self.closed = true;
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }
}

/// Add with a ceiling; reports whether the ceiling truncated the add.
fn add_capped_u32(current: &mut u32, extra: u32, cap: u32) -> bool {
    let target = current.saturating_add(extra);
    *current = target.min(cap);
    target > cap
}

fn add_capped_u64(current: &mut u64, extra: u64, cap: u64) -> bool {
    let target = current.saturating_add(extra);
    *current = target.min(cap);
    target > cap
}

/// One forest's usage behind a shared handle, so the branch tool loops can
/// reserve before every action while the forest holds the same ledgered
/// totals (Gate AC: reservations precede actions; usage never decreases;
/// usage never persists above a limit — overflow clamps at the limit and
/// closes the forest).
#[derive(Debug, Clone)]
pub struct SharedSearchBudget {
    inner: std::sync::Arc<std::sync::Mutex<SearchUsage>>,
    limits: SearchLimits,
    denied: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// The owning forest's `(forest_id, epoch)`, so every holder of the
    /// handle can ledger a usage snapshot under the right identity —
    /// crash-durable accounting needs snapshots at action granularity,
    /// not only at branch boundaries.
    identity: std::sync::Arc<std::sync::Mutex<(String, u64)>>,
}

impl SharedSearchBudget {
    pub fn new(limits: SearchLimits) -> Self {
        Self::with_usage(limits, SearchUsage::default())
    }

    /// Open with prior usage (resume: an interrupted forest's consumption
    /// is not silently forgotten).
    pub fn with_usage(limits: SearchLimits, usage: SearchUsage) -> Self {
        Self {
            inner: std::sync::Arc::new(std::sync::Mutex::new(usage)),
            limits,
            denied: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            identity: std::sync::Arc::new(std::sync::Mutex::new((String::new(), 0))),
        }
    }

    /// Bind the owning forest's identity (set once at forest opening).
    pub fn bind_forest(&self, forest_id: &str) {
        self.identity.lock().expect("identity lock").0 = forest_id.to_string();
    }

    /// Advance the identity's epoch (set at each frontier epoch opening).
    pub fn set_epoch(&self, epoch: u64) {
        self.identity.lock().expect("identity lock").1 = epoch;
    }

    /// The bound `(forest_id, epoch)`; the forest id is empty outside a
    /// forest.
    pub fn identity(&self) -> (String, u64) {
        self.identity.lock().expect("identity lock").clone()
    }

    pub fn limits(&self) -> &SearchLimits {
        &self.limits
    }

    /// Reserve one action's resources before it starts. A refusal marks
    /// the handle denied so the forest can abandon the branch gracefully.
    pub fn reserve(&self, request: ReservationRequest) -> Result<ReservationTicket> {
        let mut usage = self.inner.lock().expect("budget lock");
        usage.reserve(&self.limits, request).inspect_err(|_| {
            self.denied.store(true, std::sync::atomic::Ordering::SeqCst);
        })
    }

    /// Settle one action: release the unused part of the ticket; charge any
    /// overshoot clamped at the limits. Overshoot closes the forest and
    /// errs — the caller ledgers the overflow as an observation.
    pub fn settle(&self, ticket: ReservationTicket, actual: ReservationRequest) -> Result<()> {
        let mut usage = self.inner.lock().expect("budget lock");
        let held = *ticket.request();
        usage.release_unused(ticket, actual);
        let overshoot = ReservationRequest {
            model_turns: actual.model_turns.saturating_sub(held.model_turns),
            tool_calls: actual.tool_calls.saturating_sub(held.tool_calls),
            mutations: actual.mutations.saturating_sub(held.mutations),
            verifier_runs: actual.verifier_runs.saturating_sub(held.verifier_runs),
            tokens: actual.tokens.saturating_sub(held.tokens),
            result_bytes: actual.result_bytes.saturating_sub(held.result_bytes),
            workspace_files: actual.workspace_files.saturating_sub(held.workspace_files),
            workspace_bytes: actual.workspace_bytes.saturating_sub(held.workspace_bytes),
        };
        if overshoot == ReservationRequest::default() {
            return Ok(());
        }
        self.charge_clamped(&mut usage, overshoot)
    }

    fn charge_clamped(&self, usage: &mut SearchUsage, extra: ReservationRequest) -> Result<()> {
        let limits = &self.limits;
        let mut over = false;
        over |= add_capped_u32(
            &mut usage.model_turns,
            extra.model_turns,
            limits.model_turns,
        );
        over |= add_capped_u32(&mut usage.tool_calls, extra.tool_calls, limits.tool_calls);
        over |= add_capped_u32(&mut usage.mutations, extra.mutations, limits.mutations);
        over |= add_capped_u32(
            &mut usage.verifier_runs,
            extra.verifier_runs,
            limits.verifier_runs,
        );
        over |= add_capped_u64(&mut usage.tokens, extra.tokens, limits.tokens);
        over |= add_capped_u64(
            &mut usage.result_bytes,
            extra.result_bytes,
            limits.result_bytes,
        );
        over |= add_capped_u64(
            &mut usage.workspace_files,
            extra.workspace_files,
            limits.workspace_files,
        );
        over |= add_capped_u64(
            &mut usage.workspace_bytes,
            extra.workspace_bytes,
            limits.workspace_bytes,
        );
        if over {
            usage.close();
            return Err(SdkError::Domain(
                "consumed actuals crossed a search limit; charged to the limit and closed".into(),
            ));
        }
        Ok(())
    }

    /// Charge one wall-clock interval, clamped at the limit; crossing it
    /// closes the forest.
    pub fn charge_elapsed(&self, seconds: u64) -> Result<()> {
        let mut usage = self.inner.lock().expect("budget lock");
        let over = add_capped_u64(&mut usage.elapsed_secs, seconds, self.limits.elapsed_secs);
        if over {
            usage.close();
            return Err(SdkError::Domain(
                "elapsed time crossed the search limit; the forest closed".into(),
            ));
        }
        Ok(())
    }

    /// Non-consuming check: would this reservation be admitted right now?
    pub fn headroom(&self, request: &ReservationRequest) -> bool {
        let usage = self.inner.lock().expect("budget lock");
        let limits = &self.limits;
        !usage.is_closed()
            && usage.actions < limits.actions
            && usage.model_turns + request.model_turns <= limits.model_turns
            && usage.tool_calls + request.tool_calls <= limits.tool_calls
            && usage.verifier_runs + request.verifier_runs <= limits.verifier_runs
            && usage.tokens + request.tokens <= limits.tokens
    }

    pub fn snapshot(&self) -> SearchUsage {
        self.inner.lock().expect("budget lock").clone()
    }

    /// Terminal snapshot for `SearchClosed`; the forest cannot reopen.
    pub fn close(&self) -> SearchUsage {
        let mut usage = self.inner.lock().expect("budget lock");
        usage.close();
        usage.clone()
    }

    pub fn is_closed(&self) -> bool {
        self.inner.lock().expect("budget lock").is_closed()
    }

    /// Whether a reservation was refused since the last call (one-shot).
    pub fn take_denied(&self) -> bool {
        self.denied.swap(false, std::sync::atomic::Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> SearchLimits {
        SearchLimits {
            actions: 2,
            model_turns: 2,
            tool_calls: 4,
            mutations: 4,
            verifier_runs: 2,
            tokens: 1_000,
            elapsed_secs: 60,
            result_bytes: 1_000,
            workspace_files: 10,
            workspace_bytes: 1_000,
        }
    }

    #[test]
    fn reservation_precedes_action_and_units_are_nonrefundable() {
        let mut usage = SearchUsage::default();
        let ticket = usage
            .reserve(
                &limits(),
                ReservationRequest {
                    tokens: 600,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(usage.actions, 1);
        assert_eq!(usage.tokens, 600);
        // Release everything unused: tokens return, the action unit stays.
        usage.release_unused(ticket, ReservationRequest::default());
        assert_eq!(usage.tokens, 0);
        assert_eq!(usage.actions, 1, "action units are never returned");
    }

    #[test]
    fn limits_refuse_without_partial_reservation() {
        let mut usage = SearchUsage::default();
        usage
            .reserve(
                &limits(),
                ReservationRequest {
                    tokens: 900,
                    ..Default::default()
                },
            )
            .unwrap();
        let before = usage.clone();
        assert!(usage
            .reserve(
                &limits(),
                ReservationRequest {
                    tokens: 200,
                    ..Default::default()
                },
            )
            .is_err());
        assert_eq!(usage, before, "a refused reservation reserves nothing");
    }

    #[test]
    fn consumed_actuals_and_elapsed_time_close_the_forest_at_the_limit() {
        let mut usage = SearchUsage::default();
        usage
            .charge(
                &limits(),
                ReservationRequest {
                    model_turns: 1,
                    tool_calls: 2,
                    tokens: 500,
                    ..Default::default()
                },
            )
            .unwrap();
        usage.charge_elapsed(&limits(), 59).unwrap();
        // The next interval crosses elapsed_secs = 60: terminal.
        assert!(usage.charge_elapsed(&limits(), 2).is_err());
        assert!(usage.is_closed(), "crossing a limit closes the forest");
        assert!(usage
            .reserve(&limits(), ReservationRequest::default())
            .is_err());
    }

    #[test]
    fn a_closed_forest_cannot_reopen() {
        let mut usage = SearchUsage::default();
        usage.close();
        assert!(usage.is_closed());
        assert!(usage
            .reserve(&limits(), ReservationRequest::default())
            .is_err());
    }

    #[test]
    fn shared_budget_reserves_settles_and_never_persists_over_limit() {
        let shared = SharedSearchBudget::new(limits());
        let ticket = shared
            .reserve(ReservationRequest {
                model_turns: 1,
                tokens: 400,
                ..Default::default()
            })
            .unwrap();
        // Actuals overshoot the ticket's tokens past the 1_000 limit:
        // charged to the limit, closed, and reported.
        let result = shared.settle(
            ticket,
            ReservationRequest {
                model_turns: 1,
                tokens: 1_500,
                ..Default::default()
            },
        );
        assert!(result.is_err());
        let usage = shared.snapshot();
        assert!(
            usage.tokens <= limits().tokens,
            "usage never sits above a limit"
        );
        assert!(shared.is_closed());
        assert!(shared.reserve(ReservationRequest::default()).is_err());
    }

    #[test]
    fn shared_budget_denial_is_observable_once() {
        let shared = SharedSearchBudget::new(limits());
        assert!(!shared.take_denied());
        let _ = shared.reserve(ReservationRequest {
            tokens: 5_000,
            ..Default::default()
        });
        assert!(shared.take_denied(), "the refusal is observable");
        assert!(!shared.take_denied(), "and one-shot");
    }

    #[test]
    fn shared_budget_headroom_is_non_consuming() {
        let shared = SharedSearchBudget::new(limits());
        let probe = ReservationRequest {
            model_turns: 1,
            ..Default::default()
        };
        assert!(shared.headroom(&probe));
        assert_eq!(shared.snapshot(), SearchUsage::default());
    }

    #[test]
    fn the_action_limit_ends_search() {
        let mut usage = SearchUsage::default();
        for _ in 0..2 {
            usage
                .reserve(&limits(), ReservationRequest::default())
                .unwrap();
        }
        assert!(usage
            .reserve(&limits(), ReservationRequest::default())
            .is_err());
    }
}
