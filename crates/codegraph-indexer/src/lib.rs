//! Deterministic project scanning for CodeGraph: walk a repository,
//! parse source files, and assemble the typed code graph from syntax
//! facts, manifests, runtime surfaces, documents, SQL, and user rules.
//! Modules map to feature areas; see each module's docs.

mod context;
mod detectors;
mod docs_ingest;
mod frameworks;
mod imports;
mod knowledge;
mod manifests;
mod options;
mod parse_cache;
mod resolve;
mod rules;
mod runtime;
mod scan;
mod sql;
mod walk;

#[cfg(test)]
mod tests;

pub use options::{
    DEFAULT_MAX_FILE_SIZE, IndexError, IndexOptionOverrides, IndexOptions, ScanCoverageReport,
    compile_ignored_globs, configured_index_options,
};
pub use resolve::RESOLUTION_BASES;
pub use scan::{
    ScanCancellation, scan_coverage, scan_project, scan_project_cancelable, scan_project_paths,
};
pub use walk::{is_index_relevant_file, should_enter};

pub(crate) use context::*;
pub(crate) use detectors::*;
pub(crate) use docs_ingest::*;
pub(crate) use frameworks::*;
pub(crate) use imports::*;
pub(crate) use knowledge::*;
pub(crate) use manifests::*;
pub(crate) use options::*;
pub(crate) use parse_cache::*;
pub(crate) use resolve::*;
pub(crate) use rules::*;
pub(crate) use runtime::*;
pub(crate) use sql::*;
pub(crate) use walk::*;
