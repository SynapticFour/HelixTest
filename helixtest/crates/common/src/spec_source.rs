// SPDX-License-Identifier: Apache-2.0
//! Explicit specification bytes for schema compilation.
//!
//! The versioned Helix path must pass a [`SpecSource`]. This module never
//! reads HelixTest's bundled `include_str!` OpenAPI. Relative `$ref`s resolve
//! only from `files`. HTTP, `file://`, absolute paths, and `..` fail closed.
//! Partial `$ref` resolution is not accepted.

use anyhow::{bail, Context, Result};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use jsonschema::JSONSchema;
use once_cell::sync::Lazy;

/// Call counters so Helix can prove the versioned path never uses bundled schema.
static BUNDLED_DRS_VALIDATE_CALLS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
static SPEC_COMPILE_CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Incremented by [`crate::ga4gh_schemas::validate_drs_object`] (bundled OpenAPI).
pub fn record_bundled_drs_validate() {
    BUNDLED_DRS_VALIDATE_CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

pub fn bundled_drs_validate_calls() -> usize {
    BUNDLED_DRS_VALIDATE_CALLS.load(std::sync::atomic::Ordering::SeqCst)
}

pub fn spec_compile_calls() -> usize {
    SPEC_COMPILE_CALLS.load(std::sync::atomic::Ordering::SeqCst)
}

pub fn reset_schema_call_counters() {
    BUNDLED_DRS_VALIDATE_CALLS.store(0, std::sync::atomic::Ordering::SeqCst);
    SPEC_COMPILE_CALLS.store(0, std::sync::atomic::Ordering::SeqCst);
}

/// SHA-256 of raw bytes, lowercase hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let d = Sha256::digest(bytes);
    d.iter().map(|b| format!("{b:02x}")).collect()
}

/// Canonical pack / document digest: sorted `"{file_sha256}  {path}\\n"` then SHA-256.
pub fn sha256_manifest_v1<'a, I>(files: I) -> String
where
    I: IntoIterator<Item = (&'a str, &'a [u8])>,
{
    let mut entries: Vec<(&str, &[u8])> = files.into_iter().collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));
    let mut manifest = String::new();
    for (path, bytes) in entries {
        manifest.push_str(&sha256_hex(bytes));
        manifest.push_str("  ");
        manifest.push_str(path);
        manifest.push('\n');
    }
    sha256_hex(manifest.as_bytes())
}

/// Caller-supplied specification bytes. No bundled fallback.
#[derive(Debug, Clone)]
pub struct SpecSource {
    pub schema_entry: String,
    pub schema_component: String,
    pub files: BTreeMap<String, Arc<[u8]>>,
}

/// Identity of a compiled schema from a [`SpecSource`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecCompileResult {
    pub schema_document_sha256: String,
    pub schema_component_sha256: String,
    pub files_opened: Vec<String>,
}

impl SpecSource {
    pub fn get(&self, path: &str) -> Result<&[u8]> {
        self.files
            .get(path)
            .map(|b| b.as_ref())
            .with_context(|| format!("SpecSource missing file {path}"))
    }
}

/// Cache keyed by schema_document_sha256, never by component name alone.
static SCHEMA_CACHE: Lazy<Mutex<BTreeMap<String, &'static JSONSchema>>> =
    Lazy::new(|| Mutex::new(BTreeMap::new()));

/// Resolve SpecSource to a JSON Schema value and identity (no jsonschema compile).
pub fn resolve_schema_value(spec: &SpecSource) -> Result<(Value, SpecCompileResult)> {
    validate_spec_paths(spec)?;
    if spec.schema_component.is_empty() {
        bail!("schema_component must be non-empty");
    }
    let entry = normalize_posix(&spec.schema_entry)?;
    if !spec.files.contains_key(&entry) {
        bail!("schema_entry {entry} is not in SpecSource.files");
    }

    let mut opened = BTreeSet::new();
    collect_closure(spec, &entry, &mut opened, &mut BTreeSet::new())?;

    let files_opened: Vec<String> = opened.iter().cloned().collect();
    let opened_bytes: Vec<(&str, &[u8])> = files_opened
        .iter()
        .map(|p| {
            let b = spec.files.get(p).expect("opened file is in map");
            (p.as_str(), b.as_ref())
        })
        .collect();
    let schema_document_sha256 = sha256_manifest_v1(opened_bytes);

    let schema_value = build_defs_document(spec, &entry, &opened)?;
    let schema_component_sha256 = sha256_hex(&serde_json::to_vec(&schema_value)?);

    Ok((
        schema_value,
        SpecCompileResult {
            schema_document_sha256,
            schema_component_sha256,
            files_opened,
        },
    ))
}

/// Resolve and compile the SpecSource (no instance document). Used before HTTP checks.
pub fn compile_identity(spec: &SpecSource) -> Result<SpecCompileResult> {
    SPEC_COMPILE_CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let (schema_value, identity) = resolve_schema_value(spec)?;
    cached_or_compile(&identity.schema_document_sha256, schema_value)?;
    Ok(identity)
}

/// Validate `value` against the SpecSource. Compiles from supplied bytes only.
pub fn validate_with_spec(spec: &SpecSource, value: &Value) -> Result<SpecCompileResult> {
    let identity = compile_identity(spec)?;
    let compiled = {
        let cache = SCHEMA_CACHE.lock().expect("schema cache lock");
        *cache
            .get(&identity.schema_document_sha256)
            .expect("compile_identity populated cache")
    };
    validate_against(compiled, value, &spec.schema_component)?;
    Ok(identity)
}

fn cached_or_compile(doc_hash: &str, schema_value: Value) -> Result<&'static JSONSchema> {
    {
        let cache = SCHEMA_CACHE.lock().expect("schema cache lock");
        if let Some(s) = cache.get(doc_hash) {
            return Ok(*s);
        }
    }
    let leaked: &'static Value = Box::leak(Box::new(schema_value));
    let compiled = JSONSchema::compile(leaked).context("compile SpecSource schema")?;
    let leaked_schema: &'static JSONSchema = Box::leak(Box::new(compiled));
    SCHEMA_CACHE
        .lock()
        .expect("schema cache lock")
        .insert(doc_hash.to_string(), leaked_schema);
    Ok(leaked_schema)
}

fn validate_against(schema: &JSONSchema, value: &Value, label: &str) -> Result<()> {
    schema.validate(value).map_err(|errors| {
        let msgs: Vec<String> = errors
            .map(|e| format!("{} at {}", e, e.instance_path))
            .collect();
        anyhow::anyhow!(
            "JSON did not validate against SpecSource {label}: {}",
            msgs.join("; ")
        )
    })?;
    Ok(())
}

fn validate_spec_paths(spec: &SpecSource) -> Result<()> {
    for path in spec.files.keys() {
        normalize_posix(path)?;
    }
    normalize_posix(&spec.schema_entry)?;
    Ok(())
}

fn normalize_posix(path: &str) -> Result<String> {
    if path.is_empty() {
        bail!("path must be non-empty");
    }
    if path.starts_with('/') || path.contains('\\') {
        bail!("SpecSource path must be relative POSIX (got {path})");
    }
    if path.contains('\0') {
        bail!("SpecSource path contains NUL");
    }
    let mut out: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => bail!("SpecSource path must not contain '..' (got {path})"),
            s => out.push(s),
        }
    }
    if out.is_empty() {
        bail!("SpecSource path is empty after normalize (got {path})");
    }
    Ok(out.join("/"))
}

fn collect_closure(
    spec: &SpecSource,
    path: &str,
    opened: &mut BTreeSet<String>,
    visiting: &mut BTreeSet<String>,
) -> Result<()> {
    let path = normalize_posix(path)?;
    if opened.contains(&path) {
        return Ok(());
    }
    if visiting.contains(&path) {
        opened.insert(path);
        return Ok(());
    }
    let bytes = spec.get(&path)?;
    visiting.insert(path.clone());
    opened.insert(path.clone());
    let value: Value =
        serde_yaml::from_slice(bytes).with_context(|| format!("parse YAML {path}"))?;
    walk_refs(&value, &path, spec, opened, visiting)?;
    visiting.remove(&path);
    Ok(())
}

fn walk_refs(
    value: &Value,
    current_file: &str,
    spec: &SpecSource,
    opened: &mut BTreeSet<String>,
    visiting: &mut BTreeSet<String>,
) -> Result<()> {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(r)) = map.get("$ref") {
                follow_ref(r, current_file, spec, opened, visiting)?;
            }
            for (k, v) in map {
                if k == "$ref" {
                    continue;
                }
                walk_refs(v, current_file, spec, opened, visiting)?;
            }
        }
        Value::Array(arr) => {
            for v in arr {
                walk_refs(v, current_file, spec, opened, visiting)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn follow_ref(
    r: &str,
    current_file: &str,
    spec: &SpecSource,
    opened: &mut BTreeSet<String>,
    visiting: &mut BTreeSet<String>,
) -> Result<()> {
    reject_forbidden_ref(r)?;
    let (file_part, _pointer) = split_ref(r);
    if file_part.is_empty() {
        return Ok(());
    }
    let target = join_posix(current_file, file_part)?;
    if !spec.files.contains_key(&target) {
        bail!("$ref {r} from {current_file} resolves to missing {target}");
    }
    collect_closure(spec, &target, opened, visiting)
}

fn reject_forbidden_ref(r: &str) -> Result<()> {
    let lower = r.trim().to_ascii_lowercase();
    if lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("file://")
        || lower.contains("://")
    {
        bail!("SpecSource $ref must be local relative (got {r})");
    }
    if r.starts_with('/') {
        bail!("SpecSource $ref must not be absolute (got {r})");
    }
    Ok(())
}

fn split_ref(r: &str) -> (&str, &str) {
    match r.split_once('#') {
        Some(("", ptr)) => ("", ptr),
        Some((file, ptr)) => (file, ptr),
        None => (r, ""),
    }
}

fn join_posix(current_file: &str, rel: &str) -> Result<String> {
    let rel = rel.trim();
    if rel.is_empty() {
        return Ok(current_file.to_string());
    }
    reject_forbidden_ref(rel)?;
    let parent = match current_file.rfind('/') {
        Some(i) => &current_file[..i],
        None => "",
    };
    let combined = if parent.is_empty() {
        rel.to_string()
    } else if let Some(stripped) = rel.strip_prefix("./") {
        format!("{parent}/{stripped}")
    } else {
        format!("{parent}/{rel}")
    };
    normalize_posix(&combined)
}

fn pointer_escape(s: &str) -> String {
    s.replace('~', "~0").replace('/', "~1")
}

fn rewrite_refs_in_place(value: &mut Value, current_file: &str) -> Result<()> {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(r)) = map.get("$ref").cloned() {
                reject_forbidden_ref(&r)?;
                let (file_part, pointer) = split_ref(&r);
                let target_file = if file_part.is_empty() {
                    current_file.to_string()
                } else {
                    join_posix(current_file, file_part)?
                };
                let def_ptr = format!("#/$defs/{}", pointer_escape(&target_file));
                let new_ref = if pointer.is_empty() {
                    def_ptr
                } else if pointer.starts_with('/') {
                    format!("{def_ptr}{pointer}")
                } else {
                    format!("{def_ptr}/{pointer}")
                };
                map.insert("$ref".into(), Value::String(new_ref));
                return Ok(());
            }
            for (k, v) in map.iter_mut() {
                if k == "$ref" {
                    continue;
                }
                rewrite_refs_in_place(v, current_file)?;
            }
        }
        Value::Array(arr) => {
            for v in arr {
                rewrite_refs_in_place(v, current_file)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn build_defs_document(spec: &SpecSource, entry: &str, opened: &BTreeSet<String>) -> Result<Value> {
    let mut defs = Map::new();
    for path in opened {
        let bytes = spec.get(path)?;
        let mut parsed: Value =
            serde_yaml::from_slice(bytes).with_context(|| format!("parse YAML {path}"))?;
        rewrite_refs_in_place(&mut parsed, path)?;
        defs.insert(path.clone(), parsed);
    }
    let entry_ref = format!("#/$defs/{}", pointer_escape(entry));
    Ok(json!({
        "$defs": defs,
        "allOf": [{ "$ref": entry_ref }]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file_map(pairs: &[(&str, &str)]) -> BTreeMap<String, Arc<[u8]>> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), Arc::<[u8]>::from(v.as_bytes())))
            .collect()
    }

    fn minimal_drs_files(extra_required: &[&str]) -> BTreeMap<String, Arc<[u8]>> {
        let mut required = vec!["id", "self_uri", "size", "created_time", "checksums"];
        required.extend_from_slice(extra_required);
        let req_yaml = required
            .iter()
            .map(|s| format!("  - {s}"))
            .collect::<Vec<_>>()
            .join("\n");
        file_map(&[
            (
                "openapi/components/schemas/DrsObject.yaml",
                &format!(
                    "type: object\nrequired:\n{req_yaml}\nproperties:\n  id:\n    type: string\n  self_uri:\n    type: string\n  size:\n    type: integer\n  created_time:\n    type: string\n  checksums:\n    type: array\n    items:\n      $ref: './Checksum.yaml'\n  access_methods:\n    type: array\n    items:\n      $ref: './AccessMethod.yaml'\n"
                ),
            ),
            (
                "openapi/components/schemas/Checksum.yaml",
                "type: object\nrequired: [checksum, type]\nproperties:\n  checksum:\n    type: string\n  type:\n    type: string\n",
            ),
            (
                "openapi/components/schemas/AccessMethod.yaml",
                "type: object\nrequired: [type]\nproperties:\n  type:\n    type: string\n  access_url:\n    $ref: './AccessURL.yaml'\n  authorizations:\n    $ref: './Authorizations.yaml'\n",
            ),
            (
                "openapi/components/schemas/AccessURL.yaml",
                "type: object\nproperties:\n  url:\n    type: string\n",
            ),
            (
                "openapi/components/schemas/Authorizations.yaml",
                "type: object\nproperties:\n  supported_types:\n    type: array\n    items:\n      type: string\n",
            ),
            (
                "openapi/components/schemas/ContentsObject.yaml",
                "type: object\nrequired: [name]\nproperties:\n  name:\n    type: string\n  contents:\n    type: array\n    items:\n      $ref: './ContentsObject.yaml'\n",
            ),
        ])
    }

    fn spec_from(files: BTreeMap<String, Arc<[u8]>>) -> SpecSource {
        SpecSource {
            schema_entry: "openapi/components/schemas/DrsObject.yaml".into(),
            schema_component: "DrsObject".into(),
            files,
        }
    }

    fn ok_payload() -> Value {
        json!({
            "id": "test-object-1",
            "self_uri": "drs://example.org/test-object-1",
            "size": 12,
            "created_time": "2026-01-01T00:00:00Z",
            "checksums": [{ "type": "sha256", "checksum": "abc" }]
        })
    }

    #[test]
    fn manifest_is_sorted_and_stable() {
        let a = sha256_manifest_v1([("b.yaml", b"x" as &[u8]), ("a.yaml", b"y")]);
        let b = sha256_manifest_v1([("a.yaml", b"y" as &[u8]), ("b.yaml", b"x")]);
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn relative_closure_compiles_and_accepts_payload() {
        let spec = spec_from(minimal_drs_files(&[]));
        validate_with_spec(&spec, &ok_payload()).expect("minimal DrsObject");
    }

    #[test]
    fn extra_required_field_rejects_payload_bundled_would_accept() {
        let spec = spec_from(minimal_drs_files(&["deliberately_injected_field"]));
        let err = validate_with_spec(&spec, &ok_payload())
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("deliberately_injected_field") || err.contains("required"),
            "{err}"
        );
    }

    #[test]
    fn http_ref_fails_closed() {
        let mut files = minimal_drs_files(&[]);
        files.insert(
            "openapi/components/schemas/Checksum.yaml".into(),
            Arc::<[u8]>::from(b"type: object\n$ref: 'https://example.com/schema.yaml'\n" as &[u8]),
        );
        let spec = spec_from(files);
        let err = resolve_schema_value(&spec).unwrap_err().to_string();
        assert!(
            err.contains("local relative") || err.contains("$ref"),
            "{err}"
        );
    }

    #[test]
    fn missing_ref_file_fails_closed() {
        let mut files = minimal_drs_files(&[]);
        files.remove("openapi/components/schemas/Checksum.yaml");
        let spec = spec_from(files);
        let err = resolve_schema_value(&spec).unwrap_err().to_string();
        assert!(err.contains("missing") || err.contains("Checksum"), "{err}");
    }

    #[test]
    fn traversal_ref_fails_closed() {
        let mut files = minimal_drs_files(&[]);
        files.insert(
            "openapi/components/schemas/Checksum.yaml".into(),
            Arc::<[u8]>::from(b"$ref: '../../../etc/passwd'\n" as &[u8]),
        );
        let spec = spec_from(files);
        let err = resolve_schema_value(&spec).unwrap_err().to_string();
        assert!(err.contains("..") || err.contains("$ref"), "{err}");
    }

    #[test]
    fn two_specs_same_component_name_do_not_cross_contaminate() {
        reset_schema_call_counters();
        let a = spec_from(minimal_drs_files(&[]));
        let b = spec_from(minimal_drs_files(&["deliberately_injected_field"]));
        validate_with_spec(&a, &ok_payload()).expect("spec A accepts");
        assert!(validate_with_spec(&b, &ok_payload()).is_err());
        validate_with_spec(&a, &ok_payload()).expect("spec A still accepts after B");
        let id_a = resolve_schema_value(&a).unwrap().1;
        let id_b = resolve_schema_value(&b).unwrap().1;
        assert_ne!(id_a.schema_document_sha256, id_b.schema_document_sha256);
        assert_ne!(id_a.schema_component_sha256, id_b.schema_component_sha256);
    }

    #[test]
    fn files_opened_is_sorted_and_is_the_closure() {
        let spec = spec_from(minimal_drs_files(&[]));
        let id = resolve_schema_value(&spec).unwrap().1;
        let mut expected: Vec<String> = vec![
            "openapi/components/schemas/AccessMethod.yaml".into(),
            "openapi/components/schemas/AccessURL.yaml".into(),
            "openapi/components/schemas/Authorizations.yaml".into(),
            "openapi/components/schemas/Checksum.yaml".into(),
            "openapi/components/schemas/DrsObject.yaml".into(),
        ];
        expected.sort();
        assert_eq!(id.files_opened, expected);
        let mut copy = id.files_opened.clone();
        copy.sort();
        assert_eq!(id.files_opened, copy);
    }
}
