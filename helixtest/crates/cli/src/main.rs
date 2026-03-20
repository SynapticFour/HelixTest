use anyhow::{Context, Result};
use clap::Parser;
use common::logging::init_logging;
use common::report::ServiceKind;
use framework::{run_all, Mode as FrameworkMode};
use std::collections::HashSet;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;
use tracing::info;

#[derive(clap::ValueEnum, Debug, Clone)]
enum Mode {
    Generic,
    Ferrum,
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
    Crypt4gh,
    E2e,
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
            ServiceArg::Crypt4gh => ServiceKind::Crypt4gh,
            ServiceArg::E2e => ServiceKind::E2e,
        }
    }
}

const BANNER: &str = "🧬 HelixTest — GA4GH Conformance Suite";
const CREDIT: &str = "Built with ❤️ by Synaptic Four · Apache-2.0";

#[derive(Parser, Debug)]
#[command(name = "helixtest")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "HelixTest — GA4GH Conformance Suite")]
#[command(
    after_help = "Synaptic Four — Built with ❤️ for the open science community. GA4GH open standards for sovereign bioinformatics. Proudly developed by individuals on the autism spectrum in Germany. © 2025 Synaptic Four · Apache-2.0. Contact: contact@synapticfour.com · synapticfour.com"
)]
struct Args {
    /// Run full HelixTest conformance suite
    #[arg(long)]
    all: bool,

    /// Execution mode (generic GA4GH vs Ferrum-native)
    #[arg(long, value_enum, default_value_t = Mode::Generic)]
    mode: Mode,

    /// Optionally start Ferrum via docker-compose before running tests
    #[arg(long)]
    start_ferrum: bool,

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

    /// Enable verbose logging (sets RUST_LOG=debug if not already set)
    #[arg(long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Configure logging verbosity before initializing tracing.
    if args.verbose && std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "debug,helixtest=debug");
    }
    if let Some(profile) = &args.profile {
        std::env::set_var("HELIXTEST_PROFILE", profile);
    }
    init_logging();
    if args.all {
        println!("{}", BANNER);
        println!("{}\n", CREDIT);
        if args.start_ferrum {
            info!(
                action = "start_ferrum",
                "Starting Ferrum via docker compose"
            );
            let status = Command::new("docker")
                .arg("compose")
                .arg("up")
                .arg("-d")
                .status()?;
            if !status.success() {
                anyhow::bail!(
                    "HelixTest: Failed to start Ferrum. Run `docker compose up -d` from the docker/ directory and check that Docker is running."
                );
            }
            // Simple wait loop to give services time to become healthy.
            let wait_secs = 10;
            info!(wait_secs, "Waiting for Ferrum services to become healthy");
            sleep(Duration::from_secs(wait_secs));
        }

        let framework_mode = match args.mode {
            Mode::Generic => FrameworkMode::Generic,
            Mode::Ferrum => FrameworkMode::Ferrum,
        };

        info!(mode = ?args.mode, "Running HelixTest conformance suite");
        let only = if args.only.is_empty() {
            None
        } else {
            Some(args.only.iter().map(|s| s.to_kind()).collect::<HashSet<_>>())
        };
        let mut report = run_all(framework_mode, only)
            .await
            .context("HelixTest conformance run failed (check config and service URLs)")?;
        // Deterministic output: same order every time (table and JSON).
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

        // Exit status logic:
        // - If any test failed, exit 1.
        // - Additionally, if --fail-level is set and overall level is below it, exit 1.
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
