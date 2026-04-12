use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::time::timeout;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

const TOTAL_TIMEOUT: Duration = Duration::from_millis(2500);
const MAX_JSON_SIZE: usize = 64 * 1024;
const MAX_FAVICON_SIZE: usize = 32 * 1024;

#[derive(Debug, Serialize)]
pub struct ServerStatus {
    pub online: bool,
    pub host: String,
    pub port: u16,
    pub version: Option<String>,
    pub protocol: Option<i32>,
    pub software: Option<String>,
    pub description: Option<String>,
    pub players_online: Option<u32>,
    pub players_max: Option<u32>,
    pub players: Vec<String>,
    pub favicon: Option<String>,
    pub latency_ms: u64,
}

#[derive(Deserialize)]
struct McStatusResponse {
    version: Option<McVersion>,
    players: Option<McPlayers>,
    description: Option<serde_json::Value>,
    favicon: Option<String>,
}

#[derive(Deserialize)]
struct McVersion {
    name: String,
    protocol: i32,
}

#[derive(Deserialize)]
struct McPlayers {
    max: u32,
    online: u32,
    sample: Option<Vec<McPlayer>>,
}

#[derive(Deserialize)]
struct McPlayer {
    name: String,
}

fn write_varint(buf: &mut Vec<u8>, mut value: i32) {
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 { byte |= 0x80; }
        buf.push(byte);
        if value == 0 { break; }
    }
}

async fn write_varint_async<W: tokio::io::AsyncWrite + Unpin>(writer: &mut W, mut value: i32) -> Result<()> {
    let mut buf = [0u8; 5];
    let mut pos = 0;
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 { byte |= 0x80; }
        buf[pos] = byte;
        pos += 1;
        if value == 0 { break; }
    }
    writer.write_all(&buf[..pos]).await?;
    Ok(())
}

async fn read_varint_async<R: tokio::io::AsyncRead + Unpin>(reader: &mut R) -> Result<i32> {
    let mut result = 0;
    let mut shift = 0;
    loop {
        let b = reader.read_u8().await?;
        result |= ((b & 0x7F) as i32) << shift;
        if b & 0x80 == 0 { break; }
        shift += 7;
        if shift >= 35 { return Err(anyhow!("VarInt too long")); }
    }
    Ok(result)
}

fn write_string(buf: &mut Vec<u8>, s: &str) {
    write_varint(buf, s.len() as i32);
    buf.extend_from_slice(s.as_bytes());
}


// § kodlarını koruyarak description'ı düz string'e çevirir
fn extract_description_raw(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Object(obj) => {
            let mut res = String::new();

            // color field: named renk veya #RRGGBB hex
            if let Some(color) = obj.get("color").and_then(|v| v.as_str()) {
                if color.starts_with('#') {
                    // hex renk — §#RRGGBB formatında gönder, CLI parse eder
                    res.push_str(&format!("§{}", color));
                } else if let Some(code) = color_name_to_code(color) {
                    res.push_str(&format!("§{}", code));
                }
            }

            if obj.get("bold").and_then(|v| v.as_bool()).unwrap_or(false)       { res.push_str("§l"); }
            if obj.get("italic").and_then(|v| v.as_bool()).unwrap_or(false)     { res.push_str("§o"); }
            if obj.get("underlined").and_then(|v| v.as_bool()).unwrap_or(false) { res.push_str("§n"); }
            if obj.get("strikethrough").and_then(|v| v.as_bool()).unwrap_or(false) { res.push_str("§m"); }

            if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
                res.push_str(text);
            }
            if let Some(extra) = obj.get("extra").and_then(|v| v.as_array()) {
                for part in extra {
                    res.push_str("§r");
                    res.push_str(&extract_description_raw(part));
                }
            }
            res
        }
        _ => String::new(),
    }
}

fn color_name_to_code(name: &str) -> Option<char> {
    match name {
        "black"        => Some('0'),
        "dark_blue"    => Some('1'),
        "dark_green"   => Some('2'),
        "dark_aqua"    => Some('3'),
        "dark_red"     => Some('4'),
        "dark_purple"  => Some('5'),
        "gold"         => Some('6'),
        "gray"         => Some('7'),
        "dark_gray"    => Some('8'),
        "blue"         => Some('9'),
        "green"        => Some('a'),
        "aqua"         => Some('b'),
        "red"          => Some('c'),
        "light_purple" => Some('d'),
        "yellow"       => Some('e'),
        "white"        => Some('f'),
        _              => None,
    }
}

fn extract_description(val: &serde_json::Value) -> String {
    extract_description_raw(val)
}

fn extract_software(version_name: &str) -> Option<String> {
    let known = ["Paper", "Leaf", "Purpur", "Spigot", "CraftBukkit", "Folia",
                 "Velocity", "Waterfall", "BungeeCord", "Fabric", "Forge"];
    known.iter().find(|&&sw| version_name.contains(sw)).map(|&sw| sw.to_string())
}

fn build_status(host: &str, port: u16, resp: McStatusResponse, latency: u64) -> ServerStatus {
    let v_name = resp.version.as_ref().map(|v| v.name.clone());
    let players: Vec<String> = resp.players.as_ref()
        .and_then(|p| p.sample.as_ref())
        .map(|s| s.iter().map(|p| p.name.clone()).collect())
        .unwrap_or_default();

    let favicon = resp.favicon.filter(|f| f.len() <= MAX_FAVICON_SIZE);

    ServerStatus {
        online: true,
        host: host.to_string(),
        port,
        version: v_name.clone(),
        protocol: resp.version.as_ref().map(|v| v.protocol),
        software: v_name.as_deref().and_then(extract_software),
        description: resp.description.as_ref().map(extract_description),
        players_online: resp.players.as_ref().map(|p| p.online),
        players_max: resp.players.as_ref().map(|p| p.max),
        players,
        favicon,
        latency_ms: latency,
    }
}

fn offline_status(host: &str, port: u16, elapsed_ms: u64) -> ServerStatus {
    ServerStatus {
        online: false, host: host.to_string(), port,
        version: None, protocol: None, software: None, description: None,
        players_online: None, players_max: None, players: vec![],
        favicon: None, latency_ms: elapsed_ms,
    }
}

pub async fn ping_addr(host: &str, addr: SocketAddr) -> ServerStatus {
    let start = Instant::now();
    let port = addr.port();
    match timeout(TOTAL_TIMEOUT, ping_process(host, addr)).await {
        Ok(Ok((resp, latency))) => build_status(host, port, resp, latency),
        _ => offline_status(host, port, start.elapsed().as_millis() as u64),
    }
}

async fn ping_process(host: &str, addr: SocketAddr) -> Result<(McStatusResponse, u64)> {
    let start = Instant::now();
    let stream = TcpStream::connect(addr).await?;
    let latency = start.elapsed().as_millis() as u64;

    let mut reader = BufReader::with_capacity(8192, stream);

    let mut handshake = Vec::with_capacity(64);
    write_varint(&mut handshake, 0x00);
    write_varint(&mut handshake, 767);
    write_string(&mut handshake, host);
    handshake.extend_from_slice(&addr.port().to_be_bytes());
    write_varint(&mut handshake, 1);

    write_varint_async(&mut reader, handshake.len() as i32).await?;
    reader.write_all(&handshake).await?;

    reader.write_all(&[1, 0x00]).await?;
    reader.flush().await?;

    let _packet_len = read_varint_async(&mut reader).await?;
    let _packet_id  = read_varint_async(&mut reader).await?;
    let json_len    = read_varint_async(&mut reader).await? as usize;

    if json_len > MAX_JSON_SIZE { return Err(anyhow!("Response too large")); }

    let mut json_buf = vec![0u8; json_len];
    reader.read_exact(&mut json_buf).await?;

    let resp: McStatusResponse = serde_json::from_slice(&json_buf)?;
    Ok((resp, latency))
}
