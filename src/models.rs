use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JarVersion {
    pub version: String,
    pub build: String,
    pub channel: String,
    pub download_url: String,
}

#[derive(Debug, Deserialize)]
pub struct PaperBuild {
    pub id: u32,
    pub channel: String,
    pub downloads: HashMap<String, PaperDownloadEntry>,
}

#[derive(Debug, Deserialize)]
pub struct PaperDownloadEntry {
    pub url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PurpurResponse {
    pub project: String,
    pub version: String,
    pub builds: PurpurBuilds,
}

#[derive(Debug, Deserialize)]
pub struct PurpurBuilds {
    pub all: Vec<String>,
}


