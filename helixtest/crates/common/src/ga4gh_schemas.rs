//! GA4GH official OpenAPI/JSON Schema validation.
//!
//! Loads vendored GA4GH OpenAPI specs, resolves `$ref`, and validates API responses
//! against the official schemas (WES ServiceInfo, TES Task, TRS Tool/ToolVersion, htsget).

use anyhow::{Context, Result};
use jsonschema::JSONSchema;
use once_cell::sync::OnceCell;
use serde_json::Value;

/// Vendored WES OpenAPI 1.1.0 from ga4gh.github.io/workflow-execution-service-schemas/openapi.yaml
const WES_OPENAPI_YAML: &str = include_str!("../../../schemas/ga4gh/wes-openapi.yaml");
/// Vendored TES OpenAPI 1.1.0 from ga4gh/task-execution-schemas
const TES_OPENAPI_YAML: &str = include_str!("../../../schemas/ga4gh/tes-openapi.yaml");
/// Vendored TRS OpenAPI 2.1.0 from ga4gh/tool-registry-service-schemas
const TRS_OPENAPI_YAML: &str = include_str!("../../../schemas/ga4gh/trs-openapi.yaml");
/// Vendored htsget OpenAPI 1.3.0 from samtools/hts-specs pub/htsget-openapi.yaml (ServiceInfo inlined for offline resolve).
const HTSGET_OPENAPI_YAML: &str = include_str!("../../../schemas/ga4gh/htsget-openapi.yaml");

static WES_SERVICE_INFO_SCHEMA: OnceCell<JSONSchema> = OnceCell::new();
static TES_TASK_SCHEMA: OnceCell<JSONSchema> = OnceCell::new();
static TES_CREATE_TASK_RESPONSE_SCHEMA: OnceCell<JSONSchema> = OnceCell::new();
static TRS_TOOL_SCHEMA: OnceCell<JSONSchema> = OnceCell::new();
static TRS_TOOL_VERSION_SCHEMA: OnceCell<JSONSchema> = OnceCell::new();
static HTSGET_SERVICE_INFO_SCHEMA: OnceCell<JSONSchema> = OnceCell::new();
static HTSGET_TICKET_READS_SCHEMA: OnceCell<JSONSchema> = OnceCell::new();
static HTSGET_TICKET_VARIANTS_SCHEMA: OnceCell<JSONSchema> = OnceCell::new();
static HTSGET_ERROR_SCHEMA: OnceCell<JSONSchema> = OnceCell::new();

/// Resolve OpenAPI YAML and extract a schema by name from components.schemas.
/// Uses resolve() so partial resolution is accepted (e.g. when external refs exist);
/// the requested schema may still be fully resolved in the output.
fn resolve_and_get_schema(yaml: &str, schema_name: &str, context: &str) -> Result<Value> {
    let openapi_value: Value =
        serde_yaml::from_str(yaml).with_context(|| format!("{}: parse error", context))?;
    let doc = openapi_deref::resolve(&openapi_value)
        .map_err(|e| anyhow::anyhow!("Failed to resolve {}: {:?}", context, e))?;
    let resolved = match doc.into_value() {
        Ok(v) => v,
        Err(partial) => partial.value,
    };
    let schemas = resolved
        .get("components")
        .and_then(|c| c.get("schemas"))
        .with_context(|| format!("{} missing components.schemas", context))?;
    let schema = schemas
        .get(schema_name)
        .with_context(|| format!("{} missing schema {}", context, schema_name))?
        .clone();
    Ok(schema)
}

fn compile_schema(schema: Value, name: &str) -> Result<JSONSchema> {
    let static_val: &'static Value = Box::leak(Box::new(schema));
    JSONSchema::compile(static_val).context(format!("Failed to compile {} schema", name))
}

/// Load the WES OpenAPI YAML, resolve all `$ref`, and return the ServiceInfo schema.
fn load_wes_service_info_schema() -> Result<JSONSchema> {
    let service_info = resolve_and_get_schema(WES_OPENAPI_YAML, "ServiceInfo", "WES OpenAPI")?;
    compile_schema(service_info, "WES ServiceInfo")
}

fn validate_against(schema: &JSONSchema, value: &Value, schema_label: &str) -> Result<()> {
    schema.validate(value).map_err(|errors| {
        let msgs: Vec<String> = errors
            .map(|e| format!("{} at {}", e, e.instance_path))
            .collect();
        anyhow::anyhow!(
            "JSON did not validate against {}: {}",
            schema_label,
            msgs.join("; ")
        )
    })?;
    Ok(())
}

/// Validate a JSON value against the official GA4GH WES ServiceInfo schema.
/// Use this for the response of GET /service-info.
pub fn validate_wes_service_info(value: &Value) -> Result<()> {
    let schema = WES_SERVICE_INFO_SCHEMA.get_or_try_init(load_wes_service_info_schema)?;
    validate_against(schema, value, "GA4GH WES ServiceInfo schema")
}

// --- TES (Task Execution Service) ---

fn load_tes_task_schema() -> Result<JSONSchema> {
    let schema = resolve_and_get_schema(TES_OPENAPI_YAML, "tesTask", "TES OpenAPI")?;
    compile_schema(schema, "TES tesTask")
}

fn load_tes_create_task_response_schema() -> Result<JSONSchema> {
    let schema = resolve_and_get_schema(TES_OPENAPI_YAML, "tesCreateTaskResponse", "TES OpenAPI")?;
    compile_schema(schema, "TES tesCreateTaskResponse")
}

/// Validate a JSON value against the official GA4GH TES Task schema (e.g. GET /tasks/{id} response).
pub fn validate_tes_task(value: &Value) -> Result<()> {
    let schema = TES_TASK_SCHEMA.get_or_try_init(load_tes_task_schema)?;
    validate_against(schema, value, "GA4GH TES tesTask schema")
}

/// Validate a JSON value against the official GA4GH TES CreateTask response schema (POST /tasks response).
pub fn validate_tes_create_task_response(value: &Value) -> Result<()> {
    let schema =
        TES_CREATE_TASK_RESPONSE_SCHEMA.get_or_try_init(load_tes_create_task_response_schema)?;
    validate_against(schema, value, "GA4GH TES tesCreateTaskResponse schema")
}

// --- TRS (Tool Registry Service) ---

fn load_trs_tool_schema() -> Result<JSONSchema> {
    let schema = resolve_and_get_schema(TRS_OPENAPI_YAML, "Tool", "TRS OpenAPI")?;
    compile_schema(schema, "TRS Tool")
}

fn load_trs_tool_version_schema() -> Result<JSONSchema> {
    let schema = resolve_and_get_schema(TRS_OPENAPI_YAML, "ToolVersion", "TRS OpenAPI")?;
    compile_schema(schema, "TRS ToolVersion")
}

/// Validate a JSON value against the official GA4GH TRS Tool schema (e.g. GET /tools, /tools/{id}).
pub fn validate_trs_tool(value: &Value) -> Result<()> {
    let schema = TRS_TOOL_SCHEMA.get_or_try_init(load_trs_tool_schema)?;
    validate_against(schema, value, "GA4GH TRS Tool schema")
}

/// Validate a JSON value against the official GA4GH TRS ToolVersion schema (e.g. GET /tools/{id}/versions).
pub fn validate_trs_tool_version(value: &Value) -> Result<()> {
    let schema = TRS_TOOL_VERSION_SCHEMA.get_or_try_init(load_trs_tool_version_schema)?;
    validate_against(schema, value, "GA4GH TRS ToolVersion schema")
}

// --- htsget (GA4GH htsget 1.3.0) ---

fn load_htsget_service_info_schema() -> Result<JSONSchema> {
    let schema = resolve_and_get_schema(HTSGET_OPENAPI_YAML, "htsgetServiceInfo", "htsget OpenAPI")?;
    compile_schema(schema, "htsgetServiceInfo")
}

fn load_htsget_ticket_reads_schema() -> Result<JSONSchema> {
    let schema = resolve_and_get_schema(HTSGET_OPENAPI_YAML, "htsgetResponseReads", "htsget OpenAPI")?;
    compile_schema(schema, "htsgetResponseReads")
}

fn load_htsget_ticket_variants_schema() -> Result<JSONSchema> {
    let schema = resolve_and_get_schema(HTSGET_OPENAPI_YAML, "htsgetResponseVariants", "htsget OpenAPI")?;
    compile_schema(schema, "htsgetResponseVariants")
}

fn load_htsget_error_schema() -> Result<JSONSchema> {
    let schema = resolve_and_get_schema(HTSGET_OPENAPI_YAML, "Error", "htsget OpenAPI")?;
    compile_schema(schema, "htsget Error")
}

/// Validate JSON body of `GET|POST …/reads/service-info` or `…/variants/service-info`.
pub fn validate_htsget_service_info(value: &Value) -> Result<()> {
    let schema = HTSGET_SERVICE_INFO_SCHEMA.get_or_try_init(load_htsget_service_info_schema)?;
    validate_against(schema, value, "GA4GH htsget htsgetServiceInfo schema")
}

/// Validate successful reads ticket (`htsgetResponseReads`).
pub fn validate_htsget_ticket_reads(value: &Value) -> Result<()> {
    let schema = HTSGET_TICKET_READS_SCHEMA.get_or_try_init(load_htsget_ticket_reads_schema)?;
    validate_against(schema, value, "GA4GH htsget htsgetResponseReads schema")
}

/// Validate successful variants ticket (`htsgetResponseVariants`).
pub fn validate_htsget_ticket_variants(value: &Value) -> Result<()> {
    let schema = HTSGET_TICKET_VARIANTS_SCHEMA.get_or_try_init(load_htsget_ticket_variants_schema)?;
    validate_against(schema, value, "GA4GH htsget htsgetResponseVariants schema")
}

/// Validate error payload (`Error`: `htsget.error`, `htsget.message`).
pub fn validate_htsget_error(value: &Value) -> Result<()> {
    let schema = HTSGET_ERROR_SCHEMA.get_or_try_init(load_htsget_error_schema)?;
    validate_against(schema, value, "GA4GH htsget Error schema")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wes_service_info_schema_loads_and_rejects_invalid() {
        let invalid = serde_json::json!(42);
        assert!(validate_wes_service_info(&invalid).is_err());
        let empty = serde_json::json!({});
        assert!(validate_wes_service_info(&empty).is_err());
    }

    #[test]
    fn tes_task_schema_loads_and_rejects_invalid() {
        assert!(validate_tes_task(&serde_json::json!(42)).is_err());
        assert!(validate_tes_task(&serde_json::json!({})).is_err());
    }

    #[test]
    fn tes_create_task_response_requires_id() {
        assert!(validate_tes_create_task_response(&serde_json::json!({})).is_err());
        // Minimal valid CreateTask response: id (required)
        let minimal = serde_json::json!({"id": "job-0012345"});
        validate_tes_create_task_response(&minimal)
            .expect("tesCreateTaskResponse with id should validate");
    }

    #[test]
    fn trs_tool_schema_rejects_invalid() {
        assert!(validate_trs_tool(&serde_json::json!(42)).is_err());
        assert!(validate_trs_tool(&serde_json::json!({})).is_err());
    }

    #[test]
    fn trs_tool_version_schema_rejects_invalid() {
        assert!(validate_trs_tool_version(&serde_json::json!(42)).is_err());
        assert!(validate_trs_tool_version(&serde_json::json!({})).is_err());
    }

    #[test]
    fn htsget_service_info_schema_accepts_minimal_reads_shape() {
        let v = serde_json::json!({
            "id": "test-htsget-reads",
            "name": "Test reads",
            "version": "0.0.1",
            "type": { "group": "org.ga4gh", "artifact": "htsget", "version": "1.3.0" },
            "organization": { "name": "ACME", "url": null },
            "htsget": {
                "datatype": "reads",
                "formats": ["BAM", "CRAM"],
                "fieldsParameterEffective": false,
                "tagsParametersEffective": false
            }
        });
        validate_htsget_service_info(&v).expect("minimal reads service-info should validate");
    }

    #[test]
    fn htsget_ticket_reads_requires_urls() {
        let bad = serde_json::json!({"htsget": {"format": "BAM"}});
        assert!(validate_htsget_ticket_reads(&bad).is_err());
        let ok = serde_json::json!({
            "htsget": {
                "format": "BAM",
                "urls": [{"url": "https://x.example/ga4gh/drs/v1/objects/id/stream"}]
            }
        });
        validate_htsget_ticket_reads(&ok).expect("reads ticket with urls");
    }

    #[test]
    fn htsget_error_schema_requires_nested_fields() {
        assert!(validate_htsget_error(&serde_json::json!({})).is_err());
        let ok = serde_json::json!({
            "htsget": { "error": "NotFound", "message": "nope" }
        });
        validate_htsget_error(&ok).expect("htsget error body");
    }
}
