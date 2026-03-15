use chrono::Local;
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Instant};
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

use crate::event_handler::EventHandler;

// ── Capacity ──────────────────────────────────────────────────────────────────
// const LOG_CHANNEL_CAPACITY: usize = 1024;

// ── Log Level ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "UPPERCASE")]
pub enum LogLevel {
    Trace = 0,
    Debug = 1,
    Info  = 2,
    Warn  = 3,
    Error = 4,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info  => "INFO ",
            LogLevel::Warn  => "WARN ",
            LogLevel::Error => "ERROR",
        };
        write!(f, "{}", s)
    }
}

// ── Log Event — what is broadcasted to each subscriber ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEvent {
    pub timestamp_ms: u128,    // Unix ms
    pub date: String,          // "2026-02-19 23:15:00.123"
    pub level: LogLevel,
    pub source: String,        // "users.repository" / "orders.domain"
    pub message: String,
}

// ── Event Emitter ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct EventEmitter {
    source: String,
    sender: broadcast::Sender<LogEvent>,
}

impl EventEmitter {
    pub fn new(source: impl Into<String>, log_channel_capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(log_channel_capacity);
        Self {
            source: source.into(),
            sender,
        }
    }

    pub fn child(&self, source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            sender: self.sender.clone(),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<LogEvent> {
        self.sender.subscribe()
    }

    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }

    pub fn emit(&self, level: LogLevel, message: impl Into<String>) {
        let _ = self.sender.send(LogEvent {
            timestamp_ms: Local::now().timestamp_millis() as u128,
            date: Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
            level,
            source: self.source.clone(),
            message: message.into(),
        });
    }

    pub fn trace(&self, msg: impl Into<String>) { self.emit(LogLevel::Trace, msg); }
    pub fn debug(&self, msg: impl Into<String>) { self.emit(LogLevel::Debug, msg); }
    pub fn info (&self, msg: impl Into<String>) { self.emit(LogLevel::Info,  msg); }
    pub fn warn (&self, msg: impl Into<String>) { self.emit(LogLevel::Warn,  msg); }
    pub fn error(&self, msg: impl Into<String>) { self.emit(LogLevel::Error, msg); }

    pub fn start_operation(&self, name: impl Into<String>) -> OperationTimer {
        let name = name.into();
        self.info(format!("▶️ Starting: {}", name));
        OperationTimer::new(name, self.clone())
    }

    pub fn event_handler<H: EventHandler>(&self, handler: H) -> &Self {
        let mut rx = self.sender.subscribe();
        let handler = Arc::new(handler);
    
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        if event.level < handler.min_level() {
                            continue;
                        }
                        if let Some(srcs) = handler.sources() {
                            if !srcs.iter().any(|s| event.source.starts_with(s)) {
                                continue;
                            }
                        }
                        // Dispatch
                        match event.level {
                            LogLevel::Trace => handler.on_trace(event).await,
                            LogLevel::Debug => handler.on_debug(event).await,
                            LogLevel::Info  => handler.on_info(event).await,
                            LogLevel::Warn  => handler.on_warn(event).await,
                            LogLevel::Error => handler.on_error(event).await,
                        }
                    }
                    Err(RecvError::Lagged(n)) => {
                        eprintln!(
                            "[EventEmitter] '{}' lagged, missed {} events",
                            handler.name(), n
                        );
                    }
                    Err(RecvError::Closed) => break,
                }
            }
        });
    
        self
    }
}

// ── Operation Timer ───────────────────────────────────────────────────────────

pub struct OperationTimer {
    name: String,
    start: Instant,
    emitter: EventEmitter,
}

impl OperationTimer {
    fn new(name: String, emitter: EventEmitter) -> Self {
        Self { name, start: Instant::now(), emitter }
    }

    pub fn elapsed_ms(&self) -> u128 {
        self.start.elapsed().as_millis()
    }

    pub fn finish(self, success: bool) {
        let elapsed = self.start.elapsed();
        self.emitter.info(format!(
            "{} Finished: {} ({:.3}s / {}ms)",
            if success { "✅" } else { "❌" },
            self.name,
            elapsed.as_secs_f64(),
            elapsed.as_millis()
        ));
    }
}
