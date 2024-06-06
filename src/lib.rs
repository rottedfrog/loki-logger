mod api;
mod log_event;
mod loki_logger_builder;

use env_filter::Filter;
use log_event::LokiLogEvent;
use reqwest::Url;
use serde::Serialize;
use std::{collections::HashMap, thread};
use tokio::sync::mpsc::UnboundedSender;

use log::{LevelFilter, Metadata, Record};

/// Re-export of the log crate for use with a different version by the `loki-logger` crate's user.
pub use log;
pub use loki_logger_builder::LokiLoggerBuilder;

pub fn builder() -> LokiLoggerBuilder {
    LokiLoggerBuilder::default()
}

#[derive(Serialize)]
struct LokiStream {
    stream: HashMap<String, String>,
    values: Vec<[String; 2]>,
}

#[derive(Serialize)]
struct LokiRequest {
    streams: Vec<LokiStream>,
}

pub struct LokiLogger {
    default_filter: LevelFilter,
    labels: HashMap<String, String>,
    filter: Filter,
    send: UnboundedSender<LokiLogEvent>,
}

impl log::Log for LokiLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.filter.enabled(metadata)
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let _ = self.send.send(record.into());
        }
    }

    fn flush(&self) {}
}

impl LokiLogger {
    fn new(url: Url, labels: HashMap<String, String>, filter: Filter) -> Self {
        let (send, mut recv) = tokio::sync::mpsc::unbounded_channel();
        let _ = thread::spawn(move || loki_executor(recv, url));

        let default_filter = std::env::var("RUST_LOG")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(LevelFilter::Off);
        Self {
            labels,
            filter,
            send,
            default_filter,
        }
    }

    pub fn filter(&self) -> LevelFilter {
        self.default_filter
    }
}

fn loki_executor(mut recv: tokio::sync::mpsc::UnboundedReceiver<LokiLogEvent>, url: Url) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .unwrap();
    rt.block_on(async move {
        let client = reqwest::Client::new();
        while let Some(event) = recv.recv().await {
            if let Err(e) = client
                .post(url.clone())
                .json(&LokiRequest::from(event))
                .send()
                .await
                .and_then(|res| res.error_for_status())
            {
                eprintln!("{:?}", e);
            } else {
                println!("Ok");
            };
        }
    })
}
