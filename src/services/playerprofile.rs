use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::time::Duration;

use crate::constants::USER_AGENT;

#[derive(Debug, Serialize, Clone)]
pub struct PlayerProfile {
    pub uuid: String,
    pub uuid_formatted: String,
    pub username: String,
    pub skin_url: Option<String>,
    pub cape_url: Option<String>,
    pub skin_model: String,
    pub avatar_url: String,
    pub body_url: String,
}

#[derive(Deserialize)]
struct MojangIdResponse {
    id: String,
    #[allow(dead_code)]
    name: String,
}

#[derive(Deserialize)]
struct SessionProfile {
    #[allow(dead_code)]
    id: String,
    name: String,
    properties: Vec<ProfileProperty>,
}

#[derive(Deserialize)]
struct ProfileProperty {
    name: String,
    value: String,
}

#[derive(Deserialize)]
struct TexturesPayload {
    textures: TexturesMap,
}

#[derive(Deserialize)]
struct TexturesMap {
    #[serde(rename = "SKIN")]
    skin: Option<TextureEntry>,
    #[serde(rename = "CAPE")]
    cape: Option<TextureEntry>,
}

#[derive(Deserialize)]
struct TextureEntry {
    url: String,
    metadata: Option<SkinMetadata>,
}

#[derive(Deserialize)]
struct SkinMetadata {
    model: Option<String>,
}

fn format_uuid(raw: &str) -> String {
    if raw.len() != 32 {
        return raw.to_string();
    }
    format!(
        "{}-{}-{}-{}-{}",
        &raw[0..8],
        &raw[8..12],
        &raw[12..16],
        &raw[16..20],
        &raw[20..32]
    )
}

#[tracing::instrument(skip(client))]
pub async fn get_profile(client: &reqwest::Client, username: &str) -> Result<PlayerProfile> {
    let id_url = format!("https://api.mojang.com/users/profiles/minecraft/{username}");
    let id_resp = client
        .get(&id_url)
        .header("User-Agent", USER_AGENT)
        .timeout(Duration::from_secs(5))
        .send()
        .await?;

    if id_resp.status() == 404 {
        return Err(anyhow!("Player '{}' not found", username));
    }

    let id_data: MojangIdResponse = id_resp.json().await?;
    let uuid = &id_data.id;

    let session_url = format!(
        "https://sessionserver.mojang.com/session/minecraft/profile/{uuid}"
    );
    let session: SessionProfile = client
        .get(&session_url)
        .header("User-Agent", USER_AGENT)
        .timeout(Duration::from_secs(5))
        .send()
        .await?
        .json()
        .await?;

    let textures_prop = session
        .properties
        .iter()
        .find(|p| p.name == "textures");

    let (skin_url, cape_url, skin_model) = if let Some(prop) = textures_prop {
        let decoded = STANDARD.decode(&prop.value)?;
        let payload: TexturesPayload = serde_json::from_slice(&decoded)?;

        let skin = payload.textures.skin.as_ref().map(|s| s.url.clone());
        let cape = payload.textures.cape.as_ref().map(|c| c.url.clone());
        let model = payload.textures.skin
            .as_ref()
            .and_then(|s| s.metadata.as_ref())
            .and_then(|m| m.model.clone())
            .unwrap_or_else(|| "classic".to_string());

        (skin, cape, model)
    } else {
        (None, None, "classic".to_string())
    };

    let avatar_url = format!("https://crafatar.com/avatars/{uuid}?size=128&overlay");
    let body_url = format!("https://crafatar.com/renders/body/{uuid}?size=256&overlay");

    Ok(PlayerProfile {
        uuid_formatted: format_uuid(uuid),
        uuid: uuid.clone(),
        username: session.name,
        skin_url,
        cape_url,
        skin_model,
        avatar_url,
        body_url,
    })
}
