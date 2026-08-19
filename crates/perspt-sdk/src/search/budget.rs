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
