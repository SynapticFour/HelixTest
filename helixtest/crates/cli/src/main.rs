// SPDX-License-Identifier: Apache-2.0
use anyhow::{Context, Result};
use clap::Parser;
use common::config::TestConfig;
use common::http::HttpClient;
use common::logging::init_logging_verbose;
use common::report::{report_diagnostics_requested, ReportDiagnostics, ServiceKind};
use framework::{run_all, Mode as FrameworkMode};
use std::collections::HashSet;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};
use tracing::info;

#[derive(clap::ValueEnum, Debug, Clone)]
enum Mode {
    Generic,
    Ferrum,
    #[value(name = "ferrum-africa")]
    FerrumAfrica,
    #[value(name = "ferrum+infra")]
    FerrumInfra,
}

#[derive(clap::ValueEnum, Debug, Clone)]
enum AfricaProfileArg {
    Offline,
    Ont,
    Outbreak,
    Federation,
    All,
}

impl AfricaProfileArg {
    fn to_profile(&self) -> framework::africa::AfricaProfile {
        match self {
            AfricaProfileArg::Offline => framework::africa::AfricaProfile::Offline,
            AfricaProfileArg::Ont => framework::africa::AfricaProfile::Ont,
            AfricaProfileArg::Outbreak => framework::africa::AfricaProfile::Outbreak,
            AfricaProfileArg::Federation => framework::africa::AfricaProfile::Federation,
            AfricaProfileArg::All => framework::africa::AfricaProfile::All,
        }
    }
}

#[derive(clap::ValueEnum, Debug, Clone)]
enum ReportFormat {
    Table,
    Json,
    Scores,
    Coverage,
}

#[derive(clap::ValueEnum, Debug, Clone)]
enum ServiceArg {
    Wes,
    Tes,
    Drs,
    Trs,
    Beacon,
    Htsget,
    Auth,
    Age,
    Crypt4gh,
    E2e,
    Africa,
    Infra,
}

impl ServiceArg {
    fn to_kind(&self) -> ServiceKind {
        match self {
            ServiceArg::Wes => ServiceKind::Wes,
            ServiceArg::Tes => ServiceKind::Tes,
            ServiceArg::Drs => ServiceKind::Drs,
            ServiceArg::Trs => ServiceKind::Trs,
            ServiceArg::Beacon => ServiceKind::Beacon,
            ServiceArg::Htsget => ServiceKind::Htsget,
            ServiceArg::Auth => ServiceKind::Auth,
            ServiceArg::Age => ServiceKind::Age,
            ServiceArg::Crypt4gh => ServiceKind::Crypt4gh,
            ServiceArg::E2e => ServiceKind::E2e,
            ServiceArg::Africa => ServiceKind::Africa,
            ServiceArg::Infra => ServiceKind::Infra,
        }
    }
}

const BANNER: &str = "HelixTest — GA4GH Conformance Suite";
const CREDIT: &str = "Synaptic Four · Apache-2.0";

#[derive(Parser, Debug)]
#[command(name = "helixtest")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "HelixTest — GA4GH Conformance Suite")]
#[command(
    after_help = "Synaptic Four · Apache-2.0 · Contact: contact@synapticfour.com · synapticfour.com"
)]
struct Args {
    /// Run full HelixTest conformance suite
    #[arg(long)]
    all: bool,

    /// Execution mode (generic GA4GH vs Ferrum-native)
    #[arg(long, value_enum, default_value_t = Mode::Generic)]
    mode: Mode,

    /// Optionally start a compose stack before running tests (uses CWD or --compose-file)
    #[arg(long)]
    start_ferrum: bool,

    /// docker compose file for --start-ferrum
    #[arg(long)]
    compose_file: Option<String>,

    /// Profile name from `helixtest/profiles/<name>.toml`
    #[arg(long)]
    profile: Option<String>,

    /// Report format (table, json, or scores)
    #[arg(long, value_enum, default_value_t = ReportFormat::Table)]
    report: ReportFormat,

    /// Minimum compliance level (0-5) required; exit non-zero if overall level is lower
    #[arg(long)]
    fail_level: Option<u8>,

    /// Limit report to specific services (can be specified multiple times)
    #[arg(long, value_enum)]
    only: Vec<ServiceArg>,

    /// Africa profile when using --mode ferrum-africa (offline, ont, outbreak, federation, all)
    #[arg(long, value_enum, default_value_t = AfricaProfileArg::All)]
    africa_profile: AfricaProfileArg,

    /// Enable verbose logging (sets debug if RUST_LOG is not already set)
    #[arg(long)]
    verbose: bool,
}

fn resolve_profile(args: &Args) -> Option<String> {
    if let Some(p) = &args.profile {
        return Some(p.clone());
    }
    match args.mode {
        Mode::FerrumAfrica => Some("ferrum-africa".into()),
        Mode::FerrumInfra => Some("ferrum-infra".into()),
        _ => None,
    }
}

async fn wait_for_wes(client: &HttpClient, wes_url: &str) -> Result<()> {
    let url = format!("{}/service-info", wes_url.trim_end_matches('/'));
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if client.get_json(&url).await.is_ok() {
            info!(%url, "WES service-info reachable");
            return Ok(());
        }
        if Instant::now() > deadline {
            anyhow::bail!("WES not healthy at {url} after 60s");
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

fn start_compose(compose_file: Option<&str>) -> Result<()> {
    let mut cmd = Command::new("docker");
    cmd.arg("compose");
    if let Some(f) = compose_file {
        cmd.arg("-f").arg(f);
    } else {
        let nested = Path::new("helixtest/docker/docker-compose.yml");
        if nested.exists() {
            cmd.arg("-f").arg(nested);
        }
    }
    let status = cmd.arg("up").arg("-d").status()?;
    if !status.success() {
        anyhow::bail!(
            "HelixTest: Failed to start compose. Pass --compose-file or run compose from the target stack repo."
        );
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    init_logging_verbose(args.verbose);

    let profile = resolve_profile(&args);

    if args.all {
        if matches!(args.report, ReportFormat::Table) {
            println!("{}", BANNER);
            println!("{}\n", CREDIT);
        }

        let cfg = TestConfig::load(profile.as_deref())?;
        let client = HttpClient::new();

        if args.start_ferrum {
            info!(action = "start_ferrum", "Starting stack via docker compose");
            start_compose(args.compose_file.as_deref())?;
            wait_for_wes(&client, &cfg.services.wes_url)
                .await
                .context("Waiting for WES after --start-ferrum")?;
        }

        let framework_mode = match args.mode {
            Mode::Generic => FrameworkMode::Generic,
            Mode::Ferrum => FrameworkMode::Ferrum,
            Mode::FerrumAfrica => FrameworkMode::FerrumAfrica,
            Mode::FerrumInfra => FrameworkMode::FerrumInfra,
        };

        info!(mode = ?args.mode, profile = ?profile, "Running HelixTest conformance suite");
        let run_started = Instant::now();
        let mut report = if matches!(args.mode, Mode::FerrumAfrica) {
            framework::africa::run_africa(args.africa_profile.to_profile(), profile.as_deref())
                .await
                .context("HelixTest Africa mode run failed (check config and service URLs)")?
        } else if matches!(args.mode, Mode::FerrumInfra) {
            framework::infra::run_infra(profile.as_deref())
                .await
                .context("HelixTest ferrum+infra mode run failed (check co-deploy stack URLs)")?
        } else {
            let only = if args.only.is_empty() {
                None
            } else {
                Some(
                    args.only
                        .iter()
                        .map(|s| s.to_kind())
                        .collect::<HashSet<_>>(),
                )
            };
            run_all(framework_mode, only, profile.clone())
                .await
                .context("HelixTest conformance run failed (check config and service URLs)")?
        };
        if report_diagnostics_requested() {
            report.diagnostics = Some(ReportDiagnostics {
                suite_duration_ms: run_started.elapsed().as_millis() as u64,
                note: Some(
                    "Diagnostics (e.g. suite_duration_ms) are not used for compliance levels or scores; set HELIXTEST_REPORT_DIAGNOSTICS only for troubleshooting."
                        .into(),
                ),
            });
        }
        report.sort_services_canonical();

        match args.report {
            ReportFormat::Table => {
                println!("{}", report.to_table());
            }
            ReportFormat::Json => {
                let json = serde_json::to_string_pretty(&report)?;
                println!("{}", json);
            }
            ReportFormat::Scores => {
                let summary = report.score_summary();
                let json = serde_json::to_string_pretty(&summary)?;
                println!("{}", json);
            }
            ReportFormat::Coverage => {
                let coverage = report.coverage_summary();
                let json = serde_json::to_string_pretty(&coverage)?;
                println!("{}", json);
            }
        }

        let mut exit_code = 0;
        if report.has_failures() {
            exit_code = 1;
        }
        if let Some(min_level) = args.fail_level {
            let overall_level = report.overall_level().as_int();
            if overall_level < min_level {
                exit_code = 1;
            }
        }
        if exit_code != 0 {
            std::process::exit(1);
        }
    } else {
        println!("Nothing to do. Pass --all to run the full HelixTest conformance suite.");
    }
    Ok(())
}
