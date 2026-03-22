//! WES run submission and status polling helpers.
//!
//! **Conformance semantics (not performance):** Polling continues until the WES `/runs/{id}/status`
//! response reports a **terminal** state (`COMPLETE`, `EXECUTOR_ERROR`, `SYSTEM_ERROR`, `CANCELED`)
//! or until a **timeout** elapses. Between polls, the implementation may report any valid
//! non-terminal state (`QUEUED`, `INITIALIZING`, `RUNNING`, or implementation-specific states we
//! treat as forward progress). This does **not** require other backends (e.g. TES) to be terminal
//! while WES is still `RUNNING`—only the WES API contract is asserted here. Success for a run is
//! determined by the **final** terminal state and (in tests) outputs, not by wall-clock duration.

use crate::http::HttpClient;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WesRunRequest {
    pub workflow_url: String,
    #[serde(default)]
    pub workflow_type: String,
    #[serde(default)]
    pub workflow_type_version: String,
    #[serde(default)]
    pub tags: Option<serde_json::Value>,
    #[serde(default)]
    pub workflow_params: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WesRunStatus {
    pub run_id: String,
    pub state: String,
    #[serde(skip)]
    pub states_history: Vec<String>,
}

pub async fn submit_wes_run(
    client: &HttpClient,
    wes_url: &str,
    req: &WesRunRequest,
) -> Result<String> {
    let url = format!("{}/runs", wes_url.trim_end_matches('/'));
    let body = serde_json::to_value(req)?;
    let resp = client.post_json(&url, &body).await?;
    let run_id = resp
        .get("run_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing run_id in WES run response: {}", resp))?;
    Ok(run_id.to_owned())
}

/// Poll WES `/runs/{id}/status` until a terminal state or `timeout`.
///
/// Intermediate non-terminal states may repeat; transitions must be monotonic in phase order
/// (validated when a terminal state is observed). **Canonical success** for callers is: terminal state
/// `COMPLETE` plus expected outputs—handled in each test, not inside this function.
pub async fn poll_wes_run_until_terminal(
    client: &HttpClient,
    wes_url: &str,
    run_id: &str,
    timeout: Duration,
    poll_interval: Duration,
) -> Result<WesRunStatus> {
    let url = format!("{}/runs/{}/status", wes_url.trim_end_matches('/'), run_id);
    let start = std::time::Instant::now();
    let mut states_seen = Vec::new();
    loop {
        let v = client.get_json(&url).await?;
        let state = v
            .get("state")
            .and_then(|s| s.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing state in WES status: {}", v))?;
        states_seen.push(state.to_owned());
        info!(run_id, %state, "WES run state");
        if is_terminal_state(state) {
            validate_state_sequence(&states_seen)?;
            return Ok(WesRunStatus {
                run_id: run_id.to_owned(),
                state: state.to_owned(),
                states_history: states_seen,
            });
        }
        if start.elapsed() > timeout {
            warn!(run_id, "WES run polling timed out");
            anyhow::bail!("Timed out waiting for WES run {}", run_id);
        }
        sleep(poll_interval).await;
    }
}

fn is_terminal_state(state: &str) -> bool {
    matches!(
        state,
        "COMPLETE" | "EXECUTOR_ERROR" | "SYSTEM_ERROR" | "CANCELED"
    )
}

fn validate_state_sequence(states: &[String]) -> Result<()> {
    if states.is_empty() {
        anyhow::bail!("No states observed for WES run");
    }

    // GA4GH WES typical lifecycle states we care about
    const QUEUED: &str = "QUEUED";
    const INITIALIZING: &str = "INITIALIZING";
    const RUNNING: &str = "RUNNING";

    let first = &states[0];
    let last = states.last().unwrap();

    // First state must not be terminal
    if is_terminal_state(first) {
        anyhow::bail!(
            "WES run started in terminal state: {}; full sequence: {:?}",
            first,
            states
        );
    }

    // Last state must be terminal
    if !is_terminal_state(last) {
        anyhow::bail!(
            "WES run did not end in terminal state: {}; full sequence: {:?}",
            last,
            states
        );
    }

    // Enforce monotonic progression: QUEUED -> INITIALIZING -> RUNNING -> TERMINAL
    // We allow repeated states (e.g. RUNNING, RUNNING), but no backward moves.
    #[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
    enum Phase {
        PreQueued,
        Queued,
        Initializing,
        Running,
        Terminal,
    }

    fn phase_of(state: &str) -> Phase {
        match state {
            QUEUED => Phase::Queued,
            INITIALIZING => Phase::Initializing,
            RUNNING => Phase::Running,
            s if is_terminal_state(s) => Phase::Terminal,
            // Unknown or implementation-specific states are treated as Running-like
            _ => Phase::Running,
        }
    }

    let mut prev_phase = Phase::PreQueued;
    for s in states {
        let phase = phase_of(s);
        if phase < prev_phase {
            anyhow::bail!(
                "Invalid WES state transition: {:?} -> {:?}; full sequence: {:?}",
                prev_phase,
                phase,
                states
            );
        }
        prev_phase = phase;
    }

    Ok(())
}

pub async fn fetch_wes_run_output(
    client: &HttpClient,
    wes_url: &str,
    run_id: &str,
) -> Result<serde_json::Value> {
    let url = format!("{}/runs/{}", wes_url.trim_end_matches('/'), run_id);
    let v = client.get_json(&url).await?;
    v.get("outputs")
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Missing outputs in WES run response: {}", v))
}
