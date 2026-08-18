//! Actor-turn-runner mechanism checks (PSP-10 system 27, Gates Z/AD/AF
//! prerequisites; Phase 9).
//!
//! Every stochastic actor shares one turn discipline: the raw observation
//! is recorded with its actor tag before any parse; the sticky failover
//! chain serves every actor, not only the worker; and an infeasible
//! context budget makes no model call at all.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use perspt_agent::toolloop::{LoopEvent, LoopRecorder};
use perspt_agent::turn::{transport_failure_kind, ActorKind, ActorTurnRunner};
use perspt_sdk::{
    Conversation, FailureKind, ModelFamily, ModelId, ModelTransport, ProviderCapabilities,
    ToolChoicePolicy, ToolSpec, TransportFuture, TurnOutput,
};

#[derive(Default)]
struct Recording {
    events: Mutex<Vec<LoopEvent>>,
}

impl LoopRecorder for Recording {
    fn record(&self, event: &LoopEvent) -> anyhow::Result<()> {
        self.events.lock().unwrap().push(event.clone());
        Ok(())
    }
}

/// Fails on the primary route, succeeds on any other.
struct FailPrimary {
    primary: String,
    calls: AtomicU32,
    window: u32,
}

impl ModelTransport for FailPrimary {
    fn chat_turn<'a>(
        &'a self,
        model: &'a ModelId,
        _conversation: &'a Conversation,
        _tools: &'a [ToolSpec],
        _choice: ToolChoicePolicy,
    ) -> TransportFuture<'a, TurnOutput> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let fail = model.model == self.primary;
        Box::pin(async move {
            if fail {
                return Err(perspt_sdk::SdkError::Domain(
                    "primary transport down".into(),
                ));
            }
            Ok(TurnOutput::Text("observed".into()))
        })
    }

    fn capabilities(&self, _model: &ModelId) -> ProviderCapabilities {
        ProviderCapabilities::text_only(self.window)
    }

    fn family_of(&self, model: &ModelId) -> ModelFamily {
        ModelFamily::Other(model.model.clone())
    }

    fn adapter_kind(&self) -> &'static str {
        "scripted"
    }
}

/// Every actor's raw observation is recorded, tagged, before the caller
/// sees the output; the shared failover chain serves non-worker actors.
#[tokio::test]
async fn every_actor_records_a_tagged_observation_after_shared_failover() {
    for actor in [
        ActorKind::Explorer,
        ActorKind::Architect,
        ActorKind::Adjudicator,
        ActorKind::Summarizer,
        ActorKind::CapabilityProbe,
    ] {
        let transport = FailPrimary {
            primary: "alpha".into(),
            calls: AtomicU32::new(0),
            window: 100_000,
        };
        let recording = Recording::default();
        let mut runner = ActorTurnRunner {
            transport: &transport,
            model: ModelId::new("test", "alpha"),
            fallbacks: vec![ModelId::new("test", "beta")],
            recorder: Some(&recording),
            actor,
            turn: 1,
        };
        let conversation = Conversation::with_system("probe");
        let output = runner
            .run_turn(&conversation, &[], ToolChoicePolicy::None)
            .await
            .unwrap();
        assert_eq!(output, TurnOutput::Text("observed".into()));
        let events = recording.events.lock().unwrap();
        assert!(matches!(events[0], LoopEvent::RouteFailover { .. }));
        match &events[1] {
            LoopEvent::TurnObserved {
                actor: tag, output, ..
            } => {
                assert_eq!(tag, actor.tag());
                assert_eq!(*output, TurnOutput::Text("observed".into()));
            }
            other => panic!("expected TurnObserved, got {other:?}"),
        }
    }
}

/// The chain is sticky and finite: with no remaining fallback the failure
/// surfaces instead of retrying forever.
#[tokio::test]
async fn an_exhausted_chain_surfaces_the_failure() {
    let transport = FailPrimary {
        primary: "alpha".into(),
        calls: AtomicU32::new(0),
        window: 100_000,
    };
    let recording = Recording::default();
    let mut runner = ActorTurnRunner {
        transport: &transport,
        model: ModelId::new("test", "alpha"),
        fallbacks: Vec::new(),
        recorder: Some(&recording),
        actor: ActorKind::Adjudicator,
        turn: 1,
    };
    let conversation = Conversation::with_system("probe");
    let error = runner
        .run_turn(&conversation, &[], ToolChoicePolicy::None)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("no remaining fallback"));
    assert_eq!(transport.calls.load(Ordering::SeqCst), 1);
}

/// Gate AF: an infeasible context budget records `context_infeasible` and
/// makes no transport call at all.
#[tokio::test]
async fn an_infeasible_context_makes_no_model_call() {
    let transport = FailPrimary {
        primary: "never".into(),
        calls: AtomicU32::new(0),
        window: 1_400, // allowance = 1400 - 1024 - 256 = 120 tokens
    };
    let recording = Recording::default();
    let mut runner = ActorTurnRunner {
        transport: &transport,
        model: ModelId::new("test", "alpha"),
        fallbacks: Vec::new(),
        recorder: Some(&recording),
        actor: ActorKind::Explorer,
        turn: 3,
    };
    let mut conversation = Conversation::with_system("system");
    conversation.push_user("x".repeat(4_000));
    let error = runner
        .run_turn(&conversation, &[], ToolChoicePolicy::Auto)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("no model call was made"));
    assert_eq!(
        transport.calls.load(Ordering::SeqCst),
        0,
        "infeasible means zero transport calls"
    );
    let events = recording.events.lock().unwrap();
    assert!(matches!(
        events[0],
        LoopEvent::ContextInfeasible { turn: 3, .. }
    ));
}

/// One failure taxonomy for every actor.
#[test]
fn the_failure_classification_is_shared() {
    assert_eq!(
        transport_failure_kind("HTTP 429 Too Many Requests"),
        FailureKind::ProviderRateLimit
    );
    assert_eq!(
        transport_failure_kind("connection reset by peer"),
        FailureKind::ProviderTransport
    );
}
