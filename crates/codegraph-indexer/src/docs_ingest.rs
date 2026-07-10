//! Markdown/ADR/RFC document ingestion: sections, links, front matter,
//! wikilinks, and document-to-code path/symbol references.

use std::collections::BTreeMap;
use std::path::Path;

use codegraph_core::{Confidence, EdgeKind, NodeId, NodeKind};

#[allow(unused_imports)]
use crate::*;

pub(crate) fn index_markdown_document(
    context: &mut IndexContext,
    file_id: NodeId,
    path: &Path,
    label: &str,
    source: &str,
) {
    if !is_markdown_document(path) {
        return;
    }

    let doc_kind = document_kind(path, label);
    add_file_metadata(&mut context.graph, file_id, "item_kind", "document");
    add_file_metadata(&mut context.graph, file_id, "source", "markdown");
    add_file_metadata(
        &mut context.graph,
        file_id,
        "document_kind",
        doc_kind.clone(),
    );
    // Generated Markdown sidecars (`report.pdf.md`, `slides.pptx.md`) carry
    // provenance back to the binary document they were generated from.
    if let Some(sidecar_source) = markdown_sidecar_source(label) {
        add_file_metadata(&mut context.graph, file_id, "generated", "true");
        add_file_metadata(&mut context.graph, file_id, "sidecar_of", &sidecar_source);
    }

    // YAML front matter: title, ownership, status, tags become document
    // metadata so cards and queries can filter docs by owner.
    let (front_matter, front_matter_end) = markdown_front_matter(source);
    for (key, metadata_key) in [
        ("title", "doc_title"),
        ("status", "doc_status"),
        ("tags", "doc_tags"),
        ("date", "doc_date"),
    ] {
        if let Some(value) = front_matter.get(key) {
            add_file_metadata(&mut context.graph, file_id, metadata_key, value);
        }
    }
    for owner_key in ["owner", "owners", "author", "authors", "maintainer"] {
        if let Some(value) = front_matter.get(owner_key) {
            add_file_metadata(&mut context.graph, file_id, "doc_owner", value);
            break;
        }
    }

    let mut current_section = None;
    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        if line_number <= front_matter_end {
            continue;
        }
        if let Some((level, heading)) = markdown_heading(line) {
            let mut metadata = BTreeMap::new();
            metadata.insert("item_kind".to_string(), "document_section".to_string());
            metadata.insert("source".to_string(), "markdown".to_string());
            metadata.insert("language".to_string(), "markdown".to_string());
            metadata.insert("document_kind".to_string(), doc_kind.clone());
            metadata.insert("heading".to_string(), heading.clone());
            metadata.insert("level".to_string(), level.to_string());
            metadata.insert("anchor".to_string(), markdown_anchor(&heading));
            metadata.insert("line".to_string(), line_number.to_string());
            let section_id = context.graph.add_node_with_metadata(
                NodeKind::Module,
                format!("{label}#{heading}"),
                Some(line_span(label, source, line_number)),
                metadata,
            );
            add_edge_once_with_metadata(
                &mut context.graph,
                file_id,
                section_id,
                EdgeKind::Contains,
                Confidence::Exact,
                BTreeMap::from([("relation".to_string(), "document_section".to_string())]),
            );
            current_section = Some(section_id);
        }

        let source_id = current_section.unwrap_or(file_id);
        for link in markdown_links(line) {
            if let Some(candidates) = markdown_path_candidates(label, &link.target) {
                let line_ref = markdown_line_anchor(&link.target);
                context
                    .pending_document_path_refs
                    .push(PendingDocumentPathRef {
                        source: source_id,
                        target: link.target,
                        candidates,
                        relation: "markdown_link",
                        line: line_number,
                        text: Some(link.text),
                        line_ref,
                    });
            }
        }

        for wikilink in markdown_wikilinks(line) {
            let candidates = markdown_wikilink_candidates(label, &wikilink.target);
            if !candidates.is_empty() {
                context
                    .pending_document_path_refs
                    .push(PendingDocumentPathRef {
                        source: source_id,
                        target: wikilink.target,
                        candidates,
                        relation: "markdown_wikilink",
                        line: line_number,
                        text: (!wikilink.text.is_empty()).then_some(wikilink.text),
                        line_ref: None,
                    });
            }
        }

        for code in inline_code_spans(line) {
            if let Some(candidates) = markdown_path_candidates(label, &code) {
                context
                    .pending_document_path_refs
                    .push(PendingDocumentPathRef {
                        source: source_id,
                        target: code,
                        candidates,
                        relation: "markdown_code_path",
                        line: line_number,
                        text: None,
                        line_ref: None,
                    });
            } else if is_document_symbol_reference(&code) {
                context
                    .pending_document_symbol_refs
                    .push(PendingDocumentSymbolRef {
                        source: source_id,
                        symbol: code,
                        relation: "markdown_symbol_reference",
                        line: line_number,
                    });
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MarkdownLink {
    text: String,
    target: String,
}

/// Simple YAML front matter: `key: value` pairs between leading `---`
/// fences. Returns the fields and the 1-based line number of the closing
/// fence (0 when absent).
pub(crate) fn markdown_front_matter(source: &str) -> (BTreeMap<String, String>, u32) {
    let mut lines = source.lines().enumerate();
    match lines.next() {
        Some((_, first)) if first.trim() == "---" => {}
        _ => return (BTreeMap::new(), 0),
    }
    let mut fields = BTreeMap::new();
    for (index, line) in lines {
        if line.trim() == "---" {
            return (fields, index as u32 + 1);
        }
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim().trim_matches(['"', '\'']).to_string();
            if !key.is_empty() && !value.is_empty() && !key.contains(' ') {
                fields.insert(key, value);
            }
        }
    }
    // No closing fence: not front matter.
    (BTreeMap::new(), 0)
}

/// `#L42` or `#L42-L50` fragments on markdown link targets.
pub(crate) fn markdown_line_anchor(target: &str) -> Option<String> {
    let (_, fragment) = target.split_once('#')?;
    let rest = fragment.strip_prefix('L')?;
    let valid = rest
        .split("-L")
        .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()));
    (valid && !rest.is_empty()).then(|| fragment.to_string())
}

/// Obsidian-style `[[target]]` / `[[target|text]]` wikilinks.
pub(crate) fn markdown_wikilinks(line: &str) -> Vec<MarkdownLink> {
    let mut links = Vec::new();
    let mut cursor = 0;
    while let Some(start) = line[cursor..].find("[[") {
        let start = cursor + start + 2;
        let Some(end) = line[start..].find("]]") else {
            break;
        };
        let inner = &line[start..start + end];
        let (target, text) = inner
            .split_once('|')
            .map(|(target, text)| (target.trim(), text.trim()))
            .unwrap_or((inner.trim(), ""));
        if !target.is_empty() && !target.contains("://") {
            links.push(MarkdownLink {
                text: text.to_string(),
                target: target.to_string(),
            });
        }
        cursor = start + end + 2;
    }
    links
}

/// Wikilink targets resolve like Obsidian: same directory first, then the
/// scan root, with `.md` appended when missing.
pub(crate) fn markdown_wikilink_candidates(document_label: &str, target: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let with_extension = if target.to_ascii_lowercase().ends_with(".md") {
        target.to_string()
    } else {
        format!("{target}.md")
    };
    for base in [&with_extension, &target.to_string()] {
        let relative = join_path(path_dir(document_label).as_deref(), base);
        if !relative.is_empty() && !candidates.contains(&relative) {
            candidates.push(relative);
        }
        let root_relative = normalize_path(base);
        if !root_relative.is_empty() && !candidates.contains(&root_relative) {
            candidates.push(root_relative);
        }
    }
    candidates
}

pub(crate) fn markdown_heading(line: &str) -> Option<(u8, String)> {
    let trimmed = line.trim_start();
    let level = trimmed
        .chars()
        .take_while(|character| *character == '#')
        .count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let rest = trimmed[level..].trim_start();
    if rest.is_empty() {
        return None;
    }
    let heading = rest.trim_end_matches('#').trim();
    (!heading.is_empty()).then(|| (level as u8, heading.to_string()))
}

pub(crate) fn markdown_anchor(heading: &str) -> String {
    let mut anchor = String::new();
    let mut previous_dash = false;
    for character in heading.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            anchor.push(character);
            previous_dash = false;
        } else if !previous_dash && !anchor.is_empty() {
            anchor.push('-');
            previous_dash = true;
        }
    }
    anchor.trim_matches('-').to_string()
}

pub(crate) fn markdown_links(line: &str) -> Vec<MarkdownLink> {
    let bytes = line.as_bytes();
    let mut links = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let Some(open_offset) = line[index..].find('[') else {
            break;
        };
        let open = index + open_offset;
        if open > 0 && bytes[open - 1] == b'!' {
            index = open + 1;
            continue;
        }
        let Some(close_offset) = line[open + 1..].find(']') else {
            break;
        };
        let close = open + 1 + close_offset;
        if line[close + 1..].starts_with('(') {
            let target_start = close + 2;
            if let Some(target_offset) = line[target_start..].find(')') {
                let target_end = target_start + target_offset;
                let target = markdown_link_target(&line[target_start..target_end]);
                if !target.is_empty() {
                    links.push(MarkdownLink {
                        text: line[open + 1..close].trim().to_string(),
                        target,
                    });
                }
                index = target_end + 1;
                continue;
            }
        }
        index = close + 1;
    }
    links
}

pub(crate) fn markdown_link_target(raw: &str) -> String {
    raw.trim()
        .trim_matches('<')
        .trim_matches('>')
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim()
        .to_string()
}

pub(crate) fn inline_code_spans(line: &str) -> Vec<String> {
    if line.contains("```") {
        return Vec::new();
    }

    let mut spans = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find('`') {
        let after_start = &rest[start + 1..];
        let Some(end) = after_start.find('`') else {
            break;
        };
        let value = after_start[..end].trim();
        if !value.is_empty() {
            spans.push(value.to_string());
        }
        rest = &after_start[end + 1..];
    }
    spans
}

pub(crate) fn markdown_path_candidates(
    document_label: &str,
    raw_target: &str,
) -> Option<Vec<String>> {
    let target = clean_markdown_path_target(raw_target)?;
    if !is_document_path_reference(&target) {
        return None;
    }

    let mut candidates = Vec::new();
    let relative = join_path(path_dir(document_label).as_deref(), &target);
    if !relative.is_empty() {
        candidates.push(relative);
    }
    let root_relative = normalize_path(&target);
    if !root_relative.is_empty() && !candidates.contains(&root_relative) {
        candidates.push(root_relative);
    }
    (!candidates.is_empty()).then_some(candidates)
}

pub(crate) fn clean_markdown_path_target(raw_target: &str) -> Option<String> {
    let trimmed = raw_target.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('#')
        || trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("mailto:")
        || trimmed.starts_with("git@")
    {
        return None;
    }
    let without_fragment = trimmed
        .split_once('#')
        .map(|(path, _)| path)
        .unwrap_or(trimmed);
    let without_query = without_fragment
        .split_once('?')
        .map(|(path, _)| path)
        .unwrap_or(without_fragment)
        .trim()
        .trim_matches('"')
        .trim_matches('\'');
    (!without_query.is_empty()).then(|| without_query.to_string())
}

pub(crate) fn is_document_path_reference(target: &str) -> bool {
    target.contains('/')
        || target.starts_with("./")
        || target.starts_with("../")
        || target.rsplit('/').next().is_some_and(|name| {
            name.rsplit_once('.').is_some_and(|(_, extension)| {
                matches!(
                    extension.to_ascii_lowercase().as_str(),
                    "rs" | "py"
                        | "js"
                        | "jsx"
                        | "ts"
                        | "tsx"
                        | "go"
                        | "c"
                        | "h"
                        | "cc"
                        | "cpp"
                        | "hpp"
                        | "dart"
                        | "php"
                        | "sh"
                        | "bash"
                        | "md"
                        | "markdown"
                        | "toml"
                        | "json"
                        | "yaml"
                        | "yml"
                        | "lock"
                        | "txt"
                        | "cfg"
                        | "ini"
                        | "env"
                        | "sql"
                )
            })
        })
}

pub(crate) fn is_document_symbol_reference(value: &str) -> bool {
    let value = value.trim().trim_end_matches("()").trim_end_matches('!');
    (3..=96).contains(&value.len())
        && !value.contains(char::is_whitespace)
        && !value.contains('/')
        && !value.starts_with('-')
        && !value.starts_with('$')
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "_:.#<>".contains(character))
        && value.chars().any(|character| {
            character == '_'
                || character == ':'
                || character == '.'
                || character.is_ascii_lowercase()
        })
}

/// Maximum path references extracted from one plain-text document, so a
/// large notes file cannot flood the graph with reference facts.
pub(crate) const PLAIN_TEXT_REFERENCE_LIMIT: usize = 100;

pub(crate) fn is_plain_text_document(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let lower = name.to_ascii_lowercase();
    if !(lower.ends_with(".txt") || lower.ends_with(".text")) {
        return false;
    }
    // Manifest-convention .txt files are package manifests, not documents.
    !matches!(
        lower.as_str(),
        "requirements.txt" | "conanfile.txt" | "cmakelists.txt"
    )
}

/// `report.pdf.md` -> `report.pdf`: a generated Markdown sidecar for a
/// binary document.
pub(crate) fn markdown_sidecar_source(label: &str) -> Option<String> {
    let stem = label
        .strip_suffix(".md")
        .or_else(|| label.strip_suffix(".markdown"))?;
    let inner_extension = stem.rsplit('.').next()?.to_ascii_lowercase();
    matches!(
        inner_extension.as_str(),
        "pdf" | "doc" | "docx" | "ppt" | "pptx" | "xls" | "xlsx" | "odt" | "odp" | "rtf"
    )
    .then(|| stem.to_string())
}

/// Index a plain-text knowledge file as a document node with provenance:
/// the file becomes a `document` fact and local path mentions become
/// pending document references resolved against scanned files, capped at
/// [`PLAIN_TEXT_REFERENCE_LIMIT`] per file (the scan file-size budget
/// already bounds how much text is read).
pub(crate) fn index_plain_text_document(
    context: &mut IndexContext,
    file_id: NodeId,
    path: &Path,
    label: &str,
    source: &str,
) {
    if !is_plain_text_document(path) {
        return;
    }

    add_file_metadata(&mut context.graph, file_id, "item_kind", "document");
    add_file_metadata(&mut context.graph, file_id, "source", "plain_text");
    add_file_metadata(
        &mut context.graph,
        file_id,
        "document_kind",
        document_kind(path, label),
    );
    add_file_metadata(
        &mut context.graph,
        file_id,
        "line_count",
        source.lines().count().to_string(),
    );

    let mut references = 0usize;
    for (index, line) in source.lines().enumerate() {
        if references >= PLAIN_TEXT_REFERENCE_LIMIT {
            break;
        }
        let line_number = index as u32 + 1;
        for token in line.split([' ', '\t', '(', ')', '<', '>', ',', ';']) {
            let token = token.trim_matches(|c: char| {
                matches!(c, '"' | '\'' | '`' | ':' | '.' | '!' | '?' | '[' | ']')
            });
            // Only path-shaped tokens: a separator plus a known source
            // extension, checked by the shared document-reference rules.
            if !token.contains('/') {
                continue;
            }
            let Some(candidates) = markdown_path_candidates(label, token) else {
                continue;
            };
            context
                .pending_document_path_refs
                .push(PendingDocumentPathRef {
                    source: file_id,
                    target: token.to_string(),
                    candidates,
                    relation: "document_path",
                    line: line_number,
                    text: Some(line.trim().chars().take(160).collect()),
                    line_ref: None,
                });
            references += 1;
            if references >= PLAIN_TEXT_REFERENCE_LIMIT {
                break;
            }
        }
    }
}

pub(crate) fn is_markdown_document(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "md" | "markdown" | "mdown" | "mkdn"
            )
        })
}

pub(crate) fn document_kind(path: &Path, label: &str) -> String {
    let normalized = label.to_ascii_lowercase();
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(label)
        .to_ascii_lowercase();
    if normalized.contains("/adr/")
        || normalized.contains("/adrs/")
        || file_name.starts_with("adr-")
        || file_name.starts_with("adr_")
    {
        "adr".to_string()
    } else if normalized.contains("/rfc/")
        || normalized.contains("/rfcs/")
        || file_name.starts_with("rfc-")
        || file_name.starts_with("rfc_")
    {
        "rfc".to_string()
    } else if file_name.ends_with(".txt") || file_name.ends_with(".text") {
        "plain_text".to_string()
    } else {
        "markdown".to_string()
    }
}
