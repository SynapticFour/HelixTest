pub mod auth;
pub mod beacon;
pub mod crypt4gh;
mod crypt4gh_ferrum_http;
pub mod drs;
pub mod e2e;
pub mod htsget;
pub mod tes;
pub mod trs;
pub mod wes;

use common::config::TestConfig;
use common::http::HttpClient;
use common::report::{OverallReport, ServiceKind, ServiceReport, SkippedService};
use serde::Deserialize;
use std::collections::HashSet;
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
    // First, honor HELIXTEST_PROFILE if set (generic, ferrum, strict, etc.).
    if let Ok(profile) = std::env::var("HELIXTEST_PROFILE") {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
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

fn parse_service_name(name: &str) -> Option<ServiceKind> {
    match name.trim().to_ascii_lowercase().as_str() {
        "wes" => Some(ServiceKind::Wes),
        "tes" => Some(ServiceKind::Tes),
        "drs" => Some(ServiceKind::Drs),
        "trs" => Some(ServiceKind::Trs),
        "beacon" => Some(ServiceKind::Beacon),
        "htsget" => Some(ServiceKind::Htsget),
        "auth" => Some(ServiceKind::Auth),
        "crypt4gh" => Some(ServiceKind::Crypt4gh),
        "e2e" => Some(ServiceKind::E2e),
        _ => None,
    }
}

fn all_services() -> Vec<ServiceKind> {
    vec![
        ServiceKind::Wes,
        ServiceKind::Tes,
        ServiceKind::Drs,
        ServiceKind::Trs,
        ServiceKind::Beacon,
        ServiceKind::Htsget,
        ServiceKind::Auth,
        ServiceKind::Crypt4gh,
        ServiceKind::E2e,
    ]
}

fn enabled_services_from_config(cfg: &TestConfig) -> HashSet<ServiceKind> {
    let mut set: HashSet<ServiceKind> = if cfg.subset.enabled_services.is_empty() {
        all_services().into_iter().collect()
    } else {
        cfg.subset
            .enabled_services
            .iter()
            .filter_map(|s| parse_service_name(s))
            .collect()
    };
    for disabled in &cfg.subset.disabled_services {
        if let Some(kind) = parse_service_name(disabled) {
            set.remove(&kind);
        }
    }
    set
}

pub async fn run_all(mode: Mode, only: Option<HashSet<ServiceKind>>) -> anyhow::Result<OverallReport> {
    // Auto-detect Ferrum by inspecting WES /service-info if in generic mode.
    let cfg = TestConfig::from_env_or_file()?;
    let client = HttpClient::new();
    let effective_mode = if let Mode::Generic = mode {
        let url = format!(
            "{}/service-info",
            cfg.services.wes_url.trim_end_matches('/')
        );
        if let Ok(v) = client.get_json(&url).await {
            if v.get("name")
                .and_then(|n| n.as_str())
                .map(|s| s.contains("Ferrum"))
                .unwrap_or(false)
            {
                info!(
                    service = "WES",
                    "Detected Ferrum in service-info, switching to Ferrum mode"
                );
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

    let mut enabled = enabled_services_from_config(&cfg);
    if let Some(only_set) = only {
        enabled = enabled
            .intersection(&only_set)
            .copied()
            .collect::<HashSet<ServiceKind>>();
    }
    let all = all_services();
    let mut services: Vec<ServiceReport> = Vec::new();
    let mut executed_test_modules = Vec::new();
    let mut skipped_services = Vec::new();
    let skip_auth = std::env::var("HELIXTEST_SKIP_AUTH")
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    for service in all {
        if !enabled.contains(&service) {
            skipped_services.push(SkippedService {
                service,
                reason: "skipped by profile or --only filter".to_string(),
            });
            continue;
        }
        let report = match service {
            ServiceKind::Wes => wes::run_wes_checks(effective_mode, &features, &cfg, &client).await?,
            ServiceKind::Tes => tes::run_tes_checks(effective_mode, &features, &cfg, &client).await?,
            ServiceKind::Drs => drs::run_drs_checks(effective_mode, &features, &cfg, &client).await?,
            ServiceKind::Trs => trs::run_trs_checks(effective_mode, &features, &cfg, &client).await?,
            ServiceKind::Beacon => beacon::run_beacon_checks(effective_mode, &features, &cfg, &client).await?,
            ServiceKind::Htsget => htsget::run_htsget_checks(effective_mode, &features, &cfg, &client).await?,
            ServiceKind::Auth => {
                if matches!(effective_mode, Mode::Ferrum) && skip_auth {
                    ServiceReport {
                        service: ServiceKind::Auth,
                        tests: Vec::new(),
                    }
                } else {
                    auth::run_auth_checks(effective_mode, &features, &cfg, &client).await?
                }
            }
            ServiceKind::Crypt4gh => {
                crypt4gh::run_crypt4gh_checks(effective_mode, &features, &cfg, &client).await?
            }
            ServiceKind::E2e => e2e::run_e2e_checks(effective_mode, &features, &cfg, &client).await?,
        };
        executed_test_modules.push(service);
        services.push(report);
    }
    let mut enabled_services: Vec<ServiceKind> = enabled.into_iter().collect();
    enabled_services.sort_by_key(|s| match s {
        ServiceKind::Wes => 0,
        ServiceKind::Tes => 1,
        ServiceKind::Drs => 2,
        ServiceKind::Trs => 3,
        ServiceKind::Beacon => 4,
        ServiceKind::Htsget => 5,
        ServiceKind::Auth => 6,
        ServiceKind::Crypt4gh => 7,
        ServiceKind::E2e => 8,
    });
    Ok(OverallReport {
        services,
        enabled_services,
        skipped_services,
        executed_test_modules,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::config::{AuthChecksConfig, ServiceConfig, SubsetConfig};

    fn cfg_with_subset(enabled: &[&str], disabled: &[&str]) -> TestConfig {
        TestConfig {
            services: ServiceConfig {
                wes_url: String::new(),
                tes_url: String::new(),
                drs_url: String::new(),
                trs_url: String::new(),
                beacon_url: String::new(),
                auth_url: String::new(),
                htsget_url: None,
            },
            subset: SubsetConfig {
                enabled_services: enabled.iter().map(|s| s.to_string()).collect(),
                disabled_services: disabled.iter().map(|s| s.to_string()).collect(),
            },
            auth_checks: AuthChecksConfig::default(),
        }
    }

    #[test]
    fn subset_enabled_services_limits_execution_set() {
        let cfg = cfg_with_subset(&["wes", "drs", "auth"], &[]);
        let enabled = enabled_services_from_config(&cfg);
        assert!(enabled.contains(&ServiceKind::Wes));
        assert!(enabled.contains(&ServiceKind::Drs));
        assert!(enabled.contains(&ServiceKind::Auth));
        assert!(!enabled.contains(&ServiceKind::Tes));
        assert!(!enabled.contains(&ServiceKind::Trs));
    }

    #[test]
    fn disabled_services_override_enabled_services() {
        let cfg = cfg_with_subset(&["wes", "drs", "auth"], &["auth"]);
        let enabled = enabled_services_from_config(&cfg);
        assert!(enabled.contains(&ServiceKind::Wes));
        assert!(!enabled.contains(&ServiceKind::Auth));
    }
}
