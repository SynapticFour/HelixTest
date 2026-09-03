// SPDX-License-Identifier: Apache-2.0
pub mod africa;
pub mod auth;
pub mod beacon;
pub mod crypt4gh;
mod crypt4gh_ferrum_http;
pub mod drs;
pub mod e2e;
pub mod htsget;
pub mod infra;
pub mod tes;
pub mod trs;
pub mod wes;

use anyhow::Context;
use common::config::TestConfig;
use common::http::HttpClient;
use common::report::{
    ComplianceLevel, OverallReport, ServiceKind, ServiceReport, SkippedService, TestCaseResult,
    TestCategory,
};
use common::util::{level0_reachable_ok, profiles_dir};

pub(crate) fn level0_http(
    name: &str,
    res: Result<reqwest::Response, reqwest::Error>,
) -> TestCaseResult {
    match res {
        Ok(resp) if level0_reachable_ok(resp.status()) => {
            TestCaseResult::pass(name, ComplianceLevel::Level0, TestCategory::Other)
        }
        Ok(resp) => TestCaseResult::fail(
            name,
            ComplianceLevel::Level0,
            TestCategory::Other,
            format!("Unexpected HTTP status: {}", resp.status()),
        ),
        Err(e) => TestCaseResult::fail(name, ComplianceLevel::Level0, TestCategory::Other, e),
    }
}
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use tracing::warn;

#[derive(Debug, Clone, Copy)]
pub enum Mode {
    Generic,
    Ferrum,
    FerrumAfrica,
    FerrumInfra,
}

impl Mode {
    pub fn parse(s: &str) -> Self {
        match s {
            "ferrum" | "Ferrum" => Mode::Ferrum,
            "ferrum-africa" | "FerrumAfrica" => Mode::FerrumAfrica,
            "ferrum+infra" | "ferrum-infra" | "FerrumInfra" => Mode::FerrumInfra,
            "generic" | "Generic" | "" => Mode::Generic,
            other => {
                warn!(mode = other, "unknown mode; using generic");
                Mode::Generic
            }
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

fn parse_features_file(path: &Path) -> anyhow::Result<Features> {
    let data = fs::read_to_string(path)
        .with_context(|| format!("Failed to read features from {}", path.display()))?;
    let v: toml::Value = toml::from_str(&data)
        .with_context(|| format!("Failed to parse TOML {}", path.display()))?;
    match v.get("features") {
        Some(feat) => feat
            .clone()
            .try_into::<Features>()
            .with_context(|| format!("Failed to parse [features] in {}", path.display())),
        None => {
            warn!(path = %path.display(), "profile has no [features]; using defaults");
            Ok(Features::default())
        }
    }
}

fn load_features(mode: Mode, profile: Option<&str>) -> anyhow::Result<Features> {
    if let Some(profile) = profile.filter(|s| !s.is_empty()) {
        let path = profiles_dir()?.join(format!("{}.toml", profile));
        if !path.exists() {
            anyhow::bail!("HELIXTEST profile not found: {}", path.display());
        }
        return parse_features_file(&path);
    }

    if let Mode::Ferrum = mode {
        let path = profiles_dir()?.join("ferrum.toml");
        if path.exists() {
            return parse_features_file(&path);
        }
        warn!("Ferrum mode but profiles/ferrum.toml missing; using default features");
    }

    Ok(Features::default())
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
        "age" => Some(ServiceKind::Age),
        "crypt4gh" => Some(ServiceKind::Crypt4gh),
        "e2e" => Some(ServiceKind::E2e),
        "africa" => Some(ServiceKind::Africa),
        "infra" => Some(ServiceKind::Infra),
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
        ServiceKind::Age,
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

pub async fn run_all(
    mode: Mode,
    only: Option<HashSet<ServiceKind>>,
    profile: Option<String>,
) -> anyhow::Result<OverallReport> {
    let profile_ref = profile.as_deref();
    let cfg = TestConfig::load(profile_ref)?;
    let client = HttpClient::new();
    // Generic stays generic. Ferrum is opt-in (`--mode ferrum` / ferrum-africa / ferrum+infra).
    // Inferring Ferrum from WES service-info `name` made generic runs look like a Ferrum self-test.
    let features = load_features(mode, profile_ref)?;

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
    for kind in [ServiceKind::Africa, ServiceKind::Infra] {
        if enabled.remove(&kind) {
            skipped_services.push(SkippedService {
                service: kind,
                reason: "use --mode ferrum-africa or --mode ferrum+infra".to_string(),
            });
        }
    }
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
            ServiceKind::Wes => wes::run_wes_checks(mode, &features, &cfg, &client).await?,
            ServiceKind::Tes => tes::run_tes_checks(mode, &features, &cfg, &client).await?,
            ServiceKind::Drs => drs::run_drs_checks(mode, &features, &cfg, &client).await?,
            ServiceKind::Trs => trs::run_trs_checks(mode, &features, &cfg, &client).await?,
            ServiceKind::Beacon => {
                beacon::run_beacon_checks(mode, &features, &cfg, &client).await?
            }
            ServiceKind::Htsget => {
                htsget::run_htsget_checks(mode, &features, &cfg, &client).await?
            }
            ServiceKind::Auth => {
                if matches!(mode, Mode::Ferrum) && skip_auth {
                    ServiceReport {
                        service: ServiceKind::Auth,
                        tests: vec![TestCaseResult::skip(
                            "Auth suite skipped (HELIXTEST_SKIP_AUTH=true)",
                            ComplianceLevel::Level4,
                            TestCategory::Security,
                            "HELIXTEST_SKIP_AUTH=true in Ferrum mode",
                        )],
                    }
                } else {
                    auth::run_auth_checks(mode, &features, &cfg, &client).await?
                }
            }
            ServiceKind::Age => crypt4gh::run_age_checks(mode, &features, &cfg, &client).await?,
            ServiceKind::Crypt4gh => {
                crypt4gh::run_crypt4gh_checks(mode, &features, &cfg, &client).await?
            }
            ServiceKind::E2e => e2e::run_e2e_checks(mode, &features, &cfg, &client).await?,
            ServiceKind::Africa | ServiceKind::Infra => {
                unreachable!("stripped from enabled before the loop")
            }
        };
        executed_test_modules.push(service);
        services.push(report);
    }
    let mut enabled_services: Vec<ServiceKind> = enabled.into_iter().collect();
    enabled_services.sort_by_key(|s| s.canonical_order());
    Ok(OverallReport {
        services,
        enabled_services,
        skipped_services,
        executed_test_modules,
        diagnostics: None,
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
    fn africa_in_enabled_services_is_parsed() {
        let cfg = cfg_with_subset(&["wes", "africa"], &[]);
        let enabled = enabled_services_from_config(&cfg);
        assert!(enabled.contains(&ServiceKind::Africa));
        assert!(enabled.contains(&ServiceKind::Wes));
    }

    #[test]
    fn disabled_services_override_enabled_services() {
        let cfg = cfg_with_subset(&["wes", "drs", "auth"], &["auth"]);
        let enabled = enabled_services_from_config(&cfg);
        assert!(enabled.contains(&ServiceKind::Wes));
        assert!(!enabled.contains(&ServiceKind::Auth));
    }
}
