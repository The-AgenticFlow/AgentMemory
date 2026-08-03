use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{sync::mpsc, thread};
use tracing::Level;
use tracing_subscriber::{EnvFilter, Layer, layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Debug, Clone)]
pub struct LokiSink {
    sender: mpsc::Sender<LokiEntry>,
}

#[derive(Debug, Clone)]
struct LokiEntry {
    timestamp_ns: String,
    line: String,
    labels: std::collections::HashMap<String, String>,
}

#[derive(Serialize)]
struct LokiStreamRequest {
    streams: Vec<LokiStream>,
}

#[derive(Serialize)]
struct LokiStream {
    stream: std::collections::HashMap<String, String>,
    /// Each entry must be a `[timestamp_ns, line]` pair per the Loki push API.
    values: Vec<[String; 2]>,
}

impl LokiSink {
    pub fn new(url: String) -> Self {
        let (sender, receiver) = mpsc::channel::<LokiEntry>();

        thread::spawn(move || {
            // Each background thread owns its own Tokio runtime so log shipping
            // is independent of the main async runtime lifetime.
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    eprintln!("Loki sink failed to create runtime: {e}");
                    return;
                }
            };

            rt.block_on(loki_forwarder(url, receiver));
        });

        Self { sender }
    }
}

async fn loki_forwarder(url: String, receiver: mpsc::Receiver<LokiEntry>) {
    let client = reqwest::Client::new();
    let url = format!("{}/loki/api/v1/push", url.trim_end_matches('/'));

    while let Ok(entry) = receiver.recv() {
        let mut stream = entry.labels.clone();
        stream.insert("level".to_string(), level_label(&entry.line));

        let request = LokiStreamRequest {
            streams: vec![LokiStream {
                stream,
                values: vec![[entry.timestamp_ns, entry.line]],
            }],
        };

        match client.post(&url).json(&request).send().await {
            Ok(resp) if !resp.status().is_success() => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                eprintln!("Loki push rejected: {status} {body}");
            }
            Err(e) => eprintln!("Loki send failed: {e}"),
            _ => {}
        }
    }
}

fn level_label(line: &str) -> String {
    // tracing_subscriber fmt output places the level after the timestamp.
    if line.contains(" ERROR ") {
        "error"
    } else if line.contains(" WARN ") {
        "warn"
    } else if line.contains(" INFO ") {
        "info"
    } else if line.contains(" DEBUG ") {
        "debug"
    } else if line.contains(" TRACE ") {
        "trace"
    } else {
        "unknown"
    }
    .to_string()
}

pub fn init_loki_sink(loki_url: String) {
    if loki_url.is_empty() {
        return;
    }

    let sink = LokiSink::new(loki_url);
    let layer = LokiLayer { sink };

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let _ = tracing_subscriber::registry()
        .with(env_filter)
        .with(layer)
        .try_init();
}

struct LokiLayer {
    sink: LokiSink,
}

impl<S: tracing::Subscriber> Layer<S> for LokiLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let metadata = event.metadata();
        let level = match *metadata.level() {
            Level::TRACE => "TRACE",
            Level::DEBUG => "DEBUG",
            Level::INFO => "INFO",
            Level::WARN => "WARN",
            Level::ERROR => "ERROR",
        };

        let mut message = String::new();

        struct Visitor<'a> {
            message: &'a mut String,
        }

        impl<'a> tracing::field::Visit for Visitor<'a> {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                if field.name() == "message" {
                    *self.message = format!("{:?}", value);
                }
            }

            fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                if field.name() == "message" {
                    *self.message = value.to_string();
                }
            }
        }

        let mut visitor = Visitor {
            message: &mut message,
        };
        event.record(&mut visitor);

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .to_string();

        let target = metadata.target();
        let log_line = if message.is_empty() {
            format!("{} {} {}", chrono::Utc::now().to_rfc3339(), level, target)
        } else {
            format!(
                "{} {} {}: {}",
                chrono::Utc::now().to_rfc3339(),
                level,
                target,
                message
            )
        };

        let mut labels = std::collections::HashMap::new();
        labels.insert("service".to_string(), "engram".to_string());
        labels.insert("job".to_string(), "engram-logs".to_string());

        let entry = LokiEntry {
            timestamp_ns: timestamp,
            line: log_line,
            labels,
        };

        // If the background thread has exited, we silently drop the log.
        let _ = self.sink.sender.send(entry);
    }
}
