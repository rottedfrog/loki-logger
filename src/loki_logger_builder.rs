// let logger = env_logger::builder()
// .filter_module("rustls", LevelFilter::Info)
// .filter_module("ureq", LevelFilter::Info)
// .filter_module("hyper", LevelFilter::Info)
// .filter_module("reqwest", LevelFilter::Info)
// .filter_module("yup_oauth2", LevelFilter::Info)
// .filter_module("h2", LevelFilter::Info)
// .filter_module("rusoto_core", LevelFilter::Info)
// .filter_module("aws_", LevelFilter::Warn)
// .filter_module("tracing", LevelFilter::Warn)
// .format(|f, record| {
//     let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.6f%z");
//     writeln!(
//         f,
//         "{} {} {} - {}",
//         timestamp,
//         record.metadata().level(),
//         record.metadata().target(),
//         record.args()
//     )
// })
// .build();

use std::collections::HashMap;

use log::LevelFilter;
use reqwest::IntoUrl;

use crate::LokiLogger;

#[derive(Default)]
pub struct LokiLoggerBuilder {
    filters: HashMap<String, LevelFilter>,
    labels: HashMap<String, String>,
}

impl LokiLoggerBuilder {
    pub fn filter_module(mut self, module: &str, level_filter: LevelFilter) -> Self {
        self.filters.insert(module.to_owned(), level_filter);
        self
    }

    pub fn label(mut self, name: &str, value: &str) -> Self {
        self.labels.insert(name.to_owned(), value.to_owned());
        self
    }

    pub fn build(self, url: impl IntoUrl) -> Result<LokiLogger, reqwest::Error> {
        Ok(LokiLogger::new(url.into_url()?, self.labels, self.filters))
    }
}
