// SPDX-License-Identifier: Apache-2.0
//! Execution-allowlist config for public / gated hosts (no HelixTest stubs).
//!
//! Fail closed: when [`crate::Features::execution_allowlist`] is set, every
//! required env var must be present and non-empty.

use anyhow::{bail, Context, Result};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct AllowlistExec {
    pub wes_workflow_id: String,
    pub tes_workflow_id: String,
    pub tes_image: String,
    pub tes_command: Vec<String>,
    pub input_drs: String,
}

fn require_env(name: &str) -> Result<String> {
    let v = std::env::var(name).unwrap_or_default();
    let trimmed = v.trim().to_owned();
    if trimmed.is_empty() {
        bail!(
            "execution_allowlist=true requires non-empty env {name} (fail closed; see docs/ferrum.md)"
        );
    }
    Ok(trimmed)
}

impl AllowlistExec {
    /// Load from env. Call only when `features.execution_allowlist` is true.
    pub fn from_env() -> Result<Self> {
        let wes_workflow_id = require_env("HELIXTEST_WES_ALLOWLIST_ID")?;
        let tes_workflow_id = require_env("HELIXTEST_TES_ALLOWLIST_ID")?;
        let tes_image = require_env("HELIXTEST_TES_ALLOWLIST_IMAGE")?;
        let cmd_raw = require_env("HELIXTEST_TES_ALLOWLIST_COMMAND")?;
        let tes_command: Vec<String> = serde_json::from_str(&cmd_raw).with_context(|| {
            format!("HELIXTEST_TES_ALLOWLIST_COMMAND must be a JSON string array, got: {cmd_raw}")
        })?;
        if tes_command.is_empty() {
            bail!("HELIXTEST_TES_ALLOWLIST_COMMAND must be a non-empty JSON array");
        }
        let input_drs = require_env("HELIXTEST_ALLOWLIST_INPUT_DRS")?;
        if !input_drs.starts_with("drs://") {
            bail!("HELIXTEST_ALLOWLIST_INPUT_DRS must start with drs://, got {input_drs}");
        }
        Ok(Self {
            wes_workflow_id,
            tes_workflow_id,
            tes_image,
            tes_command,
            input_drs,
        })
    }

    pub fn input_drs_object_id(&self) -> &str {
        self.input_drs
            .strip_prefix("drs://")
            .unwrap_or(self.input_drs.as_str())
            .trim_start_matches('/')
    }

    pub fn tes_create_body(&self) -> Value {
        serde_json::json!({
            "name": self.tes_workflow_id,
            "executors": [{
                "image": self.tes_image,
                "command": self.tes_command
            }],
            "tags": {
                "s4_workflow_id": self.tes_workflow_id
            },
            "inputs": [{
                "url": self.input_drs
            }]
        })
    }
}
