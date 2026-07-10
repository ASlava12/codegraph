//! Runtime limit constants, embedded web assets, and server version
//! metadata shared across handlers and the API schema.

use std::env;
use std::sync::atomic::AtomicU64;

#[allow(unused_imports)]
use crate::*;

pub(crate) const DEFAULT_MAX_SCAN_JOBS: usize = 64;
pub(crate) const DEFAULT_MAX_SEMANTIC_JOBS: usize = 64;
pub(crate) const DEFAULT_MAX_SCAN_CONCURRENCY: usize = 2;
pub(crate) const DEFAULT_MAX_SEMANTIC_CONCURRENCY: usize = 1;
pub(crate) const DEFAULT_JOB_LIST_LIMIT: usize = 50;
pub(crate) const MAX_JOB_LIST_LIMIT: usize = 500;
pub(crate) const DEFAULT_INCREMENTAL_REPORT_LIMIT: usize = 100;
pub(crate) const MAX_INCREMENTAL_REPORT_LIMIT: usize = 10_000;
pub(crate) const DEFAULT_GRAPH_NODE_LIMIT: usize = 250;
pub(crate) const MAX_GRAPH_NODE_LIMIT: usize = 1000;
pub(crate) const DEFAULT_GRAPH_EDGE_LIMIT: usize = 500;
pub(crate) const MAX_GRAPH_EDGE_LIMIT: usize = 2000;
pub(crate) const DEFAULT_NODE_CONTEXT_EDGE_LIMIT: usize = 80;
pub(crate) const MAX_NODE_CONTEXT_EDGE_LIMIT: usize = 500;
pub(crate) const DEFAULT_NODE_CARD_SOURCE_CONTEXT: u32 = 5;
pub(crate) const MAX_NODE_CARD_SOURCE_CONTEXT: u32 = 40;
pub(crate) const DEFAULT_NODE_CARD_INSIGHT_LIMIT: usize = 8;
pub(crate) const MAX_NODE_CARD_INSIGHT_LIMIT: usize = 500;
pub(crate) const DEFAULT_FOCUS_EDGE_LIMIT: usize = 200;
pub(crate) const MAX_FOCUS_EDGE_LIMIT: usize = 1000;
pub(crate) const DEFAULT_GRAPH_QUERY_LIMIT: usize = 100;
pub(crate) const MAX_GRAPH_QUERY_LIMIT: usize = 1000;
pub(crate) const MAX_GRAPH_QUERY_LENGTH: usize = 4096;
pub(crate) const DEFAULT_INSIGHT_LIMIT: usize = 50;
pub(crate) const MAX_INSIGHT_LIMIT: usize = 500;
pub(crate) const DEFAULT_SOURCE_CONTEXT: u32 = 4;
pub(crate) const MAX_SOURCE_CONTEXT: u32 = 40;
pub(crate) const DEFAULT_SOURCE_SEARCH_LIMIT: usize = 50;
pub(crate) const MAX_SOURCE_SEARCH_LIMIT: usize = 1000;
pub(crate) const MAX_SOURCE_SEARCH_QUERY_LENGTH: usize = 4096;
pub(crate) const DEFAULT_SOURCE_SEARCH_CONTEXT: usize = 2;
pub(crate) const MAX_SOURCE_SEARCH_CONTEXT: usize = 20;
pub(crate) const DEFAULT_API_BODY_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const MAX_API_BODY_BYTES: usize = 256 * 1024 * 1024;
pub(crate) const EXPORT_NODES_HEADER: &str = "x-codegraph-export-nodes";
pub(crate) const EXPORT_EDGES_HEADER: &str = "x-codegraph-export-edges";
pub(crate) const EXPORT_BYTES_HEADER: &str = "x-codegraph-export-bytes";
pub(crate) const RESPONSE_TIME_HEADER: &str = "x-response-time-ms";
pub(crate) const STATIC_ASSET_CACHE_CONTROL: &str = "no-cache";
pub(crate) const DYNAMIC_CACHE_CONTROL: &str = "no-store";
pub(crate) const APP_JS: &str = include_str!("../../codegraph-web/static/app.js");
pub(crate) const INDEX_HTML: &str = include_str!("../../codegraph-web/static/index.html");
pub(crate) const LABEL_POLICY_JS: &str = include_str!("../../codegraph-web/static/label-policy.js");
pub(crate) const STYLES_CSS: &str = include_str!("../../codegraph-web/static/styles.css");
pub(crate) const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
pub(crate) static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

tokio::task_local! {
    pub(crate) static CURRENT_REQUEST_ID: String;
}
