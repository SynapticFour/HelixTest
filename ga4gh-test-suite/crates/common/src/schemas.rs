use jsonschema::{Draft, JSONSchema};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

pub fn validate_json_against<T>(value: &Value) -> anyhow::Result<()>
where
    T: JsonSchema + for<'de> DeserializeOwned + Serialize,
{
    let schema = schemars::schema_for!(T);
    let schema_value = serde_json::to_value(&schema.schema)?;
    let schema_static: &'static serde_json::Value = Box::leak(Box::new(schema_value));
    let compiled = JSONSchema::options()
        .with_draft(Draft::Draft7)
        .compile(schema_static)?;

    let result = compiled.validate(value);
    if let Err(errors) = result {
        let mut msgs = Vec::new();
        for e in errors {
            msgs.push(format!("{} at {}", e, e.instance_path));
        }
        anyhow::bail!("JSON did not validate against schema: {}", msgs.join(", "));
    }
    Ok(())
}

/// Helper to assert required fields exist and have expected JSON types.
pub fn assert_required_string_field(value: &Value, field: &str) -> anyhow::Result<String> {
    let v = value
        .get(field)
        .ok_or_else(|| anyhow::anyhow!("Missing required field `{}`", field))?;
    if let Some(s) = v.as_str() {
        Ok(s.to_owned())
    } else {
        anyhow::bail!("Field `{}` is not a string: {}", field, v);
    }
}

