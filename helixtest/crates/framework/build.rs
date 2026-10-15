// SPDX-License-Identifier: Apache-2.0
//! Embed SHA-256 of the DRS checker source closure this crate compiles.
//! Same algorithm as `src/checker_identity.rs`. Not a git tag. Not VERSIONS.lock.

use sha2::{Digest, Sha256};
use std::env;
use std::path::Path;

const MANIFEST_VERSION: &str = "helix-drs-checker-v2";
const LIST_REL: &str = "crates/framework/checker_source_v2.txt";

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

fn parse_listed_paths(list: &str) -> Vec<&str> {
    list.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect()
}

fn checker_source_sha256(helixtest_root: &Path) -> String {
    let list_path = helixtest_root.join(LIST_REL);
    println!("cargo:rerun-if-changed={}", list_path.display());
    let list_bytes =
        std::fs::read(&list_path).unwrap_or_else(|e| panic!("read {}: {e}", list_path.display()));
    let list_text = String::from_utf8(list_bytes.clone()).expect("checker_source_v2.txt utf-8");
    let mut buf = format!(
        "{MANIFEST_VERSION}\nfile={LIST_REL}\nsha256={}\n",
        sha256_hex(&list_bytes)
    );
    for rel in parse_listed_paths(&list_text) {
        let path = helixtest_root.join(rel);
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        buf.push_str(&format!("file={rel}\nsha256={}\n", sha256_hex(&bytes)));
        println!("cargo:rerun-if-changed={}", path.display());
    }
    sha256_hex(buf.as_bytes())
}

fn main() {
    let manifest = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).to_path_buf();
    let helixtest_root = manifest.join("../..");
    let digest = checker_source_sha256(&helixtest_root);
    println!("cargo:rustc-env=HELIXTEST_DRS_CHECKER_SOURCE_SHA256={digest}");
}
