use anyhow::Result;
use common::report::ServiceKind;
use framework::{run_all, Mode};
use std::collections::HashSet;

#[tokio::test]
async fn framework_e2e_suite() -> Result<()> {
    let report = run_all(Mode::Generic, Some(HashSet::from([ServiceKind::E2e])), None).await?;
    anyhow::ensure!(
        !report.has_failures(),
        "framework E2E failed:\n{}",
        report.to_table()
    );
    Ok(())
}
