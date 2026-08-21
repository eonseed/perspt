//! The `ModelTransport` port (PSP-9, Layer A).
//!
//! The SDK declares the port; `perspt-core` provides the `genai` driver;
//! `perspt-agent::transport` supplies the adapter. The tool loop is written
//! against `dyn ModelTransport`, so it can inspect provider-neutral
//! capabilities and model identity for routing and attribution, but it
//! cannot inspect vendor adapter types or credentials. This is what makes
//! Gate S structural rather than aspirational: a consumer who brings a
//! different transport never links `genai`.
//!
//! The boxed-future signature is deliberate: an `async fn` directly on the
//! trait would not be object-safe on this workspace's toolchain, and the SDK
//! stays free of any async runtime.

use std::future::Future;
use std::pin::Pin;

use super::capabilities::ProviderCapabilities;
use super::conversation::Conversation;
use super::family::ModelFamily;
use super::id::ModelId;
use super::tool::{ToolChoicePolicy, ToolSpec, TurnOutput};
use crate::error::Result;
use crate::prompt::PromptRoute;

/// Provider-neutral generation controls for a single model turn.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GenerationOptions {
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub stop_sequences: Vec<String>,
}

/// Boxed future returned by transport calls.
pub type TransportFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

/// The transport port: one assistant turn against one fully qualified model.
pub trait ModelTransport: Send + Sync {
    /// Run one chat turn: render the conversation for the route's provider,
    /// send the tool specs, and return either tool calls or text.
    ///
    /// The transport records nothing; observation recording is the caller's
    /// obligation (R2) so that replay never re-invokes a provider.
    fn chat_turn<'a>(
        &'a self,
        model: &'a ModelId,
        conversation: &'a Conversation,
        tools: &'a [ToolSpec],
        choice: ToolChoicePolicy,
    ) -> TransportFuture<'a, TurnOutput>;

    /// Run one turn with explicit generation controls. Implementations that
    /// cannot express them retain their ordinary turn behavior.
    fn chat_turn_with_options<'a>(
        &'a self,
        model: &'a ModelId,
        conversation: &'a Conversation,
        tools: &'a [ToolSpec],
        choice: ToolChoicePolicy,
        _options: GenerationOptions,
    ) -> TransportFuture<'a, TurnOutput> {
        self.chat_turn(model, conversation, tools, choice)
    }

    /// The route's capability record (declared or probed).
    fn capabilities(&self, model: &ModelId) -> ProviderCapabilities;

    /// The model's training lineage — a routing prior, never a measurement.
    fn family_of(&self, model: &ModelId) -> ModelFamily;

    /// The stable transport adapter identity (PSP-10 system 24). This is
    /// NOT the configured endpoint id: endpoint names identify locations
    /// and credentials, never model behavior. Scripted transports return
    /// `"scripted"`.
    fn adapter_kind(&self) -> &'static str;

    /// The prompt identity of one call: adapter kind, model family, and the
    /// exact model. A model served through a compatible gateway keeps its
    /// own family identity.
    fn prompt_route(&self, model: &ModelId) -> PromptRoute {
        PromptRoute {
            adapter: self.adapter_kind().to_owned(),
            family: self.family_of(model),
            exact_model: Some(model.model.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scripted transport: proves the port is object-safe and lets loop
    /// tests run without a network.
    struct Scripted(Vec<TurnOutput>);

    impl ModelTransport for Scripted {
        fn chat_turn<'a>(
            &'a self,
            _model: &'a ModelId,
            conversation: &'a Conversation,
            _tools: &'a [ToolSpec],
            _choice: ToolChoicePolicy,
        ) -> TransportFuture<'a, TurnOutput> {
            let index = conversation.len().min(self.0.len().saturating_sub(1));
            let output = self.0[index].clone();
            Box::pin(async move { Ok(output) })
        }

        fn capabilities(&self, _model: &ModelId) -> ProviderCapabilities {
            ProviderCapabilities::text_only(8192)
        }

        fn family_of(&self, model: &ModelId) -> ModelFamily {
            ModelFamily::from_model_name(&model.model)
        }

        fn adapter_kind(&self) -> &'static str {
            "scripted"
        }
    }

    #[test]
    fn the_port_is_object_safe() {
        let transport: Box<dyn ModelTransport> =
            Box::new(Scripted(vec![TurnOutput::Text("ok".into())]));
        let model = ModelId::new("test", "scripted");
        assert_eq!(
            transport.family_of(&model),
            ModelFamily::Other("scripted".into())
        );
        let route = transport.prompt_route(&model);
        assert_eq!(route.adapter, "scripted");
        assert_eq!(route.exact_model.as_deref(), Some("scripted"));
    }

    #[test]
    fn a_scripted_turn_resolves_without_a_runtime_dependency() {
        let transport = Scripted(vec![TurnOutput::Text("done".into())]);
        let model = ModelId::new("test", "scripted");
        let conversation = Conversation::with_system("s");
        let future = transport.chat_turn(&model, &conversation, &[], ToolChoicePolicy::Auto);
        // Poll to completion on a trivial executor: the SDK has no tokio.
        let output = futures_executor(future).unwrap();
        assert_eq!(output, TurnOutput::Text("done".into()));
    }

    /// Minimal single-future executor for tests (no async runtime in the SDK).
    fn futures_executor<T>(mut future: TransportFuture<'_, T>) -> Result<T> {
        use std::task::{Context, Poll, Waker};
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(value) => return value,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }
}
