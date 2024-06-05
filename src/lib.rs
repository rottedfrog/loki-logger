mod loki_logger_builder;

use reqwest::Url;
use serde::Serialize;
use std::{
    collections::HashMap,
    error::Error,
    thread,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::mpsc::UnboundedSender;

use log::{kv::Visitor, Level, LevelFilter, Metadata, Record};

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
    filters: HashMap<String, LevelFilter>,
    send: UnboundedSender<LokiRequest>,
}

struct LokiVisitor<'a> {
    values: HashMap<log::kv::Key<'a>, log::kv::Value<'a>>,
}

impl<'a> LokiVisitor<'a> {
    pub fn with_capacity(count: usize) -> Self {
        Self {
            values: HashMap::with_capacity(count),
        }
    }

    pub fn from_record(record: &'a Record) -> Self {
        let kv = record.key_values();
        let mut me = LokiVisitor::with_capacity(kv.count());
        for _ in 0..kv.count() {
            let _ = kv.visit(&mut me);
        }
        me
    }
}

impl<'a> Visitor<'a> for LokiVisitor<'a> {
    fn visit_pair(
        &mut self,
        key: log::kv::Key<'a>,
        value: log::kv::Value<'a>,
    ) -> Result<(), log::kv::Error> {
        self.values.insert(key, value);
        Ok(())
    }
}

impl log::Log for LokiLogger {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            if let Err(e) = self.log_event_record(record) {
                eprintln!("Unable to send event to loki: {:?}", e)
            } else {
                //eprintln!("Sent event to loki! Woo {record:?}")
            }
        }
    }

    fn flush(&self) {}
}

impl LokiLogger {
    fn new(
        url: Url,
        labels: HashMap<String, String>,
        filters: HashMap<String, LevelFilter>,
    ) -> Self {
        let (send, mut recv) = tokio::sync::mpsc::unbounded_channel();
        let _ = thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_io()
                .enable_time()
                .build()
                .unwrap();
            rt.block_on(async move {
                let client = reqwest::Client::new();
                while let Some(loki_request) = recv.recv().await {
                    if let Err(e) = client
                        .post(url.clone())
                        .json(&loki_request)
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
        });

        let default_filter = std::env::var("RUST_LOG")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(LevelFilter::Off);
        Self {
            labels,
            filters,
            send,
            default_filter,
        }
    }

    pub fn filter(&self) -> LevelFilter {
        self.default_filter
    }

    fn log_event_record(&self, record: &Record) -> Result<(), Box<dyn Error>> {
        let kv_labels = LokiVisitor::from_record(record).values;
        let message = format!("{:?}", record.args());
        let mut labels = self.labels.clone();
        /*if !self.filter_record(record.metadata().level(), record.metadata().target()) {
            return Ok(());
        }*/
        labels.extend(
            kv_labels
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string())),
        );
        labels.insert(
            "level".to_string(),
            record.level().to_string().to_ascii_lowercase(),
        );
        let loki_request = make_request(message, labels)?;
        let _ = self.send.send(loki_request);
        Ok(())
    }

    fn filter_record(&self, level: Level, target: &str) -> bool {
        let target_level = self
            .filters
            .get(target)
            .copied()
            .unwrap_or(self.default_filter);
        level >= target_level
    }
}

fn make_request(
    message: String,
    labels: HashMap<String, String>,
) -> Result<LokiRequest, Box<dyn Error>> {
    let start = SystemTime::now();
    let time_ns = time_offset_since(start)?;
    let loki_request = LokiRequest {
        streams: vec![LokiStream {
            stream: labels,
            values: vec![[time_ns, message]],
        }],
    };
    Ok(loki_request)
}

fn time_offset_since(start: SystemTime) -> Result<String, Box<dyn Error>> {
    let since_start = start.duration_since(UNIX_EPOCH)?;
    let time_ns = since_start.as_nanos().to_string();
    Ok(time_ns)
}

#[cfg(test)]
mod tests {
    use crate::time_offset_since;
    use std::time::{Duration, SystemTime};

    #[test]
    fn time_offsets() {
        let t1 = time_offset_since(SystemTime::now());
        assert!(t1.is_ok());

        // Constructing a negative timestamp
        let negative_time = SystemTime::UNIX_EPOCH.checked_sub(Duration::from_secs(1));

        assert!(negative_time.is_some());

        let t2 = time_offset_since(negative_time.unwrap());
        assert!(t2.is_err());
    }
}
