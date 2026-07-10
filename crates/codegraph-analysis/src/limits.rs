//! Known insight kinds and published report/preview limit constants.

use codegraph_core::{Node, NodeId};
use serde::{Deserialize, Serialize};

#[allow(unused_imports)]
use crate::*;

pub const KNOWN_INSIGHT_KINDS: &[&str] = &[
    "ambiguous_call_resolution",
    "ambiguous_entrypoint_target",
    "conflicting_config_default",
    "conflicting_dependency_declaration",
    "cross_language_heuristic_edge",
    "custom_rule_*",
    "dependency_cycle",
    "duplicate_compose_published_port",
    "duplicate_entrypoint_label",
    "duplicate_framework_route",
    "duplicate_function_label",
    "duplicate_migration_sequence",
    "entrypoint_dead_end",
    "mixed_dependency_scope",
    "mixed_config_requirement",
    "non_runtime_dependency_import",
    "orphan_function",
    "parse_error",
    "potential_error_flow",
    "rationale_risk_comment",
    "semantic_diagnostic",
    "sensitive_ci_environment_literal",
    "sensitive_config_default",
    "skipped_large_file",
    "syntax_error",
    "test_only_runtime_dependency",
    "unmatched_platform_channel",
    "undeclared_flutter_asset",
    "undeclared_external_import",
    "unreachable_config_read",
    "unreachable_error_flow",
    "unreachable_source_file",
    "unresolved_call",
    "unresolved_compose_command_path",
    "unresolved_compose_env_file_path",
    "unresolved_compose_volume_source_path",
    "unresolved_dockerfile_command_path",
    "unresolved_entrypoint_target",
    "unresolved_framework_route_handler",
    "unresolved_github_actions_job_need",
    "unresolved_github_actions_local_action",
    "unresolved_github_actions_run_path",
    "unresolved_gitlab_ci_job_dependency",
    "unresolved_gitlab_ci_script_path",
    "unresolved_kubernetes_config_ref",
    "unresolved_kubernetes_ingress_backend",
    "unresolved_kubernetes_service_selector",
    "unresolved_local_import",
    "unresolved_makefile_command_path",
    "unresolved_sql_alter_target",
    "unresolved_sql_table_reference",
    "unused_declared_dependency",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Insight {
    pub kind: String,
    pub severity: InsightSeverity,
    pub message: String,
    pub nodes: Vec<NodeId>,
    pub edges: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckReport {
    pub passed: bool,
    pub fail_on: String,
    pub failing_insights: usize,
    pub report: InsightReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRiskSummary {
    pub score: usize,
    pub grade: String,
    pub total: usize,
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
    pub top_kinds: Vec<ProjectRiskKindSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRiskKindSummary {
    pub kind: String,
    pub severity: String,
    pub count: usize,
}

/// Maximum insight sample embedded in the report `quality_gate` payload.
/// Gate evaluation stays limit-independent; only the embedded list is capped.
pub const REPORT_QUALITY_GATE_SAMPLE_LIMIT: usize = 25;
pub const DEFAULT_REPORT_ARCHITECTURE_GROUP_LIMIT: usize = 50;
pub const MAX_REPORT_ARCHITECTURE_GROUP_LIMIT: usize = 500;
pub const DEFAULT_REPORT_ARCHITECTURE_EDGE_LIMIT: usize = 200;
pub const MAX_REPORT_ARCHITECTURE_EDGE_LIMIT: usize = 2_000;
pub const DEFAULT_REPORT_LANGUAGE_LINK_LIMIT: usize = 50;
pub const MAX_REPORT_LANGUAGE_LINK_LIMIT: usize = 500;
pub const DEFAULT_REPORT_HOTSPOT_LIMIT: usize = 25;
pub const MAX_REPORT_HOTSPOT_LIMIT: usize = 500;
pub const DEFAULT_REPORT_COMMUNITY_LIMIT: usize = 25;
pub const MAX_REPORT_COMMUNITY_LIMIT: usize = 500;
pub const DEFAULT_REPORT_INSIGHT_LIMIT: usize = 50;
pub const MAX_REPORT_INSIGHT_LIMIT: usize = 500;
pub const DEFAULT_REPORT_FILE_SUMMARY_LIMIT: usize = 25;
pub const MAX_REPORT_FILE_SUMMARY_LIMIT: usize = 500;
pub const DEFAULT_REPORT_NODE_SUMMARY_LIMIT: usize = 25;
pub const MAX_REPORT_NODE_SUMMARY_LIMIT: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectReportLimits {
    pub architecture_group_limit: usize,
    pub architecture_edge_limit: usize,
    pub language_link_limit: usize,
    pub hotspot_limit: usize,
    pub community_limit: usize,
    pub insight_limit: usize,
    pub file_summary_limit: usize,
    pub node_summary_limit: usize,
    pub fail_on: InsightSeverity,
}

impl Default for ProjectReportLimits {
    fn default() -> Self {
        Self {
            architecture_group_limit: DEFAULT_REPORT_ARCHITECTURE_GROUP_LIMIT,
            architecture_edge_limit: DEFAULT_REPORT_ARCHITECTURE_EDGE_LIMIT,
            language_link_limit: DEFAULT_REPORT_LANGUAGE_LINK_LIMIT,
            hotspot_limit: DEFAULT_REPORT_HOTSPOT_LIMIT,
            community_limit: DEFAULT_REPORT_COMMUNITY_LIMIT,
            insight_limit: DEFAULT_REPORT_INSIGHT_LIMIT,
            file_summary_limit: DEFAULT_REPORT_FILE_SUMMARY_LIMIT,
            node_summary_limit: DEFAULT_REPORT_NODE_SUMMARY_LIMIT,
            fail_on: InsightSeverity::Error,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectCompactFileSummaryReport {
    pub files: Vec<ProjectCompactFileSummary>,
    pub total_files: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectCompactFileSummary {
    pub node: Node,
    pub summary: FileNodeSummary,
    pub insight_summary: NodeInsightSummary,
    pub score: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectCompactNodeSummaryReport {
    pub nodes: Vec<ProjectCompactNodeSummary>,
    pub total_nodes: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectCompactNodeSummary {
    pub node: Node,
    pub dependency_summary: NodeDependencySummary,
    pub insight_summary: NodeInsightSummary,
    pub roles: Vec<String>,
    pub score: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectReport {
    pub graph_schema_version: u32,
    pub summary: GraphSummary,
    pub entrypoints: Vec<Node>,
    pub insights: InsightReport,
    pub risk_summary: ProjectRiskSummary,
    pub quality_gate: CheckReport,
    pub architecture: ArchitectureMap,
    pub language_dependencies: LanguageDependencyReport,
    pub surprising_links: SurprisingLinkReport,
    pub hotspots: HotspotReport,
    pub communities: CommunityReport,
    pub file_summaries: ProjectCompactFileSummaryReport,
    pub node_summaries: ProjectCompactNodeSummaryReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectReportMarkdownOptions {
    pub title: String,
    pub root: Option<String>,
    pub generated_at_unix: Option<u64>,
}

impl Default for ProjectReportMarkdownOptions {
    fn default() -> Self {
        Self {
            title: "CodeGraph Project Report".to_string(),
            root: None,
            generated_at_unix: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InsightSeverity {
    Info,
    Warning,
    Error,
}

impl Default for InsightFilter {
    fn default() -> Self {
        Self {
            severity: None,
            kind: None,
            search: None,
            limit: 50,
        }
    }
}
