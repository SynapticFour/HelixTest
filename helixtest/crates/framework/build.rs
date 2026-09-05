// SPDX-License-Identifier: Apache-2.0
//! Embed SHA-256 of the DRS checker sources this crate compiles.
//! Not a git tag. Not VERSIONS.lock. Helix compares this to the lock at build/runtime.

use sha2::{Digest, Sha256};
use std::env;
use std::path::Path;

const FILES: &[&str] = &[
    "crates/framework/src/drs.rs",
    "crates/common/src/ga4gh_schemas.rs",
    "crates/common/src/spec_source.rs",
];

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

fn checker_source_sha256(helixtest_root: &Path) -> String {
    let mut buf = String::from("helix-drs-checker-v1\n");
    for rel in FILES {
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
