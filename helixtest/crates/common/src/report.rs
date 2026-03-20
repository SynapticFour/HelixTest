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
    Crypt4gh,
    E2e,
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
            ServiceKind::Crypt4gh => "Crypt4GH",
            ServiceKind::E2e => "E2E",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum TestCategory {
    Schema,
    Lifecycle,
    WorkflowCorrectness,
    Checksum,
    Interoperability,
    Security,
    Robustness,
    Other,
}

impl Default for TestCategory {
    fn default() -> Self {
        TestCategory::Other
    }
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

#[allow(dead_code)] // used by serde default
fn default_weight() -> f32 {
    1.0
}

#[derive(Debug, Clone, Serialize)]
pub struct TestCaseResult {
    pub name: String,
    pub level: ComplianceLevel,
    pub passed: bool,
    pub error: Option<String>,
    /// Category of the test (used for coverage/scoring reports)
    #[serde(default)]
    pub category: TestCategory,
    /// Relative importance weight (default 1.0; critical tests can use >1.0)
    #[serde(default = "default_weight")]
    pub weight: f32,
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

/// Canonical order for deterministic report output (table, JSON).
fn service_order(s: &ServiceKind) -> u8 {
    match s {
        ServiceKind::Wes => 0,
        ServiceKind::Tes => 1,
        ServiceKind::Drs => 2,
        ServiceKind::Trs => 3,
        ServiceKind::Beacon => 4,
        ServiceKind::Htsget => 5,
        ServiceKind::Auth => 6,
        ServiceKind::Crypt4gh => 7,
        ServiceKind::E2e => 8,
    }
}

impl ServiceReport {
    pub fn achieved_level(&self) -> ComplianceLevel {
        // A service's level is the max level where all tests up to that level passed.
        let mut max_level = ComplianceLevel::Level0;
        for lvl in [
            ComplianceLevel::Level0,
            ComplianceLevel::Level1,
            ComplianceLevel::Level2,
            ComplianceLevel::Level3,
            ComplianceLevel::Level4,
            ComplianceLevel::Level5,
        ] {
            let any_at_level = self.tests.iter().any(|t| t.level == lvl);
            if any_at_level
                && self
                    .tests
                    .iter()
                    .filter(|t| t.level == lvl)
                    .all(|t| t.passed)
            {
                max_level = lvl;
            } else if any_at_level {
                break;
            }
        }
        max_level
    }
}

impl ServiceReport {
    /// Weighted score in [0.0, 1.0] based on test weights.
    /// 1.0 means all weighted tests for this service passed.
    pub fn weighted_score(&self) -> f32 {
        let mut total_weight = 0.0_f32;
        let mut passed_weight = 0.0_f32;
        for t in &self.tests {
            let w = if t.weight <= 0.0 { 1.0 } else { t.weight };
            total_weight += w;
            if t.passed {
                passed_weight += w;
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

#[derive(Debug, Clone, Serialize)]
pub struct OverallReport {
    pub services: Vec<ServiceReport>,
    #[serde(default)]
    pub enabled_services: Vec<ServiceKind>,
    #[serde(default)]
    pub skipped_services: Vec<SkippedService>,
    #[serde(default)]
    pub executed_test_modules: Vec<ServiceKind>,
}

impl OverallReport {
    /// Sort services into canonical order (WES, TES, DRS, …) for deterministic table/JSON output.
    pub fn sort_services_canonical(&mut self) {
        self.services.sort_by_key(|s| service_order(&s.service));
    }

    pub fn overall_level(&self) -> ComplianceLevel {
        self.services
            .iter()
            .map(|s| s.achieved_level())
            .min()
            .unwrap_or(ComplianceLevel::Level0)
    }

    pub fn has_failures(&self) -> bool {
        self.services
            .iter()
            .flat_map(|s| &s.tests)
            .any(|t| !t.passed)
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
        services.sort_by_key(|s| service_order(&s.service));
        for s in services {
            let lvl = s.achieved_level();
            let mut failures = s
                .tests
                .iter()
                .filter(|t| !t.passed)
                .map(|t| format!("{}: {}", t.name, t.error.as_deref().unwrap_or("failed")))
                .collect::<Vec<_>>();
            if failures.is_empty() {
                failures.push("OK".to_string());
            }
            out.push_str(&format!(
                "{:<8} {:<7} {}\n",
                s.service,
                lvl.as_int(),
                failures.join(" | ")
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
        services.sort_by_key(|s| service_order(&s.service));

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

    /// Return a simple coverage matrix per service and category:
    /// - Pass: at least one test in the category and all of them passed
    /// - Fail: at least one test in the category and at least one failed
    /// - Missing: no tests in the category
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
        sorted.sort_by_key(|s| service_order(&s.service));
        let mut services = Vec::new();
        for s in sorted {
            let mut cats = Vec::new();
            for cat in &all_categories {
                let tests_in_cat: Vec<&TestCaseResult> =
                    s.tests.iter().filter(|t| t.category == *cat).collect();
                let state = if tests_in_cat.is_empty() {
                    CoverageState::Missing
                } else if tests_in_cat.iter().all(|t| t.passed) {
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
        };
        let table = report.to_table();
        assert!(table.contains("Enabled services:"));
        assert!(table.contains("Skipped services:"));
        assert!(table.contains("Executed modules:"));
    }
}
