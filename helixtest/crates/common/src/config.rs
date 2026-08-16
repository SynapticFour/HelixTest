// SPDX-License-Identifier: Apache-2.0
use anyhow::{Context, Result};
use serde::Deserialize;
use std::{env, fs, path::Path};

use crate::util::profiles_dir;

#[derive(Debug, Clone, Deserialize)]
pub struct ServiceConfig {
    #[serde(default, alias = "wes")]
    pub wes_url: String,
    #[serde(default, alias = "tes")]
    pub tes_url: String,
    #[serde(default, alias = "drs")]
    pub drs_url: String,
    #[serde(default, alias = "trs")]
    pub trs_url: String,
    #[serde(default, alias = "beacon")]
    pub beacon_url: String,
    #[serde(default, alias = "auth")]
    pub auth_url: String,
    /// Optional htsget base URL (path prefix `/ga4gh/htsget/v1` is usually included).
    #[serde(default, alias = "htsget")]
    pub htsget_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SubsetConfig {
    #[serde(default)]
    pub enabled_services: Vec<String>,
    #[serde(default)]
    pub disabled_services: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ProtectedEndpointConfig {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub check_invalid_token: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct AuthChecksConfig {
    /// Modes:
    /// - "ga4gh-passports" (default)
    /// - "token-protected-endpoints"
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub protected_endpoints: Vec<ProtectedEndpointConfig>,
    /// Environment variable name containing the valid bearer token.
    #[serde(default)]
    pub valid_token_env: Option<String>,
    /// Optional static invalid token used for negative checks.
    #[serde(default)]
    pub invalid_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TestConfig {
    #[serde(flatten)]
    pub services: ServiceConfig,
    #[serde(default)]
    pub subset: SubsetConfig,
    #[serde(default)]
    pub auth_checks: AuthChecksConfig,
}

impl TestConfig {
    fn apply_env_overrides(mut cfg: Self) -> Self {
        if let Ok(v) = env::var("WES_URL") {
            cfg.services.wes_url = v;
        }
        if let Ok(v) = env::var("TES_URL") {
            cfg.services.tes_url = v;
        }
        if let Ok(v) = env::var("DRS_URL") {
            cfg.services.drs_url = v;
        }
        if let Ok(v) = env::var("TRS_URL") {
            cfg.services.trs_url = v;
        }
        if let Ok(v) = env::var("BEACON_URL") {
            cfg.services.beacon_url = v;
        }
        if let Ok(v) = env::var("AUTH_URL") {
            cfg.services.auth_url = v;
        }
        if let Ok(v) = env::var("HTSGET_URL") {
            cfg.services.htsget_url = Some(v);
        }
        cfg
    }

    pub fn from_env_or_file() -> Result<Self> {
        Self::load(None)
    }

    /// Load config. `profile_override` wins over `HELIXTEST_PROFILE` (no process-env mutation).
    pub fn load(profile_override: Option<&str>) -> Result<Self> {
        let profile = profile_override
            .map(|s| s.to_string())
            .or_else(|| env::var("HELIXTEST_PROFILE").ok());

        if let Some(profile) = profile {
            let path = profiles_dir()?.join(format!("{}.toml", profile));
            let data = fs::read_to_string(&path).with_context(|| {
                format!(
                    "Failed to read profile config at {} (from profile {:?})",
                    path.display(),
                    profile
                )
            })?;
            let cfg: TestConfig =
                toml::from_str(&data).context("Failed to parse profile TOML configuration")?;
            return Ok(Self::apply_env_overrides(cfg));
        }

        if let Ok(path) = env::var("HELIXTEST_CONFIG") {
            let p = Path::new(&path);
            let data = fs::read_to_string(p)
                .with_context(|| format!("Failed to read config file at {}", p.display()))?;
            let cfg: TestConfig =
                toml::from_str(&data).context("Failed to parse TOML configuration")?;
            return Ok(Self::apply_env_overrides(cfg));
        }

        let default_path = Path::new("helixtest-config.toml");
        if default_path.exists() {
            let data = fs::read_to_string(default_path).with_context(|| {
                format!("Failed to read config file at {}", default_path.display())
            })?;
            let cfg: TestConfig =
                toml::from_str(&data).context("Failed to parse TOML configuration")?;
            return Ok(Self::apply_env_overrides(cfg));
        }

        Ok(Self {
            services: ServiceConfig {
                wes_url: env::var("WES_URL")
                    .unwrap_or_else(|_| "http://localhost:8080".to_string()),
                tes_url: env::var("TES_URL")
                    .unwrap_or_else(|_| "http://localhost:8081".to_string()),
                drs_url: env::var("DRS_URL")
                    .unwrap_or_else(|_| "http://localhost:8082".to_string()),
                trs_url: env::var("TRS_URL")
                    .unwrap_or_else(|_| "http://localhost:8083".to_string()),
                beacon_url: env::var("BEACON_URL")
                    .unwrap_or_else(|_| "http://localhost:8084".to_string()),
                auth_url: env::var("AUTH_URL")
                    .unwrap_or_else(|_| "http://localhost:8085".to_string()),
                htsget_url: env::var("HTSGET_URL").ok().filter(|s| !s.trim().is_empty()),
            },
            subset: SubsetConfig::default(),
            auth_checks: AuthChecksConfig::default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tempfile::tempdir;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn set_env(key: &str, val: &str) {
        // SAFETY: tests hold `ENV_LOCK`; this is the only mutation of these keys.
        unsafe { env::set_var(key, val) }
    }

    fn unset_env(key: &str) {
        unsafe { env::remove_var(key) }
    }

    fn clear_env() {
        for k in [
            "HELIXTEST_PROFILE",
            "HELIXTEST_CONFIG",
            "WES_URL",
            "TES_URL",
            "DRS_URL",
            "TRS_URL",
            "BEACON_URL",
            "AUTH_URL",
            "HTSGET_URL",
        ] {
            unset_env(k);
        }
    }

    #[test]
    fn env_fallback_defaults_work() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_env();

        let cfg = TestConfig::from_env_or_file().unwrap();
        assert_eq!(cfg.services.wes_url, "http://localhost:8080");
        assert_eq!(cfg.services.auth_url, "http://localhost:8085");
    }

    #[test]
    fn env_vars_override_defaults() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_env();
        set_env("WES_URL", "http://example-wes");
        set_env("TES_URL", "http://example-tes");

        let cfg = TestConfig::from_env_or_file().unwrap();
        assert_eq!(cfg.services.wes_url, "http://example-wes");
        assert_eq!(cfg.services.tes_url, "http://example-tes");
    }

    #[test]
    fn env_overrides_config_file_values() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_env();

        set_env("WES_URL", "http://env-wes");

        let dir = tempdir().unwrap();
        let p = dir.path().join("cfg.toml");
        fs::write(
            &p,
            r#"
wes_url = "http://file-wes"
tes_url = "http://file-tes"
drs_url = "http://file-drs"
trs_url = "http://file-trs"
beacon_url = "http://file-beacon"
auth_url = "http://file-auth"
"#,
        )
        .unwrap();
        set_env("HELIXTEST_CONFIG", p.to_string_lossy().as_ref());

        let cfg = TestConfig::from_env_or_file().unwrap();
        assert_eq!(cfg.services.wes_url, "http://env-wes");
        assert_eq!(cfg.services.tes_url, "http://file-tes");
    }

    #[test]
    fn profile_has_highest_precedence() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_env();

        set_env("HELIXTEST_PROFILE", "does-not-exist");
        let err = TestConfig::from_env_or_file().unwrap_err().to_string();
        assert!(
            err.contains("HELIXTEST_PROFILE")
                || err.contains("profile config")
                || err.contains("does-not-exist"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn load_profile_override_does_not_require_env() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_env();
        let err = TestConfig::load(Some("does-not-exist"))
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("does-not-exist") || err.contains("profile"),
            "unexpected error: {}",
            err
        );
    }
}
