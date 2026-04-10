use anyhow::{anyhow, Result};
use serde::Deserialize;
use crate::models::JarVersion;
use std::time::Duration;

use crate::constants::USER_AGENT;

const BASE: &str = "https://api.leafmc.one/v2/projects/leaf";

fn is_safe_filename(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
}

#[derive(Deserialize)]
struct BuildsResponse {
    builds: Vec<LeafBuild>,
}

#[derive(Deserialize)]
struct LeafBuild {
    build: u32,
    channel: String,
    downloads: LeafDownloads,
}

#[derive(Deserialize)]
struct LeafDownloads {
    primary: LeafDownloadEntry,
}

#[derive(Deserialize)]
struct LeafDownloadEntry {
    name: String,
}

#[tracing::instrument(skip(client))]
pub async fn get_builds_for_version(client: &reqwest::Client, version: &str) -> Result<Vec<JarVersion>> {
    let url = format!("{BASE}/versions/{version}/builds");
    let resp: BuildsResponse = client
        .get(&url)
        .header("User-Agent", USER_AGENT)
        .timeout(Duration::from_secs(3))
        .send()
        .await?
        .json()
        .await?;

    if resp.builds.is_empty() {
        return Err(anyhow!("No Leaf builds for version {version}"));
    }

    let builds: Vec<JarVersion> = resp.builds
        .into_iter()
        .rev()
        .filter_map(|b| {
            if !is_safe_filename(&b.downloads.primary.name) {
                return None;
            }
            let download_url = format!(
                "{BASE}/versions/{version}/builds/{}/downloads/{}",
                b.build, b.downloads.primary.name
            );
            Some(JarVersion {
                version: version.to_string(),
                build: b.build.to_string(),
                channel: b.channel.to_lowercase(),
                download_url,
            })
        })
        .collect();

    Ok(builds)
}
