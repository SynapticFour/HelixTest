// SPDX-License-Identifier: Apache-2.0
//! HTTP Level-0 reachable classification used by the DRS checker.
//! Not a GA4GH MUST. Identity-hashed: changing this changes executed_checker_id.

use common::report::{ComplianceLevel, TestCaseResult, TestCategory};
use common::util::level0_reachable_ok;

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
