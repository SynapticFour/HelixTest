// SPDX-License-Identifier: Apache-2.0
//! Deterministic DRS checker source digest. Same algorithm as `build.rs`.
//! Not a git SHA. Not HELIOS.

use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;

/// Manifest version. Changing the prefix changes every checker identity.
pub const MANIFEST_VERSION: &str = "helix-drs-checker-v2";

/// Closure list, relative to this crate (`checker_source_v2.txt` next to `src/`).
pub const SOURCE_LIST: &str = include_str!("../checker_source_v2.txt");

pub fn listed_paths() -> Vec<&'static str> {
    parse_listed_paths(SOURCE_LIST)
}

pub fn parse_listed_paths(list: &str) -> Vec<&str> {
    list.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect()
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

/// Canonical manifest then SHA-256. `list_rel` is hashed first so changing the
/// closure list changes identity even if every listed file is unchanged.
pub fn digest_from_parts(list_rel: &str, list_bytes: &[u8], files: &[(&str, &[u8])]) -> String {
    let mut buf = format!(
        "{MANIFEST_VERSION}\nfile={list_rel}\nsha256={}\n",
        sha256_hex(list_bytes)
    );
    for (rel, bytes) in files {
        buf.push_str(&format!("file={rel}\nsha256={}\n", sha256_hex(bytes)));
    }
    sha256_hex(buf.as_bytes())
}

/// Recompute from a helixtest/ root. Tests use this; production uses `env!`.
pub fn digest_from_helixtest_root(root: &Path) -> String {
    let list_rel = "crates/framework/checker_source_v2.txt";
    let list_path = root.join(list_rel);
    let list_bytes =
        std::fs::read(&list_path).unwrap_or_else(|e| panic!("read {}: {e}", list_path.display()));
    let list_text = String::from_utf8(list_bytes.clone()).expect("checker_source_v2.txt utf-8");
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    for rel in parse_listed_paths(&list_text) {
        let path = root.join(rel);
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        files.push((rel.to_string(), bytes));
    }
    let refs: Vec<(&str, &[u8])> = files
        .iter()
        .map(|(r, b)| (r.as_str(), b.as_slice()))
        .collect();
    digest_from_parts(list_rel, &list_bytes, &refs)
}

/// Same digest with one listed path's bytes replaced. No production backdoor.
pub fn digest_with_override(root: &Path, rel: &str, replacement: &[u8]) -> String {
    let list_rel = "crates/framework/checker_source_v2.txt";
    let list_bytes = std::fs::read(root.join(list_rel)).expect("list");
    let list_text = String::from_utf8(list_bytes.clone()).expect("utf-8");
    let mut map: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for p in parse_listed_paths(&list_text) {
        map.insert(p.to_string(), std::fs::read(root.join(p)).expect("file"));
    }
    assert!(
        map.contains_key(rel),
        "{rel} is not in the DRS checker source closure"
    );
    map.insert(rel.to_string(), replacement.to_vec());
    let files: Vec<(String, Vec<u8>)> = parse_listed_paths(&list_text)
        .into_iter()
        .map(|p| (p.to_string(), map.get(p).unwrap().clone()))
        .collect();
    let refs: Vec<(&str, &[u8])> = files
        .iter()
        .map(|(r, b)| (r.as_str(), b.as_slice()))
        .collect();
    digest_from_parts(list_rel, &list_bytes, &refs)
}

pub fn helixtest_root_from_framework_manifest() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drs::executed_checker_source_sha256;

    #[test]
    fn t7_digest_is_deterministic_and_matches_embedded() {
        let root = helixtest_root_from_framework_manifest();
        let a = digest_from_helixtest_root(&root);
        let b = digest_from_helixtest_root(&root);
        assert_eq!(a, b);
        assert_eq!(a, executed_checker_source_sha256());
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn t1_changing_drs_rs_changes_identity() {
        let root = helixtest_root_from_framework_manifest();
        let base = digest_from_helixtest_root(&root);
        let mut bytes = std::fs::read(root.join("crates/framework/src/drs.rs")).unwrap();
        bytes.extend_from_slice(b"\n// identity-probe\n");
        let changed = digest_with_override(&root, "crates/framework/src/drs.rs", &bytes);
        assert_ne!(base, changed);
    }

    #[test]
    fn t2_changing_schema_helper_changes_identity() {
        let root = helixtest_root_from_framework_manifest();
        let base = digest_from_helixtest_root(&root);
        let mut bytes = std::fs::read(root.join("crates/common/src/spec_source.rs")).unwrap();
        bytes.extend_from_slice(b"\n// identity-probe\n");
        let changed = digest_with_override(&root, "crates/common/src/spec_source.rs", &bytes);
        assert_ne!(base, changed);
        let mut g = std::fs::read(root.join("crates/common/src/ga4gh_schemas.rs")).unwrap();
        g.extend_from_slice(b"\n// identity-probe\n");
        let g_changed = digest_with_override(&root, "crates/common/src/ga4gh_schemas.rs", &g);
        assert_ne!(base, g_changed);
    }

    #[test]
    fn t3_changing_bundled_drs_openapi_changes_identity() {
        let root = helixtest_root_from_framework_manifest();
        let base = digest_from_helixtest_root(&root);
        let mut bytes = std::fs::read(root.join("schemas/ga4gh/drs-openapi.yaml")).unwrap();
        bytes.extend_from_slice(b"\n# identity-probe\n");
        let changed = digest_with_override(&root, "schemas/ga4gh/drs-openapi.yaml", &bytes);
        assert_ne!(base, changed);
    }

    #[test]
    fn t4_changing_http_client_changes_identity() {
        let root = helixtest_root_from_framework_manifest();
        let base = digest_from_helixtest_root(&root);
        let mut bytes = std::fs::read(root.join("crates/common/src/http.rs")).unwrap();
        bytes.extend_from_slice(b"\n// identity-probe\n");
        let changed = digest_with_override(&root, "crates/common/src/http.rs", &bytes);
        assert_ne!(base, changed);
    }

    #[test]
    fn t5_changing_sha256_impl_changes_identity() {
        let root = helixtest_root_from_framework_manifest();
        let base = digest_from_helixtest_root(&root);
        let mut bytes = std::fs::read(root.join("crates/common/src/util.rs")).unwrap();
        bytes.extend_from_slice(b"\n// identity-probe\n");
        let changed = digest_with_override(&root, "crates/common/src/util.rs", &bytes);
        assert_ne!(base, changed);
        assert!(
            std::fs::read_to_string(root.join("crates/common/src/util.rs"))
                .unwrap()
                .contains("pub fn sha256_bytes"),
            "closure must hash the file that defines sha256_bytes"
        );
    }

    #[test]
    fn t6_readme_is_not_in_the_closure() {
        let paths = listed_paths();
        assert!(!paths.iter().any(|p| p.contains("README")));
        assert!(!paths.iter().any(|p| p.ends_with(".md")));
        let root = helixtest_root_from_framework_manifest();
        let base = digest_from_helixtest_root(&root);
        // README is not a listed path; override API must refuse it.
        let err = std::panic::catch_unwind(|| {
            digest_with_override(&root, "README.md", b"unrelated");
        });
        assert!(err.is_err());
        assert_eq!(base, digest_from_helixtest_root(&root));
    }
}
