//! Optional Ferrum HTTP integration: DRS Crypt4GH **rewrap** (`X-Crypt4GH-Public-Key`) vs **decrypt_plain** (plaintext URL).

use anyhow::Result;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use common::config::TestConfig;
use common::http::HttpClient;
use common::report::{ComplianceLevel, TestCaseResult, TestCategory};
use crypt4gh::keys;
use crypt4gh::Keys;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use std::path::Path;
use tracing::info;

fn env_flag(name: &str) -> bool {
    matches!(
        std::env::var(name).map(|v| v == "1" || v.eq_ignore_ascii_case("true")),
        Ok(true)
    )
}

fn skip(
    name: &str,
    level: ComplianceLevel,
    category: TestCategory,
    msg: impl Into<String>,
) -> TestCaseResult {
    let msg = msg.into();
    info!(test = name, reason = %msg, "Crypt4GH Ferrum HTTP check skipped");
    TestCaseResult {
        name: name.to_string(),
        level,
        passed: true,
        error: Some(msg),
        category,
        weight: 1.0,
    }
}

fn fail(
    name: &str,
    level: ComplianceLevel,
    category: TestCategory,
    msg: impl Into<String>,
) -> TestCaseResult {
    TestCaseResult {
        name: name.to_string(),
        level,
        passed: false,
        error: Some(msg.into()),
        category,
        weight: 1.0,
    }
}

fn drs_object_url(cfg: &TestConfig, object_id: &str) -> String {
    format!(
        "{}/objects/{}",
        cfg.services.drs_url.trim_end_matches('/'),
        url_encode_object_id(object_id)
    )
}

fn url_encode_object_id(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    for b in id.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

fn encrypted_object_id() -> String {
    std::env::var("CRYPT4GH_ENCRYPTED_DRS_OBJECT_ID")
        .unwrap_or_else(|_| "test-object-1".to_string())
}

fn resolve_plain_download_url() -> Option<String> {
    if let Ok(u) = std::env::var("C4_PLAIN_DOWNLOAD_URL") {
        let t = u.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    let base = std::env::var("C4_PLAIN_URL_BASE").ok()?;
    let base = base.trim().trim_end_matches('/');
    if base.is_empty() {
        return None;
    }
    let path = std::env::var("C4_PLAIN_URL_PATH").unwrap_or_default();
    let path = path.trim();
    let combined = if path.is_empty() {
        base.to_string()
    } else if path.starts_with('/') {
        format!("{}{}", base, path)
    } else {
        format!("{}/{}", base, path)
    };
    Some(combined)
}

fn pick_drs_access_url(drs_object: &Value) -> Result<String> {
    let methods = drs_object
        .get("access_methods")
        .and_then(|x| x.as_array())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "DRS object JSON missing access_methods[] — cannot locate stream URL for Crypt4GH rewrap test"
            )
        })?;
    let mut fallback: Option<String> = None;
    for m in methods {
        let url = m
            .get("access_url")
            .and_then(|a| a.get("url"))
            .and_then(|u| u.as_str());
        let Some(url) = url else { continue };
        let typ = m.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if typ.to_lowercase().contains("crypt4gh") {
            return Ok(url.to_string());
        }
        if fallback.is_none() {
            fallback = Some(url.to_string());
        }
    }
    fallback.ok_or_else(|| {
        anyhow::anyhow!(
            "No access_methods[].access_url.url in DRS object — register a download URL for Crypt4GH rewrap"
        )
    })
}

fn drs_sha256_plaintext_checksum(drs_object: &Value) -> Option<String> {
    let checksums = drs_object.get("checksums")?.as_array()?;
    for c in checksums {
        let typ = c.get("type")?.as_str()?;
        if typ.eq_ignore_ascii_case("sha256") {
            return c.get("checksum")?.as_str().map(|s| s.to_string());
        }
    }
    None
}

fn sha256_bytes(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    format!("{:x}", h.finalize())
}

fn load_client_private_key() -> Result<Vec<u8>> {
    let path_str = std::env::var("CRYPT4GH_CLIENT_SECRET_KEY_PATH")
        .map_err(|_| anyhow::anyhow!("CRYPT4GH_CLIENT_SECRET_KEY_PATH is not set"))?;
    let path = Path::new(&path_str);
    if !path.is_file() {
        anyhow::bail!(
            "Crypt4GH client secret key file not found at {} — export CRYPT4GH_CLIENT_SECRET_KEY_PATH",
            path.display()
        );
    }
    let pass = std::env::var("CRYPT4GH_CLIENT_KEY_PASSPHRASE").unwrap_or_default();
    keys::get_private_key(path, || Ok(pass.clone()))
        .map_err(|e| anyhow::anyhow!("Crypt4GH load client key: {}", e))
}

fn public_key_b64_for_header(priv_blob: &[u8]) -> Result<String> {
    if let Ok(b64) = std::env::var("CRYPT4GH_CLIENT_PUBLIC_KEY_B64") {
        let t = b64.trim();
        if !t.is_empty() {
            return Ok(t.to_string());
        }
    }
    let pk = if priv_blob.len() >= 64 {
        priv_blob[32..64].to_vec()
    } else if priv_blob.len() >= 32 {
        keys::get_public_key_from_private_key(&priv_blob[..32])
            .map_err(|e| anyhow::anyhow!("derive public key from secret: {}", e))?
    } else {
        anyhow::bail!("client private key blob too short after load");
    };
    Ok(STANDARD.encode(pk))
}

fn decrypt_crypt4gh_body(ciphertext: &[u8], priv_blob: &[u8]) -> Result<Vec<u8>> {
    let keys_vec = vec![Keys {
        method: 0,
        privkey: priv_blob.to_vec(),
        recipient_pubkey: vec![],
    }];
    let mut input = Cursor::new(ciphertext);
    let mut plaintext = Vec::new();
    crypt4gh::decrypt(&keys_vec, &mut input, &mut plaintext, 0, None, &None)
        .map_err(|e| anyhow::anyhow!("Crypt4GH decrypt (rewrapped stream): {}", e))?;
    Ok(plaintext)
}

/// DRS download with `X-Crypt4GH-Public-Key`, local decrypt, optional checksum match.
pub async fn ferrum_crypt4gh_drs_rewrap(cfg: &TestConfig, client: &HttpClient) -> TestCaseResult {
    const NAME: &str = "Crypt4GH DRS rewrap download (X-Crypt4GH-Public-Key)";
    if !env_flag("HELIXTEST_FEATURE_CRYPT4GH_REWRAP") {
        return skip(
            NAME,
            ComplianceLevel::Level3,
            TestCategory::Interoperability,
            "skipped: set HELIXTEST_FEATURE_CRYPT4GH_REWRAP=1 and CRYPT4GH_CLIENT_SECRET_KEY_PATH to run Ferrum rewrap integration",
        );
    }

    let priv_blob = match load_client_private_key() {
        Ok(b) => b,
        Err(e) => {
            return fail(
                NAME,
                ComplianceLevel::Level3,
                TestCategory::Interoperability,
                format!(
                    "{} — Ferrum must return a Crypt4GH stream when the client sends X-Crypt4GH-Public-Key",
                    e
                ),
            );
        }
    };

    let object_id = encrypted_object_id();
    let drs_url = drs_object_url(cfg, &object_id);
    let drs_json = match client.get_json(&drs_url).await {
        Ok(v) => v,
        Err(e) => {
            return fail(
                NAME,
                ComplianceLevel::Level3,
                TestCategory::Interoperability,
                format!(
                    "DRS GET {} failed: {} — check CRYPT4GH_ENCRYPTED_DRS_OBJECT_ID and DRS_URL",
                    drs_url, e
                ),
            );
        }
    };

    let access_url = match pick_drs_access_url(&drs_json) {
        Ok(u) => u,
        Err(e) => {
            return fail(
                NAME,
                ComplianceLevel::Level3,
                TestCategory::Interoperability,
                e.to_string(),
            );
        }
    };

    let pub_b64 = match public_key_b64_for_header(&priv_blob) {
        Ok(b) => b,
        Err(e) => {
            return fail(
                NAME,
                ComplianceLevel::Level3,
                TestCategory::Interoperability,
                e.to_string(),
            );
        }
    };

    let resp = match client
        .inner()
        .get(&access_url)
        .header("X-Crypt4GH-Public-Key", pub_b64)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return fail(
                NAME,
                ComplianceLevel::Level3,
                TestCategory::Interoperability,
                format!(
                    "GET DRS access URL failed (with X-Crypt4GH-Public-Key): {} — URL={}",
                    e, access_url
                ),
            );
        }
    };

    if !resp.status().is_success() {
        return fail(
            NAME,
            ComplianceLevel::Level3,
            TestCategory::Interoperability,
            format!(
                "DRS stream returned HTTP {} for {} — expected 2xx Crypt4GH body when rewrap is enabled",
                resp.status(),
                access_url
            ),
        );
    }

    let ciphertext = match resp.bytes().await {
        Ok(b) => b.to_vec(),
        Err(e) => {
            return fail(
                NAME,
                ComplianceLevel::Level3,
                TestCategory::Interoperability,
                format!("reading rewrap response body: {}", e),
            );
        }
    };

    let plaintext = match decrypt_crypt4gh_body(&ciphertext, &priv_blob) {
        Ok(p) => p,
        Err(e) => {
            return fail(
                NAME,
                ComplianceLevel::Level3,
                TestCategory::Interoperability,
                format!(
                    "{} — response may be plaintext, wrong key, or not Crypt4GH (Ferrum Crypt4GHLayer rewrap)",
                    e
                ),
            );
        }
    };

    if let Some(expected) = drs_sha256_plaintext_checksum(&drs_json) {
        let actual = sha256_bytes(&plaintext);
        if !actual.eq_ignore_ascii_case(&expected) {
            return fail(
                NAME,
                ComplianceLevel::Level3,
                TestCategory::Checksum,
                format!(
                    "Decrypted plaintext SHA256 {} does not match DRS checksums sha256 {} — rewrap/plaintext mismatch or wrong object",
                    actual, expected
                ),
            );
        }
    }

    TestCaseResult {
        name: NAME.into(),
        level: ComplianceLevel::Level3,
        passed: true,
        error: drs_sha256_plaintext_checksum(&drs_json)
            .is_none()
            .then(|| "note: DRS object had no sha256 checksum to verify against".into()),
        category: TestCategory::Interoperability,
        weight: 1.0,
    }
}

/// Plain HTTP download vs rewrap-decrypt SHA256 (decrypt_plain mode on Ferrum).
pub async fn ferrum_crypt4gh_plain_matches_rewrap(
    cfg: &TestConfig,
    client: &HttpClient,
) -> TestCaseResult {
    const NAME: &str = "Crypt4GH plain download matches rewrap plaintext (decrypt_plain)";
    if !env_flag("HELIXTEST_FEATURE_CRYPT4GH_PLAIN") {
        return skip(
            NAME,
            ComplianceLevel::Level3,
            TestCategory::Checksum,
            "skipped: set HELIXTEST_FEATURE_CRYPT4GH_PLAIN=1 with C4_PLAIN_DOWNLOAD_URL or C4_PLAIN_URL_BASE (+ optional C4_PLAIN_URL_PATH) when Ferrum decrypt_plain is enabled",
        );
    }

    let plain_url = match resolve_plain_download_url() {
        Some(u) => u,
        None => {
            return fail(
                NAME,
                ComplianceLevel::Level3,
                TestCategory::Checksum,
                "HELIXTEST_FEATURE_CRYPT4GH_PLAIN=1 but neither C4_PLAIN_DOWNLOAD_URL nor C4_PLAIN_URL_BASE is set — provide a full GET URL for server-side-decrypted bytes",
            );
        }
    };

    if !env_flag("HELIXTEST_FEATURE_CRYPT4GH_REWRAP") {
        return fail(
            NAME,
            ComplianceLevel::Level3,
            TestCategory::Checksum,
            "HELIXTEST_FEATURE_CRYPT4GH_PLAIN requires HELIXTEST_FEATURE_CRYPT4GH_REWRAP=1 and the same client key material to compare hashes",
        );
    }

    let priv_blob = match load_client_private_key() {
        Ok(b) => b,
        Err(e) => {
            return fail(
                NAME,
                ComplianceLevel::Level3,
                TestCategory::Checksum,
                e.to_string(),
            );
        }
    };

    let object_id = encrypted_object_id();
    let drs_url = drs_object_url(cfg, &object_id);
    let drs_json = match client.get_json(&drs_url).await {
        Ok(v) => v,
        Err(e) => {
            return fail(
                NAME,
                ComplianceLevel::Level3,
                TestCategory::Checksum,
                format!("DRS GET {} failed: {}", drs_url, e),
            );
        }
    };

    let access_url = match pick_drs_access_url(&drs_json) {
        Ok(u) => u,
        Err(e) => {
            return fail(
                NAME,
                ComplianceLevel::Level3,
                TestCategory::Checksum,
                e.to_string(),
            );
        }
    };

    let pub_b64 = match public_key_b64_for_header(&priv_blob) {
        Ok(b) => b,
        Err(e) => {
            return fail(
                NAME,
                ComplianceLevel::Level3,
                TestCategory::Checksum,
                e.to_string(),
            );
        }
    };

    let rewrap_resp = match client
        .inner()
        .get(&access_url)
        .header("X-Crypt4GH-Public-Key", pub_b64)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return fail(
                NAME,
                ComplianceLevel::Level3,
                TestCategory::Checksum,
                format!("rewrap GET failed: {}", e),
            );
        }
    };
    if !rewrap_resp.status().is_success() {
        return fail(
            NAME,
            ComplianceLevel::Level3,
            TestCategory::Checksum,
            format!(
                "rewrap stream HTTP {} — cannot compare to plain download",
                rewrap_resp.status()
            ),
        );
    }
    let rewrap_ct = match rewrap_resp.bytes().await {
        Ok(b) => b.to_vec(),
        Err(e) => {
            return fail(
                NAME,
                ComplianceLevel::Level3,
                TestCategory::Checksum,
                format!("rewrap body: {}", e),
            );
        }
    };
    let rewrap_plain = match decrypt_crypt4gh_body(&rewrap_ct, &priv_blob) {
        Ok(p) => p,
        Err(e) => {
            return fail(
                NAME,
                ComplianceLevel::Level3,
                TestCategory::Checksum,
                format!("rewrap decrypt: {}", e),
            );
        }
    };
    let hash_rewrap = sha256_bytes(&rewrap_plain);

    let plain_resp = match client.inner().get(&plain_url).send().await {
        Ok(r) => r,
        Err(e) => {
            return fail(
                NAME,
                ComplianceLevel::Level3,
                TestCategory::Checksum,
                format!(
                    "plain GET {} failed: {} — check C4_PLAIN_* URL (Ferrum decrypt_plain / stream_decrypt path)",
                    plain_url, e
                ),
            );
        }
    };
    if !plain_resp.status().is_success() {
        return fail(
            NAME,
            ComplianceLevel::Level3,
            TestCategory::Checksum,
            format!(
                "plain download HTTP {} for {} — decrypt_plain endpoint missing or policy denied",
                plain_resp.status(),
                plain_url
            ),
        );
    }
    let plain_bytes = match plain_resp.bytes().await {
        Ok(b) => b.to_vec(),
        Err(e) => {
            return fail(
                NAME,
                ComplianceLevel::Level3,
                TestCategory::Checksum,
                format!("plain body: {}", e),
            );
        }
    };
    let hash_plain = sha256_bytes(&plain_bytes);

    if hash_plain != hash_rewrap {
        return fail(
            NAME,
            ComplianceLevel::Level3,
            TestCategory::Checksum,
            format!(
                "SHA256 mismatch: plain={} rewrap_decrypted={} — plain URL must return same logical object as DRS rewrap after local decrypt (check server decrypt_plain vs Crypt4GHLayer)",
                hash_plain, hash_rewrap
            ),
        );
    }

    TestCaseResult {
        name: NAME.into(),
        level: ComplianceLevel::Level3,
        passed: true,
        error: None,
        category: TestCategory::Checksum,
        weight: 1.0,
    }
}
