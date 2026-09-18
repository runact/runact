use crate::headers::Headers;
use crate::response::{Response, StatusCode};
use base64::Engine as _;
use std::error::Error;
use std::fmt;

const WEBSOCKET_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

#[derive(Debug)]
pub enum UpgradeError {
    MissingHeader(String),
    InvalidHeader(String),
}

impl fmt::Display for UpgradeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UpgradeError::MissingHeader(h) => write!(f, "missing header: {h}"),
            UpgradeError::InvalidHeader(h) => write!(f, "invalid header: {h}"),
        }
    }
}

impl Error for UpgradeError {}

pub fn validate_upgrade_request(headers: &Headers) -> Result<(), UpgradeError> {
    let upgrade = headers
        .get("Upgrade")
        .ok_or_else(|| UpgradeError::MissingHeader("Upgrade".into()))?;
    if upgrade.to_lowercase() != "websocket" {
        return Err(UpgradeError::InvalidHeader(format!("Upgrade: {upgrade}")));
    }

    let connection = headers
        .get("Connection")
        .ok_or_else(|| UpgradeError::MissingHeader("Connection".into()))?;
    if !connection.to_lowercase().contains("upgrade") {
        return Err(UpgradeError::InvalidHeader(format!(
            "Connection: {connection}"
        )));
    }

    headers
        .get("Sec-WebSocket-Key")
        .ok_or_else(|| UpgradeError::MissingHeader("Sec-WebSocket-Key".into()))?;

    let version = headers
        .get("Sec-WebSocket-Version")
        .ok_or_else(|| UpgradeError::MissingHeader("Sec-WebSocket-Version".into()))?;
    if version != "13" {
        return Err(UpgradeError::InvalidHeader(format!(
            "Sec-WebSocket-Version: {version}"
        )));
    }

    Ok(())
}

pub fn build_accept_key(client_key: &str) -> String {
    use sha1::Digest;
    let mut hasher = sha1::Sha1::new();
    hasher.update(client_key.as_bytes());
    hasher.update(WEBSOCKET_GUID.as_bytes());
    let result = hasher.finalize();
    base64::engine::general_purpose::STANDARD.encode(result)
}

pub fn build_upgrade_response(headers: &Headers) -> Result<Response, UpgradeError> {
    let key = headers
        .get("Sec-WebSocket-Key")
        .ok_or_else(|| UpgradeError::MissingHeader("Sec-WebSocket-Key".into()))?;

    let accept = build_accept_key(key);

    let mut resp_headers = Headers::new();
    resp_headers.insert("Upgrade", "websocket");
    resp_headers.insert("Connection", "Upgrade");
    resp_headers.insert("Sec-WebSocket-Accept", &accept);

    Ok(Response {
        status: StatusCode::SwitchingProtocols,
        reason: "Switching Protocols".to_string(),
        version: "HTTP/1.1".to_string(),
        headers: resp_headers,
        body: vec![],
    })
}
