use anyhow::{Result, anyhow};
use base64::{Engine as _, engine::general_purpose};

pub fn parse_proxy_auth_token(token: &[u8]) -> Result<(String, String)> {
    let token_str = std::str::from_utf8(token)?;

    let encoded_cred = token_str
        .strip_prefix("Basic ")
        .ok_or_else(|| anyhow!("Invalid auth format: expected 'Basic ...'"))?;

    let decoded = general_purpose::STANDARD.decode(encoded_cred)?;
    let credentials = String::from_utf8(decoded)?;

    credentials
        .split_once(':')
        .map(|(u, p)| (u.to_string(), p.to_string()))
        .ok_or_else(|| anyhow!("Invalid credentials format: expected 'user:password'"))
}

