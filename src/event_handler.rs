use async_trait::async_trait;
use crate::event_emitter::{LogEvent, LogLevel};

#[async_trait]
pub trait EventHandler: Send + Sync + 'static {
    fn sources(&self) -> Option<&[&'static str]> { None }
    fn min_level(&self) -> LogLevel { LogLevel::Trace }
    fn name(&self) -> &str { std::any::type_name::<Self>() }
    async fn on_trace(&self, _event: LogEvent) {}
    async fn on_debug(&self, _event: LogEvent) {}
    async fn on_info (&self, _event: LogEvent) {}
    async fn on_warn (&self, _event: LogEvent) {}
    async fn on_error(&self, _event: LogEvent) {}
}
