use anyhow::Result;
use common::report::ServiceKind;
use framework::{run_all, Mode};
use std::collections::HashSet;

#[tokio::test]
async fn framework_wes_drs_trs_contract() -> Result<()> {
    let report = run_all(
        Mode::Generic,
        Some(HashSet::from([
            ServiceKind::Wes,
            ServiceKind::Drs,
            ServiceKind::Trs,
        ])),
        None,
    )
    .await?;
    anyhow::ensure!(
        !report.has_failures(),
        "framework API subset failed:\n{}",
        report.to_table()
    );
    Ok(())
}
