//! Structured logging (SPEC.md section 5.5).
//!
//! Two sinks with different verbosity: a daily-rotating file that keeps the
//! noisy DEBUG/TRACE record, and an in-memory ring buffer of INFO-and-above
//! events that the Diagnostics UI can read without touching the disk.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tracing::Level;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

const DIAGNOSTIC_BUFFER_CAPACITY: usize = 500;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticEntry {
    pub timestamp: String,
    pub level: String,
    pub target: String,
    pub message: String,
}

/// Bounded history of user-visible log lines.
#[derive(Clone, Default)]
pub struct DiagnosticBuffer {
    entries: Arc<Mutex<VecDeque<DiagnosticEntry>>>,
}

impl DiagnosticBuffer {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(VecDeque::with_capacity(
                DIAGNOSTIC_BUFFER_CAPACITY,
            ))),
        }
    }

    fn push(&self, entry: DiagnosticEntry) {
        let Ok(mut entries) = self.entries.lock() else {
            return;
        };
        if entries.len() == DIAGNOSTIC_BUFFER_CAPACITY {
            entries.pop_front();
        }
        entries.push_back(entry);
    }

    pub fn snapshot(&self) -> Vec<DiagnosticEntry> {
        self.entries
            .lock()
            .map(|entries| entries.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn clear(&self) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.clear();
        }
    }
}

/// Captures the `message` field of an event, ignoring structured fields.
struct MessageVisitor(String);

impl tracing::field::Visit for MessageVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0 = format!("{value:?}").trim_matches('"').to_string();
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.0 = value.to_string();
        }
    }
}

struct DiagnosticLayer(DiagnosticBuffer);

impl<S: tracing::Subscriber> Layer<S> for DiagnosticLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let metadata = event.metadata();
        if *metadata.level() > Level::INFO {
            return;
        }

        let mut visitor = MessageVisitor(String::new());
        event.record(&mut visitor);

        self.0.push(DiagnosticEntry {
            timestamp: crate::db::now_iso8601(),
            level: metadata.level().to_string(),
            target: metadata.target().to_string(),
            message: visitor.0,
        });
    }
}

/// Guard that must stay alive for the process lifetime so buffered log lines
/// are flushed to disk.
pub struct LoggingHandle {
    pub buffer: DiagnosticBuffer,
    _file_guard: tracing_appender::non_blocking::WorkerGuard,
}

pub fn init(log_dir: &Path) -> LoggingHandle {
    let _ = std::fs::create_dir_all(log_dir);

    let file_appender = tracing_appender::rolling::daily(log_dir, "arcade-rom-router.log");
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);

    let buffer = DiagnosticBuffer::new();

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(file_writer)
        .with_ansi(false)
        .with_target(true)
        .with_filter(
            EnvFilter::try_from_env("ARR_LOG")
                .unwrap_or_else(|_| EnvFilter::new("info,arcade_rom_router_lib=debug")),
        );

    let console_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_filter(EnvFilter::new("info"));

    let registry = tracing_subscriber::registry()
        .with(file_layer)
        .with(console_layer)
        .with(DiagnosticLayer(buffer.clone()));

    // `try_init` rather than `init`: tests may install their own subscriber.
    let _ = registry.try_init();

    LoggingHandle {
        buffer,
        _file_guard: guard,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_keeps_only_the_most_recent_entries() {
        let buffer = DiagnosticBuffer::new();

        for i in 0..(DIAGNOSTIC_BUFFER_CAPACITY + 25) {
            buffer.push(DiagnosticEntry {
                timestamp: "t".into(),
                level: "INFO".into(),
                target: "test".into(),
                message: format!("entry {i}"),
            });
        }

        let snapshot = buffer.snapshot();
        assert_eq!(snapshot.len(), DIAGNOSTIC_BUFFER_CAPACITY);
        assert_eq!(snapshot[0].message, "entry 25");
        assert_eq!(
            snapshot.last().unwrap().message,
            format!("entry {}", DIAGNOSTIC_BUFFER_CAPACITY + 24)
        );
    }

    #[test]
    fn clearing_empties_the_buffer() {
        let buffer = DiagnosticBuffer::new();
        buffer.push(DiagnosticEntry {
            timestamp: "t".into(),
            level: "INFO".into(),
            target: "test".into(),
            message: "hello".into(),
        });

        buffer.clear();
        assert!(buffer.snapshot().is_empty());
    }
}
