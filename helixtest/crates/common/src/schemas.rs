// SPDX-License-Identifier: Apache-2.0
use serde_json::Value;

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
