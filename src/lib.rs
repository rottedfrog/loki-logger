mod api;
mod log_event;
mod loki_logger_builder;

use api::{EntryAdapter, LabelPairAdapter, StreamAdapter};
use env_filter::Filter;
use log_event::LokiLogEvent;
use prost::Message;
use reqwest::Url;
use serde::Serialize;
use std::{
    collections::{BTreeMap, HashMap},
    thread::{self, JoinHandle},
    time::UNIX_EPOCH,
};
use tokio::sync::mpsc::UnboundedSender;

use log::{Level, LevelFilter, Metadata, Record};

/// Re-export of the log crate for use with a different version by the `loki-logger` crate's user.
pub use log;
pub use loki_logger_builder::{builder, LokiLoggerBuilder};

#[derive(Serialize)]
struct LokiStream {
    stream: HashMap<String, String>,
    values: Vec<[String; 2]>,
}

#[derive(Serialize)]
struct LokiRequest {
    streams: Vec<LokiStream>,
}

impl From<LokiLogEvent> for [String; 2] {
    fn from(value: LokiLogEvent) -> Self {
        [
            value
                .timestamp
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
                .to_string(),
            value.content,
        ]
    }
}

pub struct LokiLogger {
    filter: Filter,
    send: UnboundedSender<Option<LokiLogEvent>>,
}

impl log::Log for LokiLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.filter.enabled(metadata)
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let _ = self.send.send(Some(record.into()));
        }
    }

    fn flush(&self) {}
}

impl LokiLogger {
    fn new_with_handle(
        url: Url,
        labels: BTreeMap<String, String>,
        filter: Filter,
    ) -> (Self, JoinHandle<()>) {
        let (send, recv) = tokio::sync::mpsc::unbounded_channel();
        let handle = thread::spawn(move || loki_executor(recv, labels, url));

        (Self { filter, send }, handle)
    }

    fn new(url: Url, labels: BTreeMap<String, String>, filter: Filter) -> Self {
        Self::new_with_handle(url, labels, filter).0
    }

    pub fn filter(&self) -> LevelFilter {
        self.filter.filter()
    }
}

/// Wraps a join handle for the loki execution thread, allowing synchronization with the logger during a shutdown.
/// It is designed to be callable multiple times, so it can be called from, for example, a panic hook, signal handler and the main function so that
/// logging is correctly completed on abort or on program exit.

pub struct LokiCloser {
    join_handle: Option<JoinHandle<()>>,
    send: UnboundedSender<Option<LokiLogEvent>>,
}

impl LokiCloser {
    /// Shuts down the associated loki executor, blocking until all messages have been sent. Can be called multiple times safely.
    pub fn shutdown(&mut self) {
        let Ok(_) = self.send.send(None) else { return };
        let Some(join_handle) = self.join_handle.take() else {
            return;
        };
        if let Err(e) = join_handle.join() {
            eprintln!("Error closing loki logger: {:?}", e);
        }
    }
}

fn build_labels(labels: BTreeMap<String, String>) -> String {
    let mut s = "{".to_owned();
    for (name, value) in labels {
        s.push_str(&name);
        s.push('=');
        s.push('"');
        s.push_str(&value.replace('"', "\""));
        s.push('"');
        s.push(',')
    }
    if let Some('{') = s.pop() {
        s.push('{')
    };
    s.push('}');
    s
}

fn init_labels(labels: BTreeMap<String, String>) -> HashMap<Level, String> {
    let mut level_labels = HashMap::new();
    for level in [
        Level::Error,
        Level::Warn,
        Level::Info,
        Level::Debug,
        Level::Trace,
    ] {
        let mut labels = labels.clone();
        labels.insert("level".to_owned(), level.to_string());
        level_labels.insert(level, build_labels(labels));
    }
    level_labels
}

fn loki_executor(
    mut recv: tokio::sync::mpsc::UnboundedReceiver<Option<LokiLogEvent>>,
    labels: BTreeMap<String, String>,
    url: Url,
) {
    let labels = init_labels(labels);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .unwrap();
    rt.block_on(async move {
        let client = reqwest::Client::new();
        let mut req = api::PushRequest {
            streams: Vec::new(),
        };

        while let Some(Some(event)) = recv.recv().await {
            let mut req = api::PushRequest {
                streams: vec![StreamAdapter {
                    labels: labels[&event.level].clone(),
                    entries: vec![EntryAdapter {
                        timestamp: Some(event.timestamp.into()),
                        line: ,
                        structured_metadata: event
                            .structured_metadata
                            .into_iter()
                            .map(|(name, value)| LabelPairAdapter { name, value })
                            .collect(),
                    }],
                    hash: 0,
                }],
            };

            let w = snap::write::FrameEncoder::new(Vec::new());
            w.
            if let Err(e) = client
                .post(url.clone())
                .body(req.encode(&mut w))
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
