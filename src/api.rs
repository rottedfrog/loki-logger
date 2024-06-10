use std::collections::{BTreeMap, HashMap};

use log::Level;
use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub struct PushRequest {
    #[prost(message, repeated, tag = "1")]
    pub streams: Vec<StreamAdapter>,
}

#[derive(Clone, PartialEq, Message)]
pub struct PushResponse {}

#[derive(Clone, PartialEq, Message)]
pub struct StreamAdapter {
    #[prost(string, tag = "1")]
    pub labels: String,
    #[prost(message, repeated, tag = "2")]
    pub entries: Vec<EntryAdapter>,
    /// hash contains the original hash of the stream.
    #[prost(uint64, tag = "3")]
    pub hash: u64,
}

#[derive(Clone, PartialEq, Message)]
pub struct LabelPairAdapter {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(string, tag = "2")]
    pub value: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct EntryAdapter {
    #[prost(message, optional, tag = "1")]
    pub timestamp: Option<prost_types::Timestamp>,
    #[prost(string, tag = "2")]
    pub line: String,
    #[prost(message, repeated, tag = "3")]
    pub structured_metadata: Vec<LabelPairAdapter>,
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
