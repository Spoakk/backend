use anyhow::{anyhow, Result};
use crate::models::{JarVersion, PurpurResponse};
use std::time::Duration;

use crate::constants::USER_AGENT;

const BASE: &str = "https://api.purpurmc.org/v2/purpur";

#[tracing::instrument(skip(client))]
pub async fn get_builds(client: &reqwest::Client, version: &str) -> Result<Vec<JarVersion>> {
    let url = format!("{}/{}", BASE, version);
    
    let res = client
        .get(&url)
        .header("User-Agent", USER_AGENT)
        .timeout(Duration::from_millis(5000))
        .send()
        .await?;
        
    if !res.status().is_success() {
        return Err(anyhow!("Purpur API error: {}", res.status()));
    }
    
    let data: PurpurResponse = res.json().await?;

    let mut result: Vec<JarVersion> = data.builds.all
        .into_iter()
        .map(|b| {
            let download_url = format!("{}/{}/{}/download", BASE, version, b);
            JarVersion {
                version: version.to_string(),
                build: b,
                channel: "stable".to_string(), // Purpur API doesn't distinguish channel directly via 'all'
                download_url,
            }
        })
        .collect();
        
    // Reverse to show latest first
    result.reverse();

    if result.is_empty() {
        return Err(anyhow!("No builds found for version {version}"));
    }

    Ok(result)
}
