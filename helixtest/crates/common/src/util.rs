// SPDX-License-Identifier: Apache-2.0
use anyhow::Result;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub fn sha256_file<P: AsRef<Path>>(path: P) -> Result<String> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let hash = hasher.finalize();
    Ok(format!("{:x}", hash))
}

pub fn sha256_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// SHA-256 of `path` only if it exists and `mtime >= not_before` (this run).
/// Refuses stale fixtures that would otherwise false-green checksum tests.
pub fn sha256_file_if_fresh(path: impl AsRef<Path>, not_before: SystemTime) -> Result<String> {
    let path = path.as_ref();
    if !path.exists() {
        anyhow::bail!("missing file {}", path.display());
    }
    let mtime = std::fs::metadata(path)?.modified()?;
    if mtime < not_before {
        anyhow::bail!(
            "local file {} is older than this run (mtime before submit); refusing stale fixture",
            path.display()
        );
    }
    sha256_file(path)
}

/// RFC 3986 unreserved path-segment encoding.
pub fn percent_encode_path_segment(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// Directory containing `profiles/` and `test-data/` (the `helixtest/` tree).
/// `CARGO_MANIFEST_DIR` is `helixtest/crates/<crate>`.
pub fn helixtest_root() -> Result<PathBuf> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "cannot resolve helixtest root from CARGO_MANIFEST_DIR={}",
                manifest.display()
            )
        })
}

pub fn profiles_dir() -> Result<PathBuf> {
    Ok(helixtest_root()?.join("profiles"))
}

pub fn test_data_dir() -> Result<PathBuf> {
    Ok(helixtest_root()?.join("test-data"))
}

/// Level 0: the spec path answered. 2xx or 401 (auth required). 404/405 fail.
pub fn level0_reachable_ok(status: reqwest::StatusCode) -> bool {
    status.is_success() || status.as_u16() == 401
}
