use log::kv::Source;

use log::kv::VisitSource;

use log::Record;

use std::collections::HashMap;

use std::time::SystemTime;

use log::Level;

pub struct LokiLogEvent {
    pub level: Level,
    pub timestamp: SystemTime,
    pub structured_metadata: HashMap<String, String>,
    pub content: String,
}

impl<'a> From<&Record<'a>> for LokiLogEvent {
    fn from(record: &Record) -> Self {
        LokiLogEvent {
            level: record.level(),
            timestamp: SystemTime::now(),
            structured_metadata: collect(record.key_values()),
            content: format!("{:?}", record.args()),
        }
    }
}

pub(crate) struct KvCollector(HashMap<String, String>);

impl KvCollector {
    pub(crate) fn with_capacity(count: usize) -> Self {
        Self(HashMap::with_capacity(count))
    }
}

impl VisitSource<'_> for KvCollector {
    fn visit_pair(
        &mut self,
        key: log::kv::Key,
        value: log::kv::Value,
    ) -> Result<(), log::kv::Error> {
        self.0.insert(key.to_string(), value.to_string());
        Ok(())
    }
}

pub(crate) fn collect(src: &dyn Source) -> HashMap<String, String> {
    let mut c = KvCollector::with_capacity(src.count());
    let _ = src.visit(&mut c);
    c.0
}
