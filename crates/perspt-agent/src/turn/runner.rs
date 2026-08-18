//! The actor turn runner (PSP-10 system 27).

use anyhow::Result;
use perspt_sdk::prompt::{ContextBudget, TokenAccountantRef};
use perspt_sdk::{
    Conversation, FailureKind, ModelId, ModelTransport, ToolChoicePolicy, ToolSpec, TurnOutput,
};

use crate::toolloop::{LoopEvent, LoopRecorder};

/// The stochastic actors one runner serves (system 27).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorKind {
    Worker,
    Explorer,
    Architect,
    Adjudicator,
    Summarizer,
    CapabilityProbe,
}

impl ActorKind {
    pub fn tag(&self) -> &'static str {
        match self {
            ActorKind::Worker => "worker",
            ActorKind::Explorer => "explorer",
            ActorKind::Architect => "architect",
            ActorKind::Adjudicator => "adjudicator",
            ActorKind::Summarizer => "summarizer",
            ActorKind::CapabilityProbe => "capability_probe",
        }
    }
}

/// The default per-turn wall-clock deadline when none is configured.
pub const DEFAULT_TURN_DEADLINE_SECS: u64 = 120;

/// One transport call under a hard wall-clock deadline — the one door every
/// actor's model turn goes through. A finite turn count alone never bounds
/// wall time; the deadline does. Exceeding it is an ordinary transport
/// failure (it consumes sticky failover like a provider outage), and
/// dropping the timed future cancels the underlying request.
pub async fn chat_turn_with_deadline(
    transport: &dyn ModelTransport,
    model: &ModelId,
    conversation: &Conversation,
    tools: &[ToolSpec],
    choice: ToolChoicePolicy,
    deadline_secs: u64,
) -> perspt_sdk::Result<TurnOutput> {
    let deadline = std::time::Duration::from_secs(deadline_secs.max(1));
    match tokio::time::timeout(
        deadline,
        transport.chat_turn(model, conversation, tools, choice),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => Err(perspt_sdk::SdkError::Domain(format!(
            "transport deadline: no turn from {model} within {deadline_secs}s"
        ))),
    }
}

/// Classify a transport failure cause — the one definition the worker loop
/// and the runner share, so no actor invents its own retry taxonomy.
pub fn transport_failure_kind(cause: &str) -> FailureKind {
    let lowered = cause.to_ascii_lowercase();
    let rate_limited = ["rate limit", "429", "too many requests"]
        .iter()
        .any(|marker| lowered.contains(marker));
    if rate_limited {
        FailureKind::ProviderRateLimit
    } else {
        FailureKind::ProviderTransport
    }
}

/// One actor's turn discipline: budget feasibility, sticky failover, and
/// raw-before-parse observation recording.
pub struct ActorTurnRunner<'a> {
    pub transport: &'a dyn ModelTransport,
    pub model: ModelId,
    /// Sticky failover chain, consumed only on observed transport failure.
    pub fallbacks: Vec<ModelId>,
    pub recorder: Option<&'a dyn LoopRecorder>,
    pub actor: ActorKind,
    /// Turn ordinal within this actor's own exchange.
    pub turn: u32,
    /// Per-call wall-clock deadline in seconds ([`DEFAULT_TURN_DEADLINE_SECS`]
    /// when unconfigured).
    pub deadline_secs: u64,
}

impl ActorTurnRunner<'_> {
    fn emit(&self, event: LoopEvent) -> Result<()> {
        if let Some(recorder) = self.recorder {
            recorder.record(&event)?;
        }
        Ok(())
    }

    /// Definition 6's refusal: the composed request must fit the route's
    /// input allowance under the versioned accountant; an infeasible
    /// request makes no model call and records `ContextInfeasible`.
    fn ensure_feasible(&self, conversation: &Conversation, tools: &[ToolSpec]) -> Result<()> {
        let capabilities = self.transport.capabilities(&self.model);
        let accountant = TokenAccountantRef::approx_bytes_v1();
        let tool_reserve: u64 = tools
            .iter()
            .map(|spec| {
                accountant.count_text(&format!("{}{}{}", spec.name, spec.description, spec.schema))
            })
            .sum();
        let budget = ContextBudget {
            window_tokens: u64::from(capabilities.max_context_tokens.max(1)),
            output_reserve: 1_024,
            tool_reserve,
            guard_reserve: 256,
        };
        let allowance = budget
            .input_allowance()
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let required: u64 = conversation
            .messages()
            .iter()
            .map(|message| accountant.count_message(&format!("{message:?}")))
            .sum();
        if required > allowance {
            self.emit(LoopEvent::ContextInfeasible {
                forest_id: String::new(),
                branch_id: String::new(),
                turn: self.turn,
                required,
                allowance,
            })?;
            anyhow::bail!(
                "context budget infeasible for {}: {required} tokens over the \
                 {allowance}-token input allowance; no model call was made",
                self.actor.tag()
            );
        }
        Ok(())
    }

    /// Run one turn: feasibility, transport with the shared sticky
    /// failover, then the raw actor-tagged observation — recorded before
    /// any caller inspects the output (system 27 step 4).
    pub async fn run_turn(
        &mut self,
        conversation: &Conversation,
        tools: &[ToolSpec],
        choice: ToolChoicePolicy,
    ) -> Result<TurnOutput> {
        self.ensure_feasible(conversation, tools)?;
        let output = loop {
            match chat_turn_with_deadline(
                self.transport,
                &self.model,
                conversation,
                tools,
                choice.clone(),
                self.deadline_secs,
            )
            .await
            {
                Ok(output) => break output,
                Err(error) => {
                    let cause = error.to_string();
                    let _failure = transport_failure_kind(&cause);
                    let Some(next) = self.fallbacks.first().cloned() else {
                        anyhow::bail!(
                            "{} transport failed with no remaining fallback: {cause}",
                            self.actor.tag()
                        );
                    };
                    self.fallbacks.remove(0);
                    self.emit(LoopEvent::RouteFailover {
                        from_model: self.model.clone(),
                        to_model: next.clone(),
                        cause,
                    })?;
                    self.model = next;
                }
            }
        };
        self.emit(LoopEvent::TurnObserved {
            turn: self.turn,
            actor: self.actor.tag().to_string(),
            output: output.clone(),
        })?;
        Ok(output)
    }
}
