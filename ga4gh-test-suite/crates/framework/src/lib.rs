pub mod wes;
pub mod drs;
pub mod trs;
pub mod auth;
pub mod crypt4gh;
pub mod e2e;
pub mod tes;
pub mod beacon;

use common::config::TestConfig;
use common::http::HttpClient;
use common::report::{OverallReport, ServiceReport};
use serde::Deserialize;
use std::fs;
use std::path::Path;
use tracing::info;

#[derive(Debug, Clone, Copy)]
pub enum Mode {
    Generic,
    Ferrum,
}

impl Mode {
    pub fn from_str(s: &str) -> Self {
        match s {
            "ferrum" | "Ferrum" => Mode::Ferrum,
            _ => Mode::Generic,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Features {
    #[serde(default)]
    pub supports_scatter_gather: bool,
    #[serde(default)]
    pub supports_beacon_v2: bool,
    #[serde(default)]
    pub strict_drs_checksums: bool,
}

fn load_features(mode: Mode) -> Features {
    // First, honor GA4GH_TEST_PROFILE if set (generic, ferrum, strict, etc.).
    if let Ok(profile) = std::env::var("GA4GH_TEST_PROFILE") {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("profiles")
            .join(format!("{}.toml", profile));
        if path.exists() {
            if let Ok(data) = fs::read_to_string(&path) {
                if let Ok(v) = toml::from_str::<toml::Value>(&data) {
                    if let Some(feat) = v.get("features") {
                        if let Ok(parsed) = feat.clone().try_into::<Features>() {
                            return parsed;
                        }
                    }
                }
            }
        }
    }

    // Backwards-compatible default: Ferrum-specific profile when in Ferrum mode.
    if let Mode::Ferrum = mode {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("profiles")
            .join("ferrum.toml");
        if path.exists() {
            if let Ok(data) = fs::read_to_string(&path) {
                if let Ok(v) = toml::from_str::<toml::Value>(&data) {
                    if let Some(feat) = v.get("features") {
                        if let Ok(parsed) = feat.clone().try_into::<Features>() {
                            return parsed;
                        }
                    }
                }
            }
        }
    }

    Features::default()
}

pub async fn run_all(mode: Mode) -> anyhow::Result<OverallReport> {
    // Auto-detect Ferrum by inspecting WES /service-info if in generic mode.
    let cfg = TestConfig::from_env_or_file()?;
    let client = HttpClient::new();
    let effective_mode = if let Mode::Generic = mode {
        let url = format!("{}/service-info", cfg.services.wes_url.trim_end_matches('/'));
        if let Ok(v) = client.get_json(&url).await {
            if v
                .get("name")
                .and_then(|n| n.as_str())
                .map(|s| s.contains("Ferrum"))
                .unwrap_or(false)
            {
                info!(service = "WES", "Detected Ferrum in service-info, switching to Ferrum mode");
                Mode::Ferrum
            } else {
                Mode::Generic
            }
        } else {
            Mode::Generic
        }
    } else {
        mode
    };

    let features = load_features(effective_mode);

    let mut services: Vec<ServiceReport> = Vec::new();
    services.push(wes::run_wes_checks(effective_mode, &features, &cfg, &client).await?);
    services.push(drs::run_drs_checks(effective_mode, &features, &cfg, &client).await?);
    services.push(trs::run_trs_checks(effective_mode, &features, &cfg, &client).await?);
    services.push(tes::run_tes_checks(effective_mode, &features, &cfg, &client).await?);
    services.push(beacon::run_beacon_checks(effective_mode, &features, &cfg, &client).await?);
    services.push(auth::run_auth_checks(effective_mode, &features, &cfg, &client).await?);
    services.push(crypt4gh::run_crypt4gh_checks(effective_mode, &features, &cfg, &client).await?);
    services.push(e2e::run_e2e_checks(effective_mode, &features, &cfg, &client).await?);
    Ok(OverallReport { services })
}

