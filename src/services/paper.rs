use anyhow::{anyhow, Result};
use crate::models::{JarVersion, PaperBuild};
use std::time::Duration;

use crate::constants::USER_AGENT;

const BASE: &str = "https://fill.papermc.io/v3/projects/paper";

fn is_safe_paper_url(url: &str) -> bool {
    url.starts_with("https://fill.papermc.io/")
        || url.starts_with("https://fill-data.papermc.io/")
        || url.starts_with("https://api.papermc.io/")
}

pub async fn get_builds(client: &reqwest::Client, project: &str, version: &str) -> Result<Vec<JarVersion>> {
    let url = format!("https://fill.papermc.io/v3/projects/{}/versions/{}/builds", project, version);
    let builds: Vec<PaperBuild> = client
        .get(&url)
        .header("User-Agent", USER_AGENT)
        .timeout(Duration::from_millis(5000))
        .send()
        .await?
        .json()
        .await?;

    let result: Vec<JarVersion> = builds
        .into_iter()
        .filter_map(|b| {
            let download = b.downloads.get("server:default")
                .or_else(|| b.downloads.get("application"))?;
            let download_url = download.url.clone()?;

            if !is_safe_paper_url(&download_url) {
                return None;
            }

            Some(JarVersion {
                version: version.to_string(),
                build: b.id.to_string(),
                channel: b.channel.to_lowercase(),
                download_url,
            })
        })
        .collect();

    if result.is_empty() {
        return Err(anyhow!("No builds found for version {version}"));
    }

    Ok(result)
}
