use anyhow::{Context, Result};
use serde::Deserialize;
use std::{env, fs, path::Path};

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
        // Highest precedence: HELIXTEST_PROFILE pointing at profiles/<name>.toml
        if let Ok(profile) = env::var("HELIXTEST_PROFILE") {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("profiles")
                .join(format!("{}.toml", profile));
            let data = fs::read_to_string(&path).with_context(|| {
                format!(
                    "Failed to read profile config at {} (from HELIXTEST_PROFILE)",
                    path.display()
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

        // Fallback: default config file name in current directory
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
            env::remove_var(k);
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
        env::set_var("WES_URL", "http://example-wes");
        env::set_var("TES_URL", "http://example-tes");

        let cfg = TestConfig::from_env_or_file().unwrap();
        assert_eq!(cfg.services.wes_url, "http://example-wes");
        assert_eq!(cfg.services.tes_url, "http://example-tes");
    }

    #[test]
    fn env_overrides_config_file_values() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_env();

        env::set_var("WES_URL", "http://env-wes");

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
        env::set_var("HELIXTEST_CONFIG", p.to_string_lossy().to_string());

        let cfg = TestConfig::from_env_or_file().unwrap();
        assert_eq!(cfg.services.wes_url, "http://env-wes");
        assert_eq!(cfg.services.tes_url, "http://file-tes");
    }

    #[test]
    fn profile_has_highest_precedence() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_env();

        // Create a temporary "profiles/<name>.toml" by faking CARGO_MANIFEST_DIR is not possible,
        // so we only assert that when HELIXTEST_PROFILE is set to a missing profile, we get a clear error.
        env::set_var("HELIXTEST_PROFILE", "does-not-exist");
        let err = TestConfig::from_env_or_file().unwrap_err().to_string();
        assert!(
            err.contains("HELIXTEST_PROFILE") || err.contains("profile config"),
            "unexpected error: {}",
            err
        );
    }
}
