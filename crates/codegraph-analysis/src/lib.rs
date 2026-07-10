//! Graph analysis for CodeGraph: queries, traces, workflows, journeys,
//! refactoring reports, insights, project reports, and exports over the
//! typed code graph. Modules map to feature areas; see each module's docs.

pub mod mcp;
pub mod memory;
pub mod pr_impact;

mod ask;
mod cards;
mod exports;
mod flows;
mod insights;
mod limits;
mod model;
mod overview;
mod query;
mod refactoring;
mod report;
mod slices;
mod source_search;
mod support;
mod traces;

#[cfg(test)]
mod tests;

pub use mcp::{MCP_PROTOCOL_VERSION, McpEngine, McpToolAudit, mcp_tool_definitions};

pub use ask::*;
pub use cards::*;
pub use exports::*;
pub use flows::*;
pub use insights::*;
pub use limits::*;
pub use model::*;
pub use overview::*;
pub use query::*;
pub use refactoring::*;
pub use report::*;
pub use slices::*;
pub use source_search::*;
pub(crate) use support::*;
pub use traces::*;
