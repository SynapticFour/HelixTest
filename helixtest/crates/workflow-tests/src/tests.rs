// SPDX-License-Identifier: Apache-2.0
use anyhow::Result;
use common::report::ServiceKind;
use framework::{run_all, Mode};
use std::collections::HashSet;

#[tokio::test]
async fn framework_wes_workflows() -> Result<()> {
    let report = run_all(Mode::Generic, Some(HashSet::from([ServiceKind::Wes])), None).await?;
    anyhow::ensure!(
        !report.has_failures(),
        "framework WES/workflow failed:\n{}",
        report.to_table()
    );
    Ok(())
}
