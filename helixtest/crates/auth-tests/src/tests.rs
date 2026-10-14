// SPDX-License-Identifier: Apache-2.0
use anyhow::Result;
use common::report::ServiceKind;
use framework::{run_all, Mode};
use std::collections::HashSet;

#[tokio::test]
#[ignore = "requires a live auth target (default localhost:8085); this crate does not start one. Run with: make test-live"]
async fn framework_auth_suite() -> Result<()> {
    let report = run_all(
        Mode::Generic,
        Some(HashSet::from([ServiceKind::Auth])),
        None,
    )
    .await?;
    anyhow::ensure!(
        !report.has_failures(),
        "framework Auth failed:\n{}",
        report.to_table()
    );
    Ok(())
}
