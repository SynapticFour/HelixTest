use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use chrono::{Duration, Utc};
use serde::Serialize;
use hmac::{Hmac, Mac};
use sha2::Sha256;

#[derive(Debug, Clone, Serialize)]
struct Claims {
    iss: String,
    sub: String,
    aud: String,
    exp: i64,
    iat: i64,
    scope: String,
}

fn base64url_encode(data: &[u8]) -> String {
    general_purpose::URL_SAFE_NO_PAD.encode(data)
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

pub fn build_jwt(
    issuer: &str,
    subject: &str,
    audience: &str,
    scope: &str,
    lifetime: Duration,
    secret: &str,
) -> Result<String> {
    let header = serde_json::json!({
        "alg": "HS256",
        "typ": "JWT"
    });
    let now = Utc::now();
    let claims = Claims {
        iss: issuer.to_owned(),
        sub: subject.to_owned(),
        aud: audience.to_owned(),
        exp: (now + lifetime).timestamp(),
        iat: now.timestamp(),
        scope: scope.to_owned(),
    };
    let header_b64 = base64url_encode(&serde_json::to_vec(&header)?);
    let claims_b64 = base64url_encode(&serde_json::to_vec(&claims)?);
    let signing_input = format!("{}.{}", header_b64, claims_b64);
    let sig = hmac_sha256(secret.as_bytes(), signing_input.as_bytes());
    let sig_b64 = base64url_encode(&sig);
    Ok(format!("{}.{}", signing_input, sig_b64))
}

