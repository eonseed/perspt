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

    /// The route's capability record (declared or probed).
    fn capabilities(&self, model: &ModelId) -> ProviderCapabilities;

    /// The model's training lineage — a routing prior, never a measurement.
    fn family_of(&self, model: &ModelId) -> ModelFamily;
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
