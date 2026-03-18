use anyhow::{Context, Result};
use serde::Deserialize;
use std::{env, fs, path::Path};

#[derive(Debug, Clone, Deserialize)]
pub struct ServiceConfig {
    #[serde(alias = "wes")]
    pub wes_url: String,
    #[serde(alias = "tes")]
    pub tes_url: String,
    #[serde(alias = "drs")]
    pub drs_url: String,
    #[serde(alias = "trs")]
    pub trs_url: String,
    #[serde(alias = "beacon")]
    pub beacon_url: String,
    #[serde(alias = "auth")]
    pub auth_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TestConfig {
    #[serde(flatten)]
    pub services: ServiceConfig,
}

impl TestConfig {
    pub fn from_env_or_file() -> Result<Self> {
        // Highest precedence: HELIXTEST_PROFILE pointing at profiles/<name>.toml
        if let Ok(profile) = env::var("HELIXTEST_PROFILE") {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
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
            return Ok(cfg);
        }

        if let Ok(path) = env::var("HELIXTEST_CONFIG") {
            let p = Path::new(&path);
            let data = fs::read_to_string(p)
                .with_context(|| format!("Failed to read config file at {}", p.display()))?;
            let cfg: TestConfig =
                toml::from_str(&data).context("Failed to parse TOML configuration")?;
            return Ok(cfg);
        }

        // Fallback: default config file name in current directory
        let default_path = Path::new("helixtest-config.toml");
        if default_path.exists() {
            let data = fs::read_to_string(default_path)
                .with_context(|| format!("Failed to read config file at {}", default_path.display()))?;
            let cfg: TestConfig =
                toml::from_str(&data).context("Failed to parse TOML configuration")?;
            return Ok(cfg);
        }

        Ok(Self {
            services: ServiceConfig {
                wes_url: env::var("WES_URL").unwrap_or_else(|_| "http://localhost:8080".to_string()),
                tes_url: env::var("TES_URL").unwrap_or_else(|_| "http://localhost:8081".to_string()),
                drs_url: env::var("DRS_URL").unwrap_or_else(|_| "http://localhost:8082".to_string()),
                trs_url: env::var("TRS_URL").unwrap_or_else(|_| "http://localhost:8083".to_string()),
                beacon_url: env::var("BEACON_URL").unwrap_or_else(|_| "http://localhost:8084".to_string()),
                auth_url: env::var("AUTH_URL").unwrap_or_else(|_| "http://localhost:8085".to_string()),
            },
        })
    }
}

