use serde::Serialize;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Serialize)]
pub enum ComplianceLevel {
    Level0,
    Level1,
    Level2,
    Level3,
    Level4,
    Level5,
}

impl ComplianceLevel {
    pub fn as_int(self) -> u8 {
        match self {
            ComplianceLevel::Level0 => 0,
            ComplianceLevel::Level1 => 1,
            ComplianceLevel::Level2 => 2,
            ComplianceLevel::Level3 => 3,
            ComplianceLevel::Level4 => 4,
            ComplianceLevel::Level5 => 5,
        }
    }

    fn all() -> [ComplianceLevel; 6] {
        [
            ComplianceLevel::Level0,
            ComplianceLevel::Level1,
            ComplianceLevel::Level2,
            ComplianceLevel::Level3,
            ComplianceLevel::Level4,
            ComplianceLevel::Level5,
        ]
    }
}

impl fmt::Display for ComplianceLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Level {}", self.as_int())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum ServiceKind {
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

impl ServiceKind {
    /// Canonical report order (WES, TES, DRS, …).
    pub fn canonical_order(self) -> u8 {
        match self {
            ServiceKind::Wes => 0,
            ServiceKind::Tes => 1,
            ServiceKind::Drs => 2,
            ServiceKind::Trs => 3,
            ServiceKind::Beacon => 4,
            ServiceKind::Htsget => 5,
            ServiceKind::Auth => 6,
            ServiceKind::Age => 7,
            ServiceKind::Crypt4gh => 8,
            ServiceKind::E2e => 9,
            ServiceKind::Africa => 10,
            ServiceKind::Infra => 11,
        }
    }
}

impl fmt::Display for ServiceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ServiceKind::Wes => "WES",
            ServiceKind::Tes => "TES",
            ServiceKind::Drs => "DRS",
            ServiceKind::Trs => "TRS",
            ServiceKind::Beacon => "Beacon",
            ServiceKind::Htsget => "htsget",
            ServiceKind::Auth => "Auth",
            ServiceKind::Age => "Age",
            ServiceKind::Crypt4gh => "Crypt4GH",
            ServiceKind::E2e => "E2E",
            ServiceKind::Africa => "Africa",
            ServiceKind::Infra => "Infra",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
pub enum TestCategory {
    Schema,
    Lifecycle,
    WorkflowCorrectness,
    Checksum,
    Interoperability,
    Security,
    Robustness,
    #[default]
    Other,
}

impl fmt::Display for TestCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            TestCategory::Schema => "schema",
            TestCategory::Lifecycle => "lifecycle",
            TestCategory::WorkflowCorrectness => "workflow_correctness",
            TestCategory::Checksum => "checksum",
            TestCategory::Interoperability => "interoperability",
            TestCategory::Security => "security",
            TestCategory::Robustness => "robustness",
            TestCategory::Other => "other",
        };
        write!(f, "{}", s)
    }
}

/// Outcome of a single check. Skip is excluded from levels, scores, and `--fail-level`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TestStatus {
    #[default]
    Pass,
    Fail,
    Skip,
}

#[derive(Debug, Clone, Serialize)]
pub struct TestCaseResult {
    pub name: String,
    pub level: ComplianceLevel,
    pub status: TestStatus,
    /// `true` iff `status == Pass`. Prefer `status` for new consumers; skips are not passes.
    pub passed: bool,
    pub error: Option<String>,
    pub category: TestCategory,
    /// Relative importance. `<= 0` is omitted from `weighted_score` (skips use 0).
    pub weight: f32,
}

impl TestCaseResult {
    pub fn pass(name: impl Into<String>, level: ComplianceLevel, category: TestCategory) -> Self {
        Self {
            name: name.into(),
            level,
            status: TestStatus::Pass,
            passed: true,
            error: None,
            category,
            weight: 1.0,
        }
    }

    pub fn fail(
        name: impl Into<String>,
        level: ComplianceLevel,
        category: TestCategory,
        err: impl fmt::Display,
    ) -> Self {
        Self {
            name: name.into(),
            level,
            status: TestStatus::Fail,
            passed: false,
            error: Some(err.to_string()),
            category,
            weight: 1.0,
        }
    }

    pub fn skip(
        name: impl Into<String>,
        level: ComplianceLevel,
        category: TestCategory,
        reason: impl fmt::Display,
    ) -> Self {
        let reason = reason.to_string();
        let error = if reason.starts_with("skipped:") {
            reason
        } else {
            format!("skipped: {reason}")
        };
        Self {
            name: name.into(),
            level,
            status: TestStatus::Skip,
            passed: false,
            error: Some(error),
            category,
            weight: 0.0,
        }
    }

    pub fn from_outcome(
        name: impl Into<String>,
        level: ComplianceLevel,
        category: TestCategory,
        result: Result<(), impl fmt::Display>,
    ) -> Self {
        match result {
            Ok(()) => Self::pass(name, level, category),
            Err(e) => Self::fail(name, level, category, e),
        }
    }

    fn is_executed(&self) -> bool {
        self.status != TestStatus::Skip
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ServiceReport {
    pub service: ServiceKind,
    pub tests: Vec<TestCaseResult>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkippedService {
    pub service: ServiceKind,
    pub reason: String,
}

impl ServiceReport {
    /// Highest N such that Level 0 was executed and passed, and every executed
    /// (non-skip) test at each higher level that has tests also passed.
    /// Empty or skip-only levels in between do not block a higher N.
    /// If the service ran tests but none at Level 0, the achieved level is 0
    /// (a Level-5-only suite does not claim Level 5).
    pub fn achieved_level(&self) -> ComplianceLevel {
        let l0: Vec<&TestCaseResult> = self
            .tests
            .iter()
            .filter(|t| t.level == ComplianceLevel::Level0 && t.is_executed())
            .collect();
        if l0.is_empty() || l0.iter().any(|t| t.status != TestStatus::Pass) {
            return ComplianceLevel::Level0;
        }
        let mut max_level = ComplianceLevel::Level0;
        for lvl in ComplianceLevel::all()
            .into_iter()
            .filter(|l| *l != ComplianceLevel::Level0)
        {
            let executed: Vec<&TestCaseResult> = self
                .tests
                .iter()
                .filter(|t| t.level == lvl && t.is_executed())
                .collect();
            if executed.is_empty() {
                continue;
            }
            if executed.iter().all(|t| t.status == TestStatus::Pass) {
                max_level = lvl;
            } else {
                break;
            }
        }
        max_level
    }

    /// Weighted score in [0.0, 1.0]. Skips and `weight <= 0` are omitted.
    pub fn weighted_score(&self) -> f32 {
        let mut total_weight = 0.0_f32;
        let mut passed_weight = 0.0_f32;
        for t in &self.tests {
            if t.status == TestStatus::Skip || t.weight <= 0.0 {
                continue;
            }
            total_weight += t.weight;
            if t.status == TestStatus::Pass {
                passed_weight += t.weight;
            }
        }
        if total_weight == 0.0 {
            0.0
        } else {
            passed_weight / total_weight
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ServiceScoreSummary {
    pub service: ServiceKind,
    pub level: u8,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct OverallScoreSummary {
    pub services: Vec<ServiceScoreSummary>,
    pub overall_level: u8,
    pub overall_score: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CoverageState {
    Pass,
    Fail,
    Missing,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServiceCoverageSummary {
    pub service: ServiceKind,
    pub categories: Vec<(TestCategory, CoverageState)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OverallCoverageSummary {
    pub services: Vec<ServiceCoverageSummary>,
}

/// Optional run diagnostics for JSON consumers. **Never** used for compliance levels or scores.
#[derive(Debug, Clone, Serialize)]
pub struct ReportDiagnostics {
    /// Wall-clock duration of the CLI conformance run (milliseconds).
    pub suite_duration_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// `true` if `HELIXTEST_REPORT_DIAGNOSTICS` is `1` or `true` (case-insensitive).
pub fn report_diagnostics_requested() -> bool {
    std::env::var("HELIXTEST_REPORT_DIAGNOSTICS")
        .map(|v| {
            let t = v.trim();
            t.eq_ignore_ascii_case("true") || t == "1"
        })
        .unwrap_or(false)
}

#[derive(Debug, Clone, Serialize)]
pub struct OverallReport {
    pub services: Vec<ServiceReport>,
    #[serde(default)]
    pub enabled_services: Vec<ServiceKind>,
    #[serde(default)]
    pub skipped_services: Vec<SkippedService>,
    #[serde(default)]
    pub executed_test_modules: Vec<ServiceKind>,
    /// Present only when `HELIXTEST_REPORT_DIAGNOSTICS` is enabled; omitted from JSON otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<ReportDiagnostics>,
}

impl OverallReport {
    /// Sort services into canonical order (WES, TES, DRS, …) for deterministic table/JSON output.
    pub fn sort_services_canonical(&mut self) {
        self.services.sort_by_key(|s| s.service.canonical_order());
    }

    pub fn overall_level(&self) -> ComplianceLevel {
        self.services
            .iter()
            .filter(|s| s.tests.iter().any(|t| t.is_executed()))
            .map(|s| s.achieved_level())
            .min()
            .unwrap_or(ComplianceLevel::Level0)
    }

    pub fn has_failures(&self) -> bool {
        self.services
            .iter()
            .flat_map(|s| &s.tests)
            .any(|t| t.status == TestStatus::Fail)
    }

    pub fn to_table(&self) -> String {
        let mut out = String::new();
        if !self.enabled_services.is_empty() {
            let enabled = self
                .enabled_services
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("Enabled services: {}\n", enabled));
        }
        if !self.skipped_services.is_empty() {
            let skipped = self
                .skipped_services
                .iter()
                .map(|s| format!("{} ({})", s.service, s.reason))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("Skipped services: {}\n", skipped));
        }
        if !self.executed_test_modules.is_empty() {
            let executed = self
                .executed_test_modules
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("Executed modules: {}\n\n", executed));
        }
        out.push_str("Service   Level   Details\n");
        out.push_str("=======   =====   =======\n");
        let mut services: Vec<_> = self.services.iter().collect();
        services.sort_by_key(|s| s.service.canonical_order());
        for s in services {
            let lvl = s.achieved_level();
            let mut details = Vec::new();
            for t in &s.tests {
                match t.status {
                    TestStatus::Fail => details.push(format!(
                        "{}: {}",
                        t.name,
                        t.error.as_deref().unwrap_or("failed")
                    )),
                    TestStatus::Skip => details.push(format!(
                        "SKIP {}: {}",
                        t.name,
                        t.error.as_deref().unwrap_or("skipped")
                    )),
                    TestStatus::Pass => {}
                }
            }
            if details.is_empty() {
                details.push("OK".to_string());
            }
            out.push_str(&format!(
                "{:<8} {:<7} {}\n",
                s.service,
                lvl.as_int(),
                details.join(" | ")
            ));
        }
        out
    }

    /// Return a numeric scoring summary per service and overall (deterministic order).
    pub fn score_summary(&self) -> OverallScoreSummary {
        let mut summaries = Vec::new();
        let mut total_score = 0.0_f32;
        let mut count = 0_u32;
        let mut services: Vec<_> = self.services.iter().collect();
        services.sort_by_key(|s| s.service.canonical_order());

        for s in services {
            let lvl = s.achieved_level().as_int();
            let score = s.weighted_score();
            summaries.push(ServiceScoreSummary {
                service: s.service,
                level: lvl,
                score,
            });
            total_score += score;
            count += 1;
        }

        let overall_level = self.overall_level().as_int();
        let overall_score = if count == 0 {
            0.0
        } else {
            total_score / (count as f32)
        };

        OverallScoreSummary {
            services: summaries,
            overall_level,
            overall_score,
        }
    }

    /// Coverage matrix per service and category (skips count as Missing when they are the only tests).
    pub fn coverage_summary(&self) -> OverallCoverageSummary {
        let all_categories = [
            TestCategory::Schema,
            TestCategory::Lifecycle,
            TestCategory::WorkflowCorrectness,
            TestCategory::Checksum,
            TestCategory::Interoperability,
            TestCategory::Security,
            TestCategory::Robustness,
            TestCategory::Other,
        ];

        let mut sorted: Vec<_> = self.services.iter().collect();
        sorted.sort_by_key(|s| s.service.canonical_order());
        let mut services = Vec::new();
        for s in sorted {
            let mut cats = Vec::new();
            for cat in &all_categories {
                let executed: Vec<&TestCaseResult> = s
                    .tests
                    .iter()
                    .filter(|t| t.category == *cat && t.is_executed())
                    .collect();
                let state = if executed.is_empty() {
                    CoverageState::Missing
                } else if executed.iter().all(|t| t.status == TestStatus::Pass) {
                    CoverageState::Pass
                } else {
                    CoverageState::Fail
                };
                cats.push((*cat, state));
            }
            services.push(ServiceCoverageSummary {
                service: s.service,
                categories: cats,
            });
        }

        OverallCoverageSummary { services }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case(
        name: &str,
        level: ComplianceLevel,
        status: TestStatus,
        category: TestCategory,
    ) -> TestCaseResult {
        match status {
            TestStatus::Pass => TestCaseResult::pass(name, level, category),
            TestStatus::Fail => TestCaseResult::fail(name, level, category, "failed"),
            TestStatus::Skip => TestCaseResult::skip(name, level, category, "not configured"),
        }
    }

    #[test]
    fn table_includes_subset_metadata_sections() {
        let report = OverallReport {
            services: vec![],
            enabled_services: vec![ServiceKind::Wes, ServiceKind::Drs],
            skipped_services: vec![SkippedService {
                service: ServiceKind::Tes,
                reason: "skipped by profile".to_string(),
            }],
            executed_test_modules: vec![ServiceKind::Wes, ServiceKind::Drs],
            diagnostics: None,
        };
        let table = report.to_table();
        assert!(table.contains("Enabled services:"));
        assert!(table.contains("Skipped services:"));
        assert!(table.contains("Executed modules:"));
    }

    #[test]
    fn skip_does_not_count_as_pass_or_fail() {
        let report = ServiceReport {
            service: ServiceKind::Beacon,
            tests: vec![
                case(
                    "L0",
                    ComplianceLevel::Level0,
                    TestStatus::Pass,
                    TestCategory::Other,
                ),
                case(
                    "L2 skipped",
                    ComplianceLevel::Level2,
                    TestStatus::Skip,
                    TestCategory::Interoperability,
                ),
            ],
        };
        assert_eq!(report.achieved_level(), ComplianceLevel::Level0);
        assert!((report.weighted_score() - 1.0).abs() < f32::EPSILON);
        let overall = OverallReport {
            services: vec![report],
            enabled_services: vec![],
            skipped_services: vec![],
            executed_test_modules: vec![],
            diagnostics: None,
        };
        assert!(!overall.has_failures());
        assert!(overall.to_table().contains("SKIP"));
        assert!(!overall.to_table().contains("L2 skipped: failed"));
    }

    #[test]
    fn skip_weight_zero_is_omitted_from_score() {
        let report = ServiceReport {
            service: ServiceKind::Drs,
            tests: vec![
                TestCaseResult::pass("checksum", ComplianceLevel::Level2, TestCategory::Checksum),
                TestCaseResult::skip(
                    "optional",
                    ComplianceLevel::Level2,
                    TestCategory::Checksum,
                    "feature off",
                ),
            ],
        };
        assert!((report.weighted_score() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn fail_at_level_stops_climb() {
        let report = ServiceReport {
            service: ServiceKind::Wes,
            tests: vec![
                case(
                    "L0",
                    ComplianceLevel::Level0,
                    TestStatus::Pass,
                    TestCategory::Other,
                ),
                case(
                    "L1",
                    ComplianceLevel::Level1,
                    TestStatus::Pass,
                    TestCategory::Schema,
                ),
                case(
                    "L2",
                    ComplianceLevel::Level2,
                    TestStatus::Fail,
                    TestCategory::Lifecycle,
                ),
            ],
        };
        assert_eq!(report.achieved_level(), ComplianceLevel::Level1);
        assert!(OverallReport {
            services: vec![report],
            enabled_services: vec![],
            skipped_services: vec![],
            executed_test_modules: vec![],
            diagnostics: None,
        }
        .has_failures());
    }

    #[test]
    fn skip_only_service_does_not_pin_overall_level() {
        let skipped = ServiceReport {
            service: ServiceKind::Htsget,
            tests: vec![TestCaseResult::skip(
                "htsget suite",
                ComplianceLevel::Level0,
                TestCategory::Other,
                "no URL",
            )],
        };
        let wes = ServiceReport {
            service: ServiceKind::Wes,
            tests: vec![
                case(
                    "L0",
                    ComplianceLevel::Level0,
                    TestStatus::Pass,
                    TestCategory::Other,
                ),
                case(
                    "L2",
                    ComplianceLevel::Level2,
                    TestStatus::Pass,
                    TestCategory::Lifecycle,
                ),
            ],
        };
        let overall = OverallReport {
            services: vec![skipped, wes],
            enabled_services: vec![],
            skipped_services: vec![],
            executed_test_modules: vec![],
            diagnostics: None,
        };
        assert_eq!(overall.overall_level(), ComplianceLevel::Level2);
    }

    #[test]
    fn coverage_treats_skip_only_category_as_missing() {
        let report = ServiceReport {
            service: ServiceKind::Beacon,
            tests: vec![TestCaseResult::skip(
                "v2",
                ComplianceLevel::Level2,
                TestCategory::Interoperability,
                "disabled",
            )],
        };
        let overall = OverallReport {
            services: vec![report],
            enabled_services: vec![],
            skipped_services: vec![],
            executed_test_modules: vec![],
            diagnostics: None,
        };
        let cov = overall.coverage_summary();
        let interop = cov.services[0]
            .categories
            .iter()
            .find(|(c, _)| *c == TestCategory::Interoperability)
            .unwrap();
        assert_eq!(interop.1, CoverageState::Missing);
    }

    #[test]
    fn only_l5_without_l0_is_level_0() {
        let report = ServiceReport {
            service: ServiceKind::Crypt4gh,
            tests: vec![case(
                "age roundtrip",
                ComplianceLevel::Level5,
                TestStatus::Pass,
                TestCategory::Robustness,
            )],
        };
        assert_eq!(report.achieved_level(), ComplianceLevel::Level0);
    }
}
