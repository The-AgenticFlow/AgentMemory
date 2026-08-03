use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::Level;
use tracing_subscriber::{EnvFilter, Layer, layer::SubscriberExt, util::SubscriberInitExt};

pub struct LokiSink {
    url: String,
    labels: std::collections::HashMap<String, String>,
    client: reqwest::Client,
}

impl LokiSink {
    pub fn new(url: String) -> Self {
        let mut labels = std::collections::HashMap::new();
        labels.insert("service".to_string(), "engram".to_string());
        labels.insert("job".to_string(), "engram-logs".to_string());
        Self {
            url,
            labels,
            client: reqwest::Client::new(),
        }
    }
}

#[derive(Serialize)]
struct LokiStreamRequest {
    streams: Vec<LokiStream>,
}

#[derive(Serialize)]
struct LokiStream {
    stream: std::collections::HashMap<String, String>,
    values: Vec<String>,
}

pub fn init_loki_sink(loki_url: String) {
    if loki_url.is_empty() {
        return;
    }

    let sink = LokiSink::new(loki_url);
    let sink = LokiLayer { sink };

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let _ = tracing_subscriber::registry()
        .with(env_filter)
        .with(sink)
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
            Level::TRACE => "trace",
            Level::DEBUG => "debug",
            Level::INFO => "info",
            Level::WARN => "warn",
            Level::ERROR => "error",
        };

        let mut message = String::new();
        let mut fields = std::collections::HashMap::new();

        struct Visitor<'a> {
            message: &'a mut String,
            fields: &'a mut std::collections::HashMap<String, String>,
        }

        impl<'a> tracing::field::Visit for Visitor<'a> {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                if field.name() == "message" || field.name() == "message" {
                    *self.message = format!("{:?}", value);
                } else {
                    self.fields
                        .insert(field.name().to_string(), format!("{:?}", value));
                }
            }
            fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                if field.name() == "message" {
                    *self.message = value.to_string();
                } else {
                    self.fields
                        .insert(field.name().to_string(), value.to_string());
                }
            }
        }

        let mut visitor = Visitor {
            message: &mut message,
            fields: &mut fields,
        };
        event.record(&mut visitor);

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .to_string();

        let target = metadata.target();
        let log_line = if message.is_empty() {
            format!(
                "{} {} {}",
                chrono::Utc::now().to_rfc3339(),
                level.to_uppercase(),
                target
            )
        } else {
            format!(
                "{} {} {}: {}",
                chrono::Utc::now().to_rfc3339(),
                level.to_uppercase(),
                target,
                message
            )
        };

        let url = format!("{}/loki/api/v1/push", self.sink.url.trim_end_matches('/'));

        let mut stream = self.sink.labels.clone();
        stream.insert("level".to_string(), level.to_string());

        let request = LokiStreamRequest {
            streams: vec![LokiStream {
                stream,
                values: vec![format!("{} {}", timestamp, log_line)],
            }],
        };

        let client = self.sink.client.clone();
        tokio::spawn(async move {
            if let Err(e) = client.post(&url).json(&request).send().await {
                eprintln!("Loki send failed: {}", e);
            }
        });
    }
}
