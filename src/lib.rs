//mod api;
mod log_event;
mod loki_logger_builder;

use core::fmt;
use env_filter::Filter;
use log_event::LokiLogEvent;
use reqwest::Url;
use serde::Serialize;
use std::{
    collections::BTreeMap,
    sync::RwLock,
    thread::{self, JoinHandle},
    time::UNIX_EPOCH,
};
use tokio::sync::mpsc::UnboundedSender;

use log::{LevelFilter, Metadata, Record};

/// Re-export of the log crate for use with a different version by the `loki-logger` crate's user.
pub use log;
pub use loki_logger_builder::{builder, LokiLoggerBuilder};

#[derive(Serialize)]
struct LokiStream {
    stream: BTreeMap<String, String>,
    values: [[String; 2]; 1],
}

#[derive(Serialize)]
struct LokiRequest {
    streams: [LokiStream; 1],
}

impl LokiRequest {
    fn new(event: LokiLogEvent, labels: &BTreeMap<String, String>) -> Self {
        let mut stream = labels.clone();
        stream.insert("level".to_owned(), event.level.to_string());
        Self {
            streams: [LokiStream {
                stream,
                values: [event.into()],
            }],
        }
    }
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
    fn new_with_closer(
        url: Url,
        labels: BTreeMap<String, String>,
        filter: Filter,
    ) -> (Self, LokiCloser) {
        let (send, recv) = tokio::sync::mpsc::unbounded_channel();
        let join_handle = thread::spawn(move || loki_executor(recv, labels, url));

        (
            Self {
                filter,
                send: send.clone(),
            },
            LokiCloser {
                join_handle: RwLock::new(Some(join_handle)),
                send,
            },
        )
    }

    pub fn filter(&self) -> LevelFilter {
        self.filter.filter()
    }
}

/// Wraps a join handle for the loki execution thread, allowing synchronization with the logger during a shutdown.
/// It is designed to be callable multiple times, so it can be called from, for example, a panic hook, signal handler and the main function so that
/// logging is correctly completed on abort or on program exit.

pub struct LokiCloser {
    join_handle: RwLock<Option<JoinHandle<()>>>,
    send: UnboundedSender<Option<LokiLogEvent>>,
}

impl fmt::Debug for LokiCloser {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LokiCloser{...}")
    }
}

impl LokiCloser {
    /// Shuts down the associated loki executor, blocking until all messages have been sent. Can be called multiple times safely.
    pub fn shutdown(&self) {
        let Ok(_) = self.send.send(None) else { return };
        let Some(join_handle) = self.join_handle.write().unwrap().take() else {
            return;
        };
        if let Err(e) = join_handle.join() {
            eprintln!("Error closing loki logger: {:?}", e);
        }
    }
}

fn loki_executor(
    mut recv: tokio::sync::mpsc::UnboundedReceiver<Option<LokiLogEvent>>,
    labels: BTreeMap<String, String>,
    url: Url,
) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .unwrap();
    rt.block_on(async move {
        let client = reqwest::Client::new();
        while let Some(Some(event)) = recv.recv().await {
            if let Err(e) = client
                .post(url.clone())
                .json(&LokiRequest::new(event, &labels))
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
