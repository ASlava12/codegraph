//! Deterministic SQL knowledge: schema extraction from `.sql` files,
//! statement-shaped inline query detection in application code, and
//! table-reference parsing.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use codegraph_core::{Confidence, EdgeKind, NodeId, NodeKind};
use codegraph_parser::{Language, ParsedFile, ParsedItemKind};

#[allow(unused_imports)]
use crate::*;

pub(crate) fn index_sql_schema(
    context: &mut IndexContext,
    file_id: NodeId,
    path: &Path,
    label: &str,
    source: &str,
) {
    if !is_sql_file(path) {
        return;
    }

    add_file_metadata(&mut context.graph, file_id, "language", "sql");
    add_file_metadata(&mut context.graph, file_id, "item_kind", "sql_schema");
    add_file_metadata(&mut context.graph, file_id, "source", "sql");

    for statement in sql_statements(source) {
        let normalized = statement.sql.trim_start().to_ascii_lowercase();
        if normalized.starts_with("create table")
            || normalized.starts_with("create temporary table")
            || normalized.starts_with("create temp table")
        {
            index_sql_create_table(context, file_id, label, source, &statement);
        } else if normalized.starts_with("create view")
            || normalized.starts_with("create or replace view")
            || normalized.starts_with("create materialized view")
        {
            index_sql_create_view(context, file_id, label, source, &statement);
        } else if normalized.starts_with("create index")
            || normalized.starts_with("create unique index")
            || normalized.starts_with("create index if not exists")
            || normalized.starts_with("create unique index if not exists")
        {
            index_sql_create_index(context, file_id, label, source, &statement);
        } else if normalized.starts_with("alter table") {
            index_sql_alter_ref(context, file_id, &statement, "alter");
        } else if normalized.starts_with("drop table") {
            index_sql_alter_ref(context, file_id, &statement, "drop");
        }
        for join in sql_join_pairs(&statement.sql) {
            context.pending_sql_joins.push(PendingSqlJoin {
                left: join.0,
                right: join.1,
                condition: join.2,
                line: statement.line,
            });
        }
    }

    if let Some((dir, sequence, sequence_text)) = sql_migration_sequence(label) {
        context.sql_migrations.push(SqlMigrationFile {
            file: file_id,
            label: label.to_string(),
            dir,
            sequence,
            sequence_text,
        });
    }
}

/// Record an ALTER/DROP TABLE target for schema-consistency resolution.
pub(crate) fn index_sql_alter_ref(
    context: &mut IndexContext,
    file_id: NodeId,
    statement: &SqlStatement,
    operation: &'static str,
) {
    let tokens = sql_query_tokens(&statement.sql);
    // ALTER TABLE [ONLY] name / DROP TABLE [IF EXISTS] name
    let mut index = 2;
    while let Some(token) = tokens.get(index) {
        let lower = token.to_ascii_lowercase();
        if matches!(lower.as_str(), "if" | "exists" | "only") {
            index += 1;
            continue;
        }
        break;
    }
    if let Some(table) = next_sql_table_token(&tokens, index) {
        context.pending_sql_alter_refs.push(PendingSqlAlterRef {
            file: file_id,
            table,
            operation,
            line: statement.line,
        });
    }
}

/// JOIN pairs `(from_table, joined_table, on_condition)` in one statement.
/// Every JOIN is paired with the statement's FROM anchor table — an
/// approximation that keeps chained joins readable without alias tracking.
pub(crate) fn sql_join_pairs(sql: &str) -> Vec<(String, String, Option<String>)> {
    const CONDITION_TERMINATORS: &[&str] = &[
        "join", "left", "right", "inner", "outer", "cross", "full", "where", "group", "order",
        "limit", "union", "having", ";",
    ];
    let tokens = sql_query_tokens(sql);
    let mut from_table: Option<String> = None;
    let mut pairs = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        let lower = tokens[index].to_ascii_lowercase();
        if lower == "from" && from_table.is_none() {
            from_table = next_sql_table_token(&tokens, index + 1);
        } else if lower == "join"
            && let Some(left) = from_table.clone()
            && let Some(right) = next_sql_table_token(&tokens, index + 1)
        {
            // Skip ahead to an optional ON clause and capture its condition.
            let mut cursor = index + 1;
            let mut condition = None;
            while let Some(token) = tokens.get(cursor) {
                let token_lower = token.to_ascii_lowercase();
                if CONDITION_TERMINATORS.contains(&token_lower.as_str()) {
                    break;
                }
                if token_lower == "on" {
                    let mut parts = Vec::new();
                    let mut condition_cursor = cursor + 1;
                    while let Some(part) = tokens.get(condition_cursor) {
                        if CONDITION_TERMINATORS.contains(&part.to_ascii_lowercase().as_str())
                            || parts.len() >= 12
                        {
                            break;
                        }
                        parts.push(part.clone());
                        condition_cursor += 1;
                    }
                    if !parts.is_empty() {
                        condition = Some(parts.join(" "));
                    }
                    break;
                }
                cursor += 1;
            }
            if !sql_identifier_key(&left).eq(&sql_identifier_key(&right)) {
                pairs.push((left, right, condition));
            }
        }
        index += 1;
    }
    pairs
}

/// Migration ordering key for a SQL file: numeric prefix (`001_init.sql`,
/// `20240101_users.sql`) or Flyway `V<digits>__name.sql`, inside any
/// directory (grouping happens per directory).
pub(crate) fn sql_migration_sequence(label: &str) -> Option<(String, u128, String)> {
    let (dir, file_name) = match label.rsplit_once('/') {
        Some((dir, file_name)) => (dir.to_string(), file_name),
        None => (String::new(), label),
    };
    let digits: String = if let Some(rest) = file_name.strip_prefix(['V', 'v']) {
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if digits.is_empty() || !rest[digits.len()..].starts_with("__") {
            return None;
        }
        digits
    } else {
        let digits: String = file_name.chars().take_while(char::is_ascii_digit).collect();
        if digits.is_empty()
            || !matches!(
                file_name[digits.len()..].chars().next(),
                Some('_') | Some('-') | Some('.')
            )
        {
            return None;
        }
        digits
    };
    let sequence = digits.parse::<u128>().ok()?;
    Some((dir, sequence, digits))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SqlStatement {
    sql: String,
    line: u32,
    /// The line the statement ends on. A CREATE TABLE is written across
    /// its columns, and a reader asking to see the table wants them.
    end_line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SqlCreateTable {
    name: String,
    body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SqlColumn {
    name: String,
    data_type: Option<String>,
    nullable: Option<bool>,
    primary_key: bool,
    line: u32,
    references: Option<SqlReference>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SqlReference {
    target_table: String,
    target_column: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SqlTableConstraint {
    columns: Vec<String>,
    reference: SqlReference,
    line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceSqlLiteral {
    pub(crate) value: String,
    pub(crate) line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SqlQueryTableRef {
    pub(crate) operation: String,
    pub(crate) table: String,
    pub(crate) role: String,
}

pub(crate) fn index_inline_sql_queries(
    context: &mut IndexContext,
    file_id: NodeId,
    label: &str,
    language: Language,
    source: &str,
    parsed: &ParsedFile,
    local_functions: &BTreeMap<String, NodeId>,
) {
    let function_ranges = parsed
        .items
        .iter()
        .filter(|item| {
            matches!(
                item.kind,
                ParsedItemKind::Function | ParsedItemKind::Entrypoint
            )
        })
        .filter_map(|item| {
            resolve_local_function(local_functions, &item.label).map(|id| {
                (
                    item.span.start_line,
                    item.span.end_line.max(item.span.start_line),
                    id,
                )
            })
        })
        .collect::<Vec<_>>();

    let test_cutoff = rust_test_module_cutoff(language, source);
    for literal in source_sql_literals(source) {
        let table_refs = sql_query_table_refs(&literal.value);
        if table_refs.is_empty() {
            continue;
        }
        let test_context = test_cutoff.is_some_and(|cutoff| literal.line >= cutoff);

        let operation = table_refs
            .iter()
            .map(|reference| reference.operation.as_str())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join(",");
        let tables = table_refs
            .iter()
            .map(|reference| reference.table.as_str())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join(",");
        let source_id = function_ranges
            .iter()
            .find(|(start, end, _)| literal.line >= *start && literal.line <= *end)
            .map(|(_, _, id)| *id)
            .unwrap_or(file_id);

        let mut metadata = BTreeMap::new();
        metadata.insert("language".to_string(), language.to_string());
        metadata.insert("item_kind".to_string(), "app_sql_query".to_string());
        metadata.insert("source".to_string(), "source_sql_literal".to_string());
        metadata.insert("line".to_string(), literal.line.to_string());
        if test_context {
            metadata.insert("test_context".to_string(), "true".to_string());
        }
        metadata.insert("operation".to_string(), operation);
        metadata.insert("tables".to_string(), tables);
        metadata.insert(
            "query".to_string(),
            truncate_metadata_value(&literal.value, 500),
        );
        let query_id = context.graph.add_node_with_metadata(
            NodeKind::Config,
            format!("sql query:{label}:{}", literal.line),
            Some(line_span(label, source, literal.line)),
            metadata,
        );
        add_edge_once_with_metadata(
            context,
            source_id,
            query_id,
            EdgeKind::References,
            Confidence::Heuristic,
            BTreeMap::from([
                ("relation".to_string(), "app_sql_query".to_string()),
                ("source".to_string(), "source_sql_literal".to_string()),
            ]),
        );

        for reference in table_refs {
            context
                .pending_sql_query_table_refs
                .push(PendingSqlQueryTableRef {
                    query: query_id,
                    table: reference.table,
                    operation: reference.operation,
                    role: reference.role,
                    line: literal.line,
                });
        }
    }
}

pub(crate) fn index_sql_create_table(
    context: &mut IndexContext,
    file_id: NodeId,
    label: &str,
    source: &str,
    statement: &SqlStatement,
) {
    let Some(table) = parse_sql_create_table(&statement.sql) else {
        return;
    };
    let table_key = sql_identifier_key(&table.name);
    if table_key.is_empty() {
        return;
    }

    let mut metadata = BTreeMap::new();
    metadata.insert("language".to_string(), "sql".to_string());
    metadata.insert("item_kind".to_string(), "sql_table".to_string());
    metadata.insert("source".to_string(), "sql".to_string());
    metadata.insert("table_name".to_string(), table.name.clone());
    metadata.insert("table_key".to_string(), table_key.clone());
    metadata.insert("line".to_string(), statement.line.to_string());
    let table_id = context.graph.add_node_with_metadata(
        NodeKind::Type,
        format!("sql table:{}", table.name),
        Some(block_span(
            label,
            source,
            statement.line,
            statement.end_line,
        )),
        metadata,
    );
    context.sql_tables.insert(table_key.clone(), table_id);
    add_edge_once_with_metadata(
        context,
        file_id,
        table_id,
        EdgeKind::Contains,
        Confidence::Exact,
        BTreeMap::from([("relation".to_string(), "sql_table".to_string())]),
    );

    for part in split_sql_comma_items(&table.body) {
        let line = statement.line + sql_line_offset(&statement.sql, &part);
        if let Some(column) = parse_sql_column(&part, line) {
            let column_key = sql_column_key(&table_key, &column.name);
            let mut metadata = BTreeMap::new();
            metadata.insert("language".to_string(), "sql".to_string());
            metadata.insert("item_kind".to_string(), "sql_column".to_string());
            metadata.insert("source".to_string(), "sql".to_string());
            metadata.insert("table_name".to_string(), table.name.clone());
            metadata.insert("table_key".to_string(), table_key.clone());
            metadata.insert("column_name".to_string(), column.name.clone());
            metadata.insert("column_key".to_string(), column_key.clone());
            metadata.insert("line".to_string(), column.line.to_string());
            metadata.insert("primary_key".to_string(), column.primary_key.to_string());
            if let Some(data_type) = column.data_type.as_deref() {
                metadata.insert("data_type".to_string(), data_type.to_string());
            }
            if let Some(nullable) = column.nullable {
                metadata.insert("nullable".to_string(), nullable.to_string());
            }
            let column_id = context.graph.add_node_with_metadata(
                NodeKind::Config,
                format!("sql column:{}.{}", table.name, column.name),
                Some(line_span(label, source, column.line)),
                metadata,
            );
            context.sql_columns.insert(column_key, column_id);
            add_edge_once_with_metadata(
                context,
                table_id,
                column_id,
                EdgeKind::Contains,
                Confidence::Exact,
                BTreeMap::from([("relation".to_string(), "sql_column".to_string())]),
            );
            if let Some(reference) = column.references {
                context.pending_sql_foreign_keys.push(PendingSqlForeignKey {
                    source: column_id,
                    source_table: table.name.clone(),
                    source_column: Some(column.name),
                    target_table: reference.target_table,
                    target_column: reference.target_column,
                    line: column.line,
                });
            }
        } else if let Some(constraint) = parse_sql_table_constraint(&part, line) {
            for column in constraint.columns {
                let source = context
                    .sql_columns
                    .get(&sql_column_key(&table_key, &column))
                    .copied()
                    .unwrap_or(table_id);
                context.pending_sql_foreign_keys.push(PendingSqlForeignKey {
                    source,
                    source_table: table.name.clone(),
                    source_column: Some(column),
                    target_table: constraint.reference.target_table.clone(),
                    target_column: constraint.reference.target_column.clone(),
                    line: constraint.line,
                });
            }
        }
    }
}

pub(crate) fn index_sql_create_view(
    context: &mut IndexContext,
    file_id: NodeId,
    label: &str,
    source: &str,
    statement: &SqlStatement,
) {
    let Some(name) = parse_sql_create_named_object(&statement.sql, "view") else {
        return;
    };
    let mut metadata = BTreeMap::new();
    metadata.insert("language".to_string(), "sql".to_string());
    metadata.insert("item_kind".to_string(), "sql_view".to_string());
    metadata.insert("source".to_string(), "sql".to_string());
    metadata.insert("view_name".to_string(), name.clone());
    metadata.insert("view_key".to_string(), sql_identifier_key(&name));
    metadata.insert("line".to_string(), statement.line.to_string());
    let view_id = context.graph.add_node_with_metadata(
        NodeKind::Type,
        format!("sql view:{name}"),
        Some(block_span(
            label,
            source,
            statement.line,
            statement.end_line,
        )),
        metadata,
    );
    add_edge_once_with_metadata(
        context,
        file_id,
        view_id,
        EdgeKind::Contains,
        Confidence::Exact,
        BTreeMap::from([("relation".to_string(), "sql_view".to_string())]),
    );
}

pub(crate) fn index_sql_create_index(
    context: &mut IndexContext,
    file_id: NodeId,
    label: &str,
    source: &str,
    statement: &SqlStatement,
) {
    let Some((index_name, table_name, columns, unique)) = parse_sql_create_index(&statement.sql)
    else {
        return;
    };
    let table_key = sql_identifier_key(&table_name);
    let mut metadata = BTreeMap::new();
    metadata.insert("language".to_string(), "sql".to_string());
    metadata.insert("item_kind".to_string(), "sql_index".to_string());
    metadata.insert("source".to_string(), "sql".to_string());
    metadata.insert("index_name".to_string(), index_name.clone());
    metadata.insert("table_name".to_string(), table_name.clone());
    metadata.insert("table_key".to_string(), table_key.clone());
    metadata.insert("columns".to_string(), columns.join(","));
    metadata.insert("unique".to_string(), unique.to_string());
    metadata.insert("line".to_string(), statement.line.to_string());
    let index_id = context.graph.add_node_with_metadata(
        NodeKind::Config,
        format!("sql index:{index_name}"),
        Some(block_span(
            label,
            source,
            statement.line,
            statement.end_line,
        )),
        metadata,
    );
    add_edge_once_with_metadata(
        context,
        file_id,
        index_id,
        EdgeKind::Contains,
        Confidence::Exact,
        BTreeMap::from([("relation".to_string(), "sql_index".to_string())]),
    );
    if let Some(table_id) = context.sql_tables.get(&table_key).copied() {
        add_edge_once_with_metadata(
            context,
            index_id,
            table_id,
            EdgeKind::References,
            Confidence::Exact,
            BTreeMap::from([
                ("relation".to_string(), "sql_index_table".to_string()),
                ("source".to_string(), "sql".to_string()),
            ]),
        );
    }
}

/// Line of the first `#[cfg(test)]` marker in a Rust source. Facts extracted
/// at or below it come from inline test modules, mirroring the
/// benchmark-oracle exclusion for test code.
pub(crate) fn rust_test_module_cutoff(language: Language, source: &str) -> Option<u32> {
    if language != Language::Rust {
        return None;
    }
    source
        .find("#[cfg(test)]")
        .map(|offset| byte_line_number(source, offset))
}

/// Whether a source could hold a SQL statement at all. A statement has to
/// open with one of six verbs, so a file whose text holds none of them
/// holds no query, and the literals never have to be pulled out of it.
fn could_hold_sql(source: &str) -> bool {
    const VERBS: [&[u8]; 6] = [
        b"select", b"insert", b"update", b"delete", b"replace", b"with",
    ];
    let bytes = source.as_bytes();
    bytes.iter().enumerate().any(|(index, byte)| {
        let opening = byte.to_ascii_lowercase();
        matches!(opening, b's' | b'i' | b'u' | b'd' | b'r' | b'w')
            && VERBS.iter().any(|verb| {
                verb[0] == opening
                    && bytes
                        .get(index..index + verb.len())
                        .is_some_and(|window| window.eq_ignore_ascii_case(verb))
            })
    })
}

pub(crate) fn source_sql_literals(source: &str) -> Vec<SourceSqlLiteral> {
    let mut literals = Vec::new();
    if !could_hold_sql(source) {
        return literals;
    }
    let mut cursor = 0;
    // The line is carried along with the cursor, which only ever moves
    // forward. Counting newlines from the start of the file for every
    // literal made a long file with many strings cost its length squared.
    let mut line = 1u32;
    let mut counted = 0usize;

    while cursor < source.len() {
        if let Some((value, end)) = raw_string_literal_at(source, cursor) {
            line += newlines_in(&source[counted..cursor]);
            counted = cursor;
            literals.push(SourceSqlLiteral { value, line });
            cursor = end;
            continue;
        }

        let Some(character) = source[cursor..].chars().next() else {
            break;
        };
        if !matches!(character, '"' | '\'' | '`') {
            cursor += character.len_utf8();
            continue;
        }
        line += newlines_in(&source[counted..cursor]);
        counted = cursor;

        let quote = character as u8;
        let bytes = source.as_bytes();
        let is_triple = character != '`'
            && bytes.get(cursor + 1) == Some(&quote)
            && bytes.get(cursor + 2) == Some(&quote);
        if is_triple {
            let content_start = cursor + 3;
            let triple = &source[cursor..content_start];
            if let Some(relative_end) = source[content_start..].find(triple) {
                literals.push(SourceSqlLiteral {
                    value: source[content_start..content_start + relative_end].to_string(),
                    line,
                });
                cursor = content_start + relative_end + triple.len();
                continue;
            }
        }

        let content_start = cursor + character.len_utf8();
        let mut escaped = false;
        let mut value = String::new();
        let mut end = None;
        for (relative, next) in source[content_start..].char_indices() {
            if escaped {
                value.push(next);
                escaped = false;
                continue;
            }
            if next == '\\' {
                escaped = true;
                continue;
            }
            if next == character {
                end = Some(content_start + relative + next.len_utf8());
                break;
            }
            value.push(next);
        }

        if let Some(end) = end {
            literals.push(SourceSqlLiteral { value, line });
            cursor = end;
        } else {
            cursor += character.len_utf8();
        }
    }

    literals
}

fn newlines_in(text: &str) -> u32 {
    text.bytes().filter(|byte| *byte == b'\n').count() as u32
}

pub(crate) fn raw_string_literal_at(source: &str, cursor: usize) -> Option<(String, usize)> {
    let rest = source.get(cursor..)?;
    if !rest.starts_with('r') {
        return None;
    }

    let bytes = rest.as_bytes();
    let mut index = 1;
    while bytes.get(index).is_some_and(|byte| *byte == b'#') {
        index += 1;
    }
    if bytes.get(index) != Some(&b'"') {
        return None;
    }

    let hashes = &rest[1..index];
    let content_start = cursor + index + 1;
    let delimiter = format!("\"{hashes}");
    let relative_end = source[content_start..].find(&delimiter)?;
    Some((
        source[content_start..content_start + relative_end].to_string(),
        content_start + relative_end + delimiter.len(),
    ))
}

/// Sentences end where SQL never does: a qualified SQL name always continues
/// after its dot, so a token that ends with one came from prose ("Select
/// context to install from. By default, install files from all contexts.").
fn ends_a_prose_sentence(token: &str) -> bool {
    token == "." || (token.ends_with('.') && token.chars().any(char::is_alphabetic))
}

/// A string literal only counts as an SQL statement when its first token is a
/// statement keyword. Without this gate, prose such as "Build a workflow from
/// a selected node" matches the bare `from <word>` pattern and produces
/// phantom table references (Phase 9 dogfooding).
pub(crate) fn sql_statement_shaped(tokens: &[String]) -> bool {
    if tokens.iter().any(|token| ends_a_prose_sentence(token)) {
        return false;
    }
    let Some(first) = tokens.first() else {
        return false;
    };
    let carries = |keyword: &str| {
        tokens
            .iter()
            .skip(1)
            .any(|token| token.eq_ignore_ascii_case(keyword))
    };
    match first.to_ascii_lowercase().as_str() {
        // Real statements name where their rows come from or go to; the test
        // name "insert, update, and delete raises on stale entries" does not.
        "select" | "delete" => carries("from"),
        "insert" | "replace" => carries("into"),
        // Real UPDATE statements carry a SET clause; "Update Cache" does not.
        "update" => carries("set"),
        // Real CTEs contain a SELECT; "With flour from the mill" does not.
        "with" => carries("select"),
        _ => false,
    }
}

/// True when the statement holding `index` was opened by `keyword`.
fn opens_the_statement(tokens: &[String], index: usize, keyword: &str) -> bool {
    let start = tokens[..index]
        .iter()
        .rposition(|token| matches!(token.as_str(), ";" | ")" | "("))
        .map(|boundary| boundary + 1)
        .unwrap_or(0);
    tokens
        .get(start)
        .is_some_and(|token| token.eq_ignore_ascii_case(keyword))
}

/// UPDATE, INSERT INTO, DELETE and REPLACE INTO open a statement. Mid-sentence
/// they are ordinary words ("... cannot update (delete first)"), so bind them
/// only where a statement can begin.
fn opens_a_statement(tokens: &[String], index: usize) -> bool {
    match index.checked_sub(1) {
        None => true,
        Some(previous) => matches!(tokens[previous].as_str(), ";" | "("),
    }
}

pub(crate) fn sql_query_table_refs(query: &str) -> Vec<SqlQueryTableRef> {
    let tokens = sql_query_tokens(query);
    if !sql_statement_shaped(&tokens) {
        return Vec::new();
    }
    let mut refs = Vec::new();
    let mut seen = BTreeSet::new();

    for (index, token) in tokens.iter().enumerate() {
        let lower = token.to_ascii_lowercase();
        let reference = match lower.as_str() {
            "from" if reads_a_row_source(&tokens, index) => {
                next_sql_table_token(&tokens, index + 1).map(|table| SqlQueryTableRef {
                    operation: "select".to_string(),
                    table,
                    role: "source".to_string(),
                })
            }
            "join" => next_sql_table_token(&tokens, index + 1).map(|table| SqlQueryTableRef {
                operation: "select".to_string(),
                table,
                role: "join".to_string(),
            }),
            "into"
                if opens_the_statement(&tokens, index, "insert")
                    || opens_the_statement(&tokens, index, "replace") =>
            {
                next_sql_table_token(&tokens, index + 1).map(|table| SqlQueryTableRef {
                    operation: "insert".to_string(),
                    table,
                    role: "target".to_string(),
                })
            }
            "update" if opens_a_statement(&tokens, index) => {
                next_sql_table_token(&tokens, index + 1).map(|table| SqlQueryTableRef {
                    operation: "update".to_string(),
                    table,
                    role: "target".to_string(),
                })
            }
            "delete" if opens_a_statement(&tokens, index) => tokens
                .get(index + 1)
                .filter(|token| token.eq_ignore_ascii_case("from"))
                .and_then(|_| next_sql_table_token(&tokens, index + 2))
                .map(|table| SqlQueryTableRef {
                    operation: "delete".to_string(),
                    table,
                    role: "target".to_string(),
                }),
            _ => None,
        };

        if let Some(reference) = reference {
            let key = (
                reference.operation.clone(),
                sql_identifier_key(&reference.table),
                reference.role.clone(),
            );
            if seen.insert(key) {
                refs.push(reference);
            }
        }
    }

    refs
}

pub(crate) fn sql_query_tokens(query: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut cursor = 0;

    while cursor < query.len() {
        let Some(character) = query[cursor..].chars().next() else {
            break;
        };

        if character.is_whitespace() {
            cursor += character.len_utf8();
            continue;
        }
        if matches!(character, '(' | ')' | ',' | ';') {
            tokens.push(character.to_string());
            cursor += character.len_utf8();
            continue;
        }
        if character == '\'' {
            cursor = skip_quoted_sql_fragment(query, cursor, '\'');
            continue;
        }
        if matches!(character, '"' | '`') {
            let quote = character;
            let content_start = cursor + quote.len_utf8();
            let mut value = String::new();
            let mut end = None;
            for (relative, next) in query[content_start..].char_indices() {
                if next == quote {
                    end = Some(content_start + relative + next.len_utf8());
                    break;
                }
                value.push(next);
            }
            if !value.trim().is_empty() {
                tokens.push(value);
            }
            cursor = end.unwrap_or(content_start);
            continue;
        }
        if character == '[' {
            let content_start = cursor + 1;
            if let Some(relative_end) = query[content_start..].find(']') {
                let value = query[content_start..content_start + relative_end].trim();
                if !value.is_empty() {
                    tokens.push(value.to_string());
                }
                cursor = content_start + relative_end + 1;
                continue;
            }
        }

        let start = cursor;
        cursor += character.len_utf8();
        while cursor < query.len() {
            let Some(next) = query[cursor..].chars().next() else {
                break;
            };
            if next.is_whitespace()
                || matches!(next, '(' | ')' | ',' | ';' | '\'' | '"' | '`' | '[' | ']')
            {
                break;
            }
            cursor += next.len_utf8();
        }
        let token = query[start..cursor]
            .trim_matches(|character: char| matches!(character, ':' | ';'))
            .to_string();
        if !token.is_empty() {
            tokens.push(token);
        }
    }

    tokens
}

pub(crate) fn skip_quoted_sql_fragment(query: &str, cursor: usize, quote: char) -> usize {
    let content_start = cursor + quote.len_utf8();
    let mut escaped = false;
    for (relative, next) in query[content_start..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if next == '\\' {
            escaped = true;
            continue;
        }
        if next == quote {
            return content_start + relative + next.len_utf8();
        }
    }
    content_start
}

/// `EXTRACT(EPOCH FROM ts)` and `SUBSTRING(x FROM 2)` spell FROM inside a call,
/// where it separates arguments instead of naming a row source. A FROM clause
/// belongs to a SELECT, so the parentheses around it must open one.
fn reads_a_row_source(tokens: &[String], index: usize) -> bool {
    let mut depth = 0usize;
    for position in (0..index).rev() {
        match tokens[position].as_str() {
            ")" => depth += 1,
            "(" if depth > 0 => depth -= 1,
            "(" => {
                return tokens[position + 1..index]
                    .iter()
                    .any(|token| token.eq_ignore_ascii_case("select"));
            }
            _ => {}
        }
    }
    true
}

/// A table alias is a bare name, so a qualified name where an alias would sit
/// means the text is prose: "Delete from uninitialized collections.Map".
fn qualified_alias_follows(tokens: &[String], table_index: usize) -> bool {
    tokens
        .get(table_index + 1)
        .is_some_and(|next| next.contains('.') && !next.starts_with('.') && !next.ends_with('.'))
}

pub(crate) fn next_sql_table_token(tokens: &[String], start: usize) -> Option<String> {
    let mut index = start;
    while let Some(token) = tokens.get(index) {
        let lower = token.to_ascii_lowercase();
        if matches!(lower.as_str(), "only" | "," | "(") {
            index += 1;
            continue;
        }
        if lower == "select" || lower == "values" {
            return None;
        }
        if is_sql_non_table_keyword(&lower) {
            return None;
        }
        if qualified_alias_follows(tokens, index) {
            return None;
        }
        let table = token
            .trim_matches(|character: char| matches!(character, '"' | '\'' | '`' | '[' | ']'))
            .trim()
            .to_string();
        return (!table.is_empty()).then_some(table);
    }
    None
}

pub(crate) fn is_sql_non_table_keyword(token: &str) -> bool {
    matches!(
        token,
        "as" | "by"
            | "case"
            | "cross"
            | "distinct"
            | "full"
            | "group"
            | "having"
            | "inner"
            | "left"
            | "limit"
            | "offset"
            | "on"
            | "order"
            | "outer"
            | "returning"
            | "right"
            | "set"
            | "using"
            | "where"
    )
}

pub(crate) fn sql_statements(source: &str) -> Vec<SqlStatement> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut line = 1u32;
    let mut statement_line = 1u32;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut chars = source.chars().peekable();

    while let Some(character) = chars.next() {
        if current.trim().is_empty() {
            statement_line = line;
        }

        if in_line_comment {
            if character == '\n' {
                in_line_comment = false;
                line += 1;
            }
            continue;
        }
        if in_block_comment {
            if character == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_block_comment = false;
            } else if character == '\n' {
                line += 1;
            }
            continue;
        }
        if !in_single_quote && !in_double_quote && character == '-' && chars.peek() == Some(&'-') {
            chars.next();
            in_line_comment = true;
            continue;
        }
        if !in_single_quote && !in_double_quote && character == '/' && chars.peek() == Some(&'*') {
            chars.next();
            in_block_comment = true;
            continue;
        }

        match character {
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
                current.push(character);
            }
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
                current.push(character);
            }
            ';' if !in_single_quote && !in_double_quote => {
                let sql = current.trim().to_string();
                if !sql.is_empty() {
                    statements.push(SqlStatement {
                        sql,
                        line: statement_line,
                        end_line: line,
                    });
                }
                current.clear();
            }
            '\n' => {
                current.push(character);
                line += 1;
            }
            _ => current.push(character),
        }
    }

    let sql = current.trim().to_string();
    if !sql.is_empty() {
        statements.push(SqlStatement {
            sql,
            line: statement_line,
            end_line: line,
        });
    }

    statements
}

pub(crate) fn parse_sql_create_table(sql: &str) -> Option<SqlCreateTable> {
    let normalized = sql.to_ascii_lowercase();
    let table_index = normalized.find("table")?;
    let after_table = sql[table_index + "table".len()..].trim_start();
    let after_table = after_table
        .strip_prefix("if not exists")
        .map(str::trim_start)
        .unwrap_or(after_table);
    let (name, rest) = read_sql_identifier(after_table)?;
    let open = rest.find('(')?;
    let close = matching_closing_paren(rest, open)?;
    Some(SqlCreateTable {
        name,
        body: rest[open + 1..close].to_string(),
    })
}

pub(crate) fn parse_sql_create_named_object(sql: &str, keyword: &str) -> Option<String> {
    let normalized = sql.to_ascii_lowercase();
    let keyword_index = normalized.find(keyword)?;
    let mut rest = sql[keyword_index + keyword.len()..].trim_start();
    rest = rest
        .strip_prefix("if not exists")
        .map(str::trim_start)
        .unwrap_or(rest);
    read_sql_identifier(rest).map(|(name, _)| name)
}

pub(crate) fn parse_sql_create_index(sql: &str) -> Option<(String, String, Vec<String>, bool)> {
    let tokens = sql_first_words(sql, 8);
    let unique = tokens.iter().any(|token| token == "unique");
    let normalized = sql.to_ascii_lowercase();
    let index_pos = normalized.find(" index ")?;
    let mut rest = sql[index_pos + " index ".len()..].trim_start();
    rest = rest
        .strip_prefix("if not exists")
        .map(str::trim_start)
        .unwrap_or(rest);
    let (index_name, after_name) = read_sql_identifier(rest)?;
    let after_name_lower = after_name.trim_start().to_ascii_lowercase();
    if !after_name_lower.starts_with("on ") {
        return None;
    }
    let after_on = after_name.trim_start()[2..].trim_start();
    let (table_name, after_table) = read_sql_identifier(after_on)?;
    let open = after_table.find('(')?;
    let close = matching_closing_paren(after_table, open)?;
    let columns = split_sql_comma_items(&after_table[open + 1..close])
        .into_iter()
        .filter_map(|item| read_sql_identifier(item.trim()).map(|(name, _)| name))
        .collect();
    Some((index_name, table_name, columns, unique))
}

pub(crate) fn parse_sql_column(part: &str, line: u32) -> Option<SqlColumn> {
    let trimmed = part.trim();
    let first = trimmed
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    if matches!(
        first.as_str(),
        "constraint" | "primary" | "foreign" | "unique" | "check" | "exclude"
    ) {
        return None;
    }
    let (name, rest) = read_sql_identifier(trimmed)?;
    let rest = rest.trim();
    let rest_lower = rest.to_ascii_lowercase();
    let data_type = sql_column_data_type(rest);
    let nullable = if rest_lower.contains("not null") {
        Some(false)
    } else if rest_lower.contains(" null") || rest_lower.ends_with(" null") {
        Some(true)
    } else {
        None
    };
    let primary_key = rest_lower.contains("primary key");
    let references = parse_sql_references(rest);
    Some(SqlColumn {
        name,
        data_type,
        nullable,
        primary_key,
        line,
        references,
    })
}

pub(crate) fn parse_sql_table_constraint(part: &str, line: u32) -> Option<SqlTableConstraint> {
    let mut text = part.trim();
    let lower = text.to_ascii_lowercase();
    if lower.starts_with("constraint ") {
        let rest = text
            .split_once(char::is_whitespace)
            .map(|(_, rest)| rest.trim_start())?;
        let (_, rest) = read_sql_identifier(rest)?;
        text = rest.trim_start();
    }
    let lower = text.to_ascii_lowercase();
    if !lower.starts_with("foreign key") {
        return None;
    }
    let open = text.find('(')?;
    let close = matching_closing_paren(text, open)?;
    let columns = split_sql_comma_items(&text[open + 1..close])
        .into_iter()
        .filter_map(|item| read_sql_identifier(item.trim()).map(|(name, _)| name))
        .collect::<Vec<_>>();
    let reference = parse_sql_references(&text[close + 1..])?;
    Some(SqlTableConstraint {
        columns,
        reference,
        line,
    })
}

pub(crate) fn parse_sql_references(text: &str) -> Option<SqlReference> {
    let lower = text.to_ascii_lowercase();
    let index = lower.find("references")?;
    let rest = text[index + "references".len()..].trim_start();
    let (target_table, after_table) = read_sql_identifier(rest)?;
    let target_column = after_table
        .find('(')
        .and_then(|open| matching_closing_paren(after_table, open).map(|close| (open, close)))
        .and_then(|(open, close)| {
            split_sql_comma_items(&after_table[open + 1..close])
                .into_iter()
                .next()
        })
        .and_then(|value| read_sql_identifier(value.trim()).map(|(name, _)| name));
    Some(SqlReference {
        target_table,
        target_column,
    })
}

pub(crate) fn sql_column_data_type(rest: &str) -> Option<String> {
    let stop_words = [
        "not",
        "null",
        "primary",
        "unique",
        "references",
        "default",
        "check",
        "constraint",
        "generated",
        "collate",
    ];
    let mut parts = Vec::new();
    for token in rest.split_whitespace() {
        let normalized = token
            .trim_matches(',')
            .trim_matches('(')
            .trim_matches(')')
            .to_ascii_lowercase();
        if stop_words.contains(&normalized.as_str()) {
            break;
        }
        parts.push(token.trim_matches(','));
    }
    (!parts.is_empty()).then(|| parts.join(" "))
}

pub(crate) fn split_sql_comma_items(text: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    for character in text.chars() {
        match character {
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
                current.push(character);
            }
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
                current.push(character);
            }
            '(' if !in_single_quote && !in_double_quote => {
                depth += 1;
                current.push(character);
            }
            ')' if !in_single_quote && !in_double_quote => {
                depth = depth.saturating_sub(1);
                current.push(character);
            }
            ',' if depth == 0 && !in_single_quote && !in_double_quote => {
                let item = current.trim();
                if !item.is_empty() {
                    items.push(item.to_string());
                }
                current.clear();
            }
            _ => current.push(character),
        }
    }
    let item = current.trim();
    if !item.is_empty() {
        items.push(item.to_string());
    }
    items
}

pub(crate) fn read_sql_identifier(text: &str) -> Option<(String, &str)> {
    let text = text.trim_start();
    let mut chars = text.char_indices();
    let (_, first) = chars.next()?;
    if matches!(first, '"' | '`' | '[') {
        let close = if first == '[' { ']' } else { first };
        let end = text[1..].find(close)? + 1;
        let name = text[1..end].to_string();
        return Some((name, &text[end + close.len_utf8()..]));
    }

    let end = text
        .char_indices()
        .take_while(|(_, character)| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '.' | '$')
        })
        .map(|(index, character)| index + character.len_utf8())
        .last()?;
    Some((text[..end].to_string(), &text[end..]))
}

pub(crate) fn matching_closing_paren(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    for (index, character) in text.char_indices().skip_while(|(index, _)| *index < open) {
        match character {
            '\'' if !in_double_quote => in_single_quote = !in_single_quote,
            '"' if !in_single_quote => in_double_quote = !in_double_quote,
            '(' if !in_single_quote && !in_double_quote => depth += 1,
            ')' if !in_single_quote && !in_double_quote => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

pub(crate) fn sql_first_words(sql: &str, limit: usize) -> Vec<String> {
    sql.split_whitespace()
        .take(limit)
        .map(|token| token.trim_matches(',').to_ascii_lowercase())
        .collect()
}

pub(crate) fn sql_identifier_key(identifier: &str) -> String {
    identifier
        .trim()
        .trim_matches('"')
        .trim_matches('`')
        .trim_matches('[')
        .trim_matches(']')
        .to_ascii_lowercase()
}

pub(crate) fn sql_column_key(table_key: &str, column: &str) -> String {
    format!("{}.{}", table_key, sql_identifier_key(column))
}

pub(crate) fn sql_line_offset(statement: &str, part: &str) -> u32 {
    statement
        .find(part.trim())
        .map(|index| statement[..index].lines().count().saturating_sub(1) as u32)
        .unwrap_or(0)
}

pub(crate) fn byte_line_number(source: &str, offset: usize) -> u32 {
    source[..offset.min(source.len())]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count() as u32
        + 1
}

pub(crate) fn truncate_metadata_value(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut truncated = value.chars().take(max_chars).collect::<String>();
    truncated.push_str("...");
    truncated
}
