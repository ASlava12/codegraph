//! Deterministic Tree-sitter parsing for CodeGraph: language detection,
//! syntax-level fact extraction, and the parsed-item model consumed by
//! the indexer. Modules map to feature areas: [`language`] (adapters and
//! grammars), [`model`] (parsed facts), [`extract`] (tree walking and
//! classification), [`effects`] (env/config/error detection), and
//! [`text`] (node text utilities).

mod effects;
mod extract;
mod language;
mod model;
mod text;

#[cfg(test)]
mod tests;

pub use extract::parse_source;
pub use language::{
    Language, LanguageAdapter, LanguageAdapterInfo, adapter_for_language, adapter_for_path,
    language_adapters,
};
pub use model::{ParseError, ParsedFile, ParsedItem, ParsedItemKind};

pub(crate) use effects::*;
pub(crate) use extract::*;
pub(crate) use model::parsed_item_kind_label;
pub(crate) use text::*;
