use anyhow::Result;
use serde::Deserialize;
use std::ffi::c_int;
use std::time::Duration;

use crate::constants::USER_AGENT;
use crate::ffi;

const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Deserialize)]
struct Manifest {
    versions: Vec<ManifestVersion>,
}

#[derive(Deserialize)]
struct ManifestVersion {
    id: String,
    #[serde(rename = "type")]
    version_type: String,
}

#[tracing::instrument(skip(client))]
pub async fn get_release_versions(client: &reqwest::Client) -> Result<Vec<String>> {
    let manifest: Manifest = client
        .get(MANIFEST_URL)
        .header("User-Agent", USER_AGENT)
        .timeout(Duration::from_secs(8))
        .send()
        .await?
        .json()
        .await?;

    let versions = manifest.versions
        .into_iter()
        .filter(|v| v.version_type == "release")
        .map(|v| v.id)
        .collect();

    Ok(versions)
}

#[tracing::instrument(skip(client))]
pub async fn get_supported_versions(client: &reqwest::Client) -> Result<Vec<String>> {
    let all = get_release_versions(client).await?;
    let filtered = all
        .into_iter()
        .filter(|v| version_to_mc_const(v).is_some())
        .collect();
    Ok(filtered)
}

#[inline]
pub fn version_to_mc_const(version: &str) -> Option<c_int> {
    let major = major_of(version);
    match major {
        "1.21" => Some(ffi::MC_1_21),
        "1.20" => Some(ffi::MC_1_20),
        "1.19" => Some(ffi::MC_1_19),
        "1.18" => Some(ffi::MC_1_18),
        "1.17" => Some(ffi::MC_1_17),
        "1.16" => Some(ffi::MC_1_16),
        _ => None,
    }
}

#[inline(always)]
fn major_of(version: &str) -> &str {
    let mut dots = 0;
    for (i, c) in version.char_indices() {
        if c == '.' {
            dots += 1;
            if dots == 2 {
                return &version[..i];
            }
        }
    }
    version
}
