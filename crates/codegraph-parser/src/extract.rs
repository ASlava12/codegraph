//! Tree walking and fact classification: parse a source file and turn
//! syntax nodes into structural, call, entrypoint, and control-flow items.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use codegraph_core::{SourceSpan, is_test_like_source_path};
use tree_sitter::{Node, Parser};

use crate::*;

pub fn parse_source(
    path: impl AsRef<Path>,
    source: &[u8],
    language: Language,
) -> Result<ParsedFile, ParseError> {
    let path = path.as_ref();
    let source_text = std::str::from_utf8(source).map_err(|_| ParseError::InvalidUtf8)?;
    let mut parser = Parser::new();
    parser
        .set_language(&language.tree_sitter_language())
        .map_err(|error| ParseError::LanguageSetup {
            language,
            message: error.to_string(),
        })?;

    // `SPDLOG_NAMESPACE_BEGIN` and `NLOHMANN_JSON_NAMESPACE_BEGIN` open a
    // namespace through a macro the grammar has never seen, and what
    // follows -- 169 files across spdlog and nlohmann/json -- is read as
    // something else entirely: spdlog's central `logger` class had no node
    // at all. Blanking the line keeps every other line where it was.
    let masked = mask_macro_namespace_lines(language, source_text);
    let source_text = masked.as_deref().unwrap_or(source_text);
    let tree = parser
        .parse(source_text, None)
        .ok_or(ParseError::ParseFailed { language })?;
    let root = tree.root_node();
    let mut facts = CollectedFacts::default();
    let config_aliases = if language == Language::Nix {
        nix_config_aliases(root, source_text.as_bytes())
    } else {
        BTreeMap::new()
    };
    collect_items(
        &WalkContext {
            language,
            source: source_text.as_bytes(),
            path: &path.to_string_lossy(),
            config_aliases: &config_aliases,
        },
        root,
        None,
        &DefinitionScope::default(),
        &mut facts,
        0,
        false,
    );
    if language == Language::Bash {
        drop_bash_local_variable_reads(&mut facts.items, root, source_text.as_bytes());
    }
    // A C# file with top-level statements IS the program: the language
    // allows them in one file per project and the compiler wraps them in
    // `Program.Main`. eShopOnWeb starts its three programs that way, and
    // with no `Main` to find, nothing said where any of them begins.
    if language == Language::CSharp
        && !facts
            .items
            .iter()
            .any(|item| item.kind == ParsedItemKind::Entrypoint)
        && let Some(statement) = csharp_top_level_statement(root)
    {
        facts.items.push(ParsedItem {
            kind: ParsedItemKind::Entrypoint,
            label: "Program".to_string(),
            span: span_for(&path.to_string_lossy(), statement),
            parent: None,
            metadata: BTreeMap::from([(
                "definition_form".to_string(),
                "top_level_statements".to_string(),
            )]),
        });
    }
    dedupe_items(&mut facts.items);
    dedupe_type_references(&mut facts.type_references);

    Ok(ParsedFile {
        language,
        items: facts.items,
        type_references: facts.type_references,
        string_constants: string_constants(language, root, source_text.as_bytes()),
        quoted_line_ranges: line_ranges_of(root, source_text, QuotedKinds::StringsAndComments),
        string_line_ranges: line_ranges_of(root, source_text, QuotedKinds::StringsOnly),
        has_error_nodes: root.has_error(),
        first_error_line: first_error_line(root),
    })
}

/// The first statement a C# file writes outside any declaration. The
/// grammar wraps each one in a `global_statement`, and a file that has
/// one is the entry point the compiler generates `Main` for.
fn csharp_top_level_statement(root: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = root.walk();
    root.named_children(&mut cursor)
        .find(|child| child.kind() == "global_statement")
}

/// The names a file binds to a string literal at its top level. A Go
/// program spells the environment variables it reads as constants --
/// `const envLogFile = "TF_LOG_PATH"` -- and the read names the constant,
/// so 45 of terraform's 62 environment reads had no variable name to give.
fn string_constants(language: Language, root: Node<'_>, source: &[u8]) -> Vec<(String, String)> {
    if language != Language::Go {
        return Vec::new();
    }
    let mut constants = Vec::new();
    let mut cursor = root.walk();
    for declaration in root.named_children(&mut cursor) {
        if !matches!(declaration.kind(), "const_declaration" | "var_declaration") {
            continue;
        }
        let mut specs = declaration.walk();
        for spec in declaration.named_children(&mut specs) {
            if !matches!(spec.kind(), "const_spec" | "var_spec") {
                continue;
            }
            // `a, b = "x", "y"` binds two names at once, and which value
            // belongs to which name is a question the field cannot answer.
            let mut names = spec.walk();
            if spec
                .named_children(&mut names)
                .filter(|child| child.kind() == "identifier")
                .count()
                != 1
            {
                continue;
            }
            let Some(name) = spec
                .child_by_field_name("name")
                .and_then(|name| node_text(name, source))
            else {
                continue;
            };
            let Some(value) = spec.child_by_field_name("value") else {
                continue;
            };
            let literal = value.named_child(0).unwrap_or(value);
            if !matches!(
                literal.kind(),
                "interpreted_string_literal" | "raw_string_literal"
            ) {
                continue;
            }
            let Some(text) = node_text(literal, source) else {
                continue;
            };
            let text = text
                .trim()
                .trim_matches(|character| matches!(character, '"' | '`'));
            if name.is_empty() || text.is_empty() {
                continue;
            }
            constants.push((name, text.to_string()));
        }
    }
    constants
}

/// A C or C++ line that is a bare uppercase macro opening or closing a
/// namespace -- `SPDLOG_NAMESPACE_BEGIN`, `NLOHMANN_JSON_NAMESPACE_END`.
/// Returns the source with those lines blanked, or `None` when the file
/// has none, so nothing is copied for the files that do not need it.
fn mask_macro_namespace_lines(language: Language, source: &str) -> Option<String> {
    if !matches!(language, Language::C | Language::Cpp) {
        return None;
    }
    let names_a_namespace_macro = |line: &str| {
        let trimmed = line.trim();
        (trimmed.ends_with("_BEGIN") || trimmed.ends_with("_END"))
            && trimmed.chars().all(|character| {
                character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
            })
            && trimmed
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_uppercase())
    };
    if !source.lines().any(names_a_namespace_macro) {
        return None;
    }
    let mut masked = String::with_capacity(source.len());
    for line in source.split_inclusive('\n') {
        if names_a_namespace_macro(line.trim_end_matches(['\n', '\r'])) {
            // Keep the line, and the file's length, without its content.
            for character in line.chars() {
                masked.push(if character == '\n' || character == '\r' {
                    character
                } else {
                    ' '
                });
            }
        } else {
            masked.push_str(line);
        }
    }
    Some(masked)
}

/// The 1-based line of the first error or missing node in the tree. Only
/// the branches that hold one are walked: `has_error` marks the path down
/// to it, so a clean file costs one check.
fn first_error_line(node: Node<'_>) -> Option<u32> {
    if !node.has_error() && !node.is_missing() {
        return None;
    }
    if node.is_error() || node.is_missing() {
        return Some(node.start_position().row as u32 + 1);
    }
    let mut cursor = node.walk();
    node.children(&mut cursor).find_map(first_error_line)
}

/// The lines covered by string literals and comments, merged. A detector
/// that scans text rather than syntax needs them: `@app.route("/")` in a
/// docstring is an example, not a route the program serves, and one such
/// line in flask claimed about 140 functions as its handler.
/// Which literals a range scan is about. A comment marker inside a raw
/// string is a fixture rather than a comment on this repository, so the
/// rationale scan needs the strings without the comments.
#[derive(Clone, Copy, PartialEq, Eq)]
enum QuotedKinds {
    StringsAndComments,
    StringsOnly,
}

fn line_ranges_of(root: Node<'_>, source: &str, kinds: QuotedKinds) -> Vec<(u32, u32)> {
    let lines: Vec<&str> = source.lines().collect();
    let mut ranges: Vec<(u32, u32)> = Vec::new();
    let mut stack = vec![root];
    let mut visited = 0usize;
    while let Some(node) = stack.pop() {
        visited += 1;
        if visited > MAX_QUOTED_SCAN_NODES {
            break;
        }
        let kind = node.kind();
        let is_string = kind.contains("string") || kind.contains("heredoc");
        let is_comment = kind.contains("comment");
        if is_string || (is_comment && kinds == QuotedKinds::StringsAndComments) {
            let start = node.start_position();
            let end = node.end_position();
            if end.row > start.row {
                // Only the lines the literal covers whole. Its first line
                // holds the opening quote and whatever preceded it, and its
                // last line holds whatever follows the close.
                if end.row > start.row + 1 {
                    ranges.push((start.row as u32 + 2, end.row as u32));
                }
            } else if lines
                .get(start.row)
                .is_some_and(|line| line[..start.column.min(line.len())].trim().is_empty())
            {
                // A line that begins with the literal is entirely inside it:
                // a commented-out route is not a route.
                ranges.push((start.row as u32 + 1, start.row as u32 + 1));
            }
            // Nothing inside a literal needs its own range.
            continue;
        }
        let mut cursor = node.walk();
        stack.extend(node.named_children(&mut cursor));
    }
    ranges.sort_unstable();
    let mut merged: Vec<(u32, u32)> = Vec::new();
    for (start, end) in ranges {
        match merged.last_mut() {
            Some((_, last_end)) if start <= last_end.saturating_add(1) => {
                *last_end = (*last_end).max(end);
            }
            _ => merged.push((start, end)),
        }
    }
    merged
}

/// A cap on the literal scan, for the same reason [`MAX_TREE_DEPTH`] exists:
/// a generated file must degrade, not hang.
const MAX_QUOTED_SCAN_NODES: usize = 200_000;

/// Deepest syntax-tree level walked. Real source nests far below this; the cap
/// exists only to keep a pathological (minified/generated) file from
/// overflowing the stack — hitting it degrades gracefully (deeper facts are
/// skipped) instead of aborting the process.
const MAX_TREE_DEPTH: usize = 512;

#[derive(Default)]
pub(crate) struct CollectedFacts {
    items: Vec<ParsedItem>,
    type_references: Vec<ParsedTypeReference>,
}

/// What stays fixed for a whole file walk, so the recursion carries only what
/// actually changes per node.
pub(crate) struct WalkContext<'a> {
    pub(crate) language: Language,
    pub(crate) source: &'a [u8],
    pub(crate) path: &'a str,
    /// What a name stands for where a file gives one to a part of the
    /// configuration: `cfg = config.programs.git;`, which home-manager
    /// writes 677 times and then reads 6546 times as `cfg.something`.
    pub(crate) config_aliases: &'a BTreeMap<String, String>,
}

/// Names the Python language provides, which no project declares: a
/// project's own `Basket` is worth an edge and `str` is not.
fn python_builtin_type_name(label: &str) -> bool {
    matches!(
        label,
        "str"
            | "int"
            | "float"
            | "bool"
            | "bytes"
            | "list"
            | "dict"
            | "set"
            | "tuple"
            | "frozenset"
            | "object"
            | "type"
            | "None"
            | "Any"
            | "Optional"
            | "Union"
            | "List"
            | "Dict"
            | "Set"
            | "Tuple"
            | "Callable"
            | "Iterable"
            | "Iterator"
            | "Sequence"
            | "Mapping"
            | "Awaitable"
            | "Self"
            | "Literal"
            | "TypeVar"
            | "Generic"
            | "Protocol"
            | "Enum"
            | "Exception"
            | "BaseException"
            | "ValueError"
            | "TypeError"
            | "KeyError"
    )
}

/// Whether this `function_definition` is really a class an export macro
/// stands in front of: its return type is a class or struct with no body
/// of its own, and what follows is a plain name rather than a parameter
/// list.
fn names_an_exported_c_class(node: Node<'_>) -> bool {
    node.child_by_field_name("type").is_some_and(|type_node| {
        matches!(
            type_node.kind(),
            "class_specifier" | "struct_specifier" | "union_specifier"
        ) && type_node.child_by_field_name("body").is_none()
    }) && node
        .child_by_field_name("declarator")
        .is_some_and(|declarator| declarator.kind() == "identifier")
}

/// Whether a name is a type parameter rather than a type: `T`, `A`, `K`,
/// `V`, `T1`. Every generic declaration writes them and no project means
/// its own type by them -- reading them as references pointed 10756 of
/// cats' 13896 at whatever happened to be called `A`.
fn names_a_type_parameter(label: &str) -> bool {
    let label = label.trim();
    if label.is_empty() {
        return true;
    }
    let mut characters = label.chars();
    let first = characters.next().unwrap_or(' ');
    first.is_ascii_uppercase() && characters.all(|character| character.is_ascii_digit())
}

/// The names one declaration writes to reach another: a Dart type, a
/// Terraform address, a Nix option, a schema's field type. Kept out of
/// [`collect_items`] so its stack frame stays small enough for the depth
/// cap to hold on a deeply nested file.
#[inline(never)]
fn collect_reference_facts(
    context: &WalkContext<'_>,
    node: Node<'_>,
    current_function: Option<&str>,
    facts: &mut CollectedFacts,
) {
    let WalkContext {
        language,
        source,
        path,
        config_aliases,
    } = *context;
    let current_function = current_function.map(ToString::to_string);
    if language == Language::Dart
        && node.kind() == "type_identifier"
        && let Some(label) = node_text(node, source)
        && !label.is_empty()
    {
        facts.type_references.push(ParsedTypeReference {
            label,
            span: span_for(path, node),
            parent: current_function.clone(),
        });
    }

    // What a declaration states about the types it works with: a
    // parameter's annotation, a property's, a return type, a generic
    // argument, the interfaces a class extends or implements, and the type
    // an `impl` block is written for. Without them vue's
    // `ComponentInternalInstance` -- the interface its whole runtime is
    // written against -- had nothing pointing at it, and so did two thirds
    // of gson's classes and six sevenths of ripgrep's types.
    if matches!(
        language,
        Language::TypeScript
            | Language::Tsx
            | Language::Java
            | Language::Rust
            | Language::Go
            // C and C++ name a type the same way: `struct client *c`, a
            // parameter's type, what a function returns. redis declares
            // 3635 types and 39 references pointed into them, and
            // nlohmann/json 940 with 87.
            | Language::C
            | Language::Cpp
    ) && node.kind() == "type_identifier"
        // The name in `interface Foo {}` declares the type rather than
        // referring to one, and the declaration is already a node. In C
        // the same shape says both: `struct client { .. }` declares the
        // type and `struct client *c` names it, and only the body tells
        // them apart -- without that, every use of redis's `client`,
        // `robj` and `redisCommand` read as another declaration.
        && !node.parent().is_some_and(|parent| {
            parent.child_by_field_name("name") == Some(node)
                && (!matches!(
                    parent.kind(),
                    "struct_specifier" | "union_specifier" | "enum_specifier" | "class_specifier"
                ) || parent.child_by_field_name("body").is_some())
        })
        && let Some(label) = node_text(node, source)
        && !names_a_type_parameter(&label)
    {
        facts.type_references.push(ParsedTypeReference {
            label,
            span: span_for(path, node),
            parent: current_function.clone(),
        });
    }

    // C# writes its types as plain identifiers, so the node kind alone
    // cannot find them: what a declaration states is in its `type` field --
    // a field's, a property's, a parameter's, a method's return -- and the
    // classes a type derives from are its base list.
    if language == Language::CSharp {
        let mut cursor = node.walk();
        let named: Vec<Node<'_>> = if node.kind() == "base_list" {
            node.named_children(&mut cursor).collect()
        } else {
            node.child_by_field_name("type").into_iter().collect()
        };
        for type_node in named {
            // `int`, `string` and `var` are the language's own, and its
            // grammar gives them a kind of their own.
            if matches!(type_node.kind(), "predefined_type" | "implicit_type") {
                continue;
            }
            if let Some(label) = node_text(type_node, source) {
                // `List<Policy>` names the generic and its argument; the
                // argument is what a reader follows, and the generic's own
                // children are walked anyway.
                let label = label
                    .split(['<', '[', '?', '('])
                    .next()
                    .unwrap_or("")
                    .trim();
                let label = label.rsplit('.').next().unwrap_or(label).trim();
                let names_a_type = !label.is_empty()
                    && label
                        .chars()
                        .next()
                        .is_some_and(|first| first.is_ascii_uppercase());
                if names_a_type {
                    facts.type_references.push(ParsedTypeReference {
                        label: label.to_string(),
                        span: span_for(path, type_node),
                        parent: current_function.clone(),
                    });
                }
            }
        }
    }

    // What a Python declaration states about the classes it works with:
    // the classes it inherits and the types it annotates. django-oscar
    // declares 1697 classes and 14% of them had anything pointing at
    // them, because a Django project states its structure through
    // inheritance -- `class Basket(AbstractBasket)` -- and nothing read
    // it.
    if language == Language::Python {
        let mut references: Vec<Node<'_>> = Vec::new();
        if node.kind() == "class_definition"
            && let Some(bases) = node.child_by_field_name("superclasses")
        {
            let mut cursor = bases.walk();
            references.extend(
                bases
                    .named_children(&mut cursor)
                    .filter(|child| matches!(child.kind(), "identifier" | "attribute")),
            );
        }
        // `def add(self, product: Product) -> Line:` names both.
        if node.kind() == "type" {
            let mut stack = vec![node];
            let mut visited = 0;
            while let Some(current) = stack.pop() {
                visited += 1;
                if visited > 64 {
                    break;
                }
                if matches!(current.kind(), "identifier" | "attribute") {
                    references.push(current);
                    continue;
                }
                let mut cursor = current.walk();
                stack.extend(current.named_children(&mut cursor));
            }
        }
        for reference in references {
            let Some(label) = node_text(reference, source) else {
                continue;
            };
            // `models.Model` names `Model`, the way a namespace does.
            let label = label
                .trim()
                .rsplit('.')
                .next()
                .unwrap_or_default()
                .trim()
                .to_string();
            if label.is_empty() || python_builtin_type_name(&label) {
                continue;
            }
            facts.type_references.push(ParsedTypeReference {
                label,
                span: span_for(path, reference),
                parent: current_function.clone(),
            });
        }
    }

    // What a Haskell signature states: `checkX :: Parameters -> Token ->
    // [TokenComment]` names the types the function works with, and a type
    // is an uppercase constructor. shellcheck writes 3663 definitions and
    // nothing pointed at any of its types.
    if language == Language::Haskell
        && node.kind() == "name"
        && let Some(label) = node_text(node, source)
        && label.starts_with(char::is_uppercase)
        && !names_a_type_parameter(&label)
        // The name in `data Parameters = ..` declares the type.
        && !node
            .parent()
            .and_then(|parent| parent.child_by_field_name("name"))
            .is_some_and(|name| name == node)
    {
        facts.type_references.push(ParsedTypeReference {
            label,
            span: span_for(path, node),
            parent: current_function.clone(),
        });
    }

    // What a Julia declaration states: `df::AbstractDataFrame` names the
    // type a value has, and `struct DataFrame <: AbstractDataFrame` the
    // type it specialises.
    if language == Language::Julia && matches!(node.kind(), "typed_expression" | "subtype_clause") {
        let mut cursor = node.walk();
        let children: Vec<Node<'_>> = node.named_children(&mut cursor).collect();
        if let Some(last) = children.last()
            && matches!(last.kind(), "identifier" | "field_expression")
            && let Some(label) = node_text(*last, source)
        {
            let label = label.trim();
            if label.starts_with(char::is_uppercase) && !names_a_type_parameter(label) {
                facts.type_references.push(ParsedTypeReference {
                    label: label.to_string(),
                    span: span_for(path, *last),
                    parent: current_function.clone(),
                });
            }
        }
    }

    // What a Swift declaration states about the types it works with: a
    // property's type, a parameter's, what a function returns, and what a
    // type conforms to. Alamofire declares `Session` -- the type its whole
    // API is written around -- and nothing pointed at it.
    if language == Language::Swift
        && node.kind() == "type_identifier"
        && let Some(label) = node_text(node, source)
        && !names_a_type_parameter(&label)
        // The name in `struct Session { .. }` declares the type.
        && !node
            .parent()
            .and_then(|parent| parent.child_by_field_name("name"))
            .is_some_and(|name| name == node)
    {
        facts.type_references.push(ParsedTypeReference {
            label,
            span: span_for(path, node),
            parent: current_function.clone(),
        });
    }

    // What an Erlang module states about the modules it calls:
    // `cowboy_req:reply(..)` names the module on the left of the colon,
    // and `-behaviour(cowboy_handler)` names the one it implements.
    if language == Language::Erlang {
        let mut references: Vec<Node<'_>> = Vec::new();
        if node.kind() == "remote"
            && let Some(module) = node.child_by_field_name("module")
        {
            references.push(module);
        }
        // `-behaviour(cowboy_handler).` names the module it implements.
        if node.kind() == "behaviour_attribute"
            && let Some(name) = node.named_child(0)
        {
            references.push(name);
        }
        for reference in references {
            let Some(label) = node_text(reference, source) else {
                continue;
            };
            let label = label.trim().trim_end_matches(':').trim();
            if label.is_empty()
                || !label
                    .chars()
                    .all(|character| character.is_alphanumeric() || character == '_')
            {
                continue;
            }
            facts.type_references.push(ParsedTypeReference {
                label: label.to_string(),
                span: span_for(path, reference),
                parent: current_function.clone(),
            });
        }
    }

    // What an Elixir module states about the modules it works with:
    // `alias Ecto.Changeset`, `use Ecto.Schema`, `import Ecto.Query`, and
    // the module a qualified call is written through --
    // `Changeset.change(..)`. ecto declares 390 modules and nothing
    // pointed at any of them.
    if language == Language::Elixir && node.kind() == "call" {
        let target = elixir_call_target(node, source);
        let mut references: Vec<Node<'_>> = Vec::new();
        if target
            .as_deref()
            .is_some_and(|target| matches!(target, "alias" | "import" | "use" | "require"))
            && let Some(arguments) = node
                .named_children(&mut node.walk())
                .find(|child| child.kind() == "arguments")
        {
            let mut cursor = arguments.walk();
            references.extend(
                arguments
                    .named_children(&mut cursor)
                    .filter(|child| child.kind() == "alias"),
            );
        }
        // `Changeset.change(x)` names the module on the left of the dot.
        if let Some(dot) = node
            .named_children(&mut node.walk())
            .find(|child| child.kind() == "dot")
            && let Some(left) = dot.named_child(0)
            && left.kind() == "alias"
        {
            references.push(left);
        }
        for reference in references {
            let Some(label) = node_text(reference, source) else {
                continue;
            };
            let label = label.trim();
            if label.is_empty() || !label.starts_with(char::is_uppercase) {
                continue;
            }
            facts.type_references.push(ParsedTypeReference {
                label: label.to_string(),
                span: span_for(path, reference),
                parent: current_function.clone(),
            });
        }
    }

    // What a Kotlin declaration states about the types it works with: a
    // parameter's type, a property's, what a function returns, and what a
    // class extends or implements. okio declares 358 types and four
    // references pointed into them, so "what breaks if I change `Buffer`"
    // -- the type its whole API is written around -- answered with
    // nothing.
    if language == Language::Kotlin && node.kind() == "user_type" {
        let mut cursor = node.walk();
        if let Some(name) = node
            .named_children(&mut cursor)
            .find(|child| child.kind() == "identifier")
            && let Some(label) = node_text(name, source)
        {
            let label = label.trim();
            // `T`, `R`, `K1` name a type parameter, and every generic
            // declaration writes one.
            if !label.is_empty() && !names_a_type_parameter(label) {
                facts.type_references.push(ParsedTypeReference {
                    label: label.to_string(),
                    span: span_for(path, name),
                    parent: current_function.clone(),
                });
            }
        }
    }

    // What a Ruby class states about the classes it works with: the class
    // it inherits from, the modules it mixes in, and the constant a call
    // is written through. mastodon declares 2083 classes and modules and
    // nothing pointed at any of them, so "what breaks if I change
    // `Account`" answered with nothing at all.
    if language == Language::Ruby {
        let mut references: Vec<Node<'_>> = Vec::new();
        if node.kind() == "superclass" {
            let mut cursor = node.walk();
            references.extend(
                node.named_children(&mut cursor)
                    .filter(|child| matches!(child.kind(), "constant" | "scope_resolution")),
            );
        }
        // `include Rememberable`, `extend Forwardable`, `prepend Sanitize`.
        if node.kind() == "call"
            && named_child_text(node, "method", source)
                .as_deref()
                .is_some_and(|method| matches!(method, "include" | "extend" | "prepend"))
            && node.child_by_field_name("receiver").is_none()
            && let Some(arguments) = node.child_by_field_name("arguments")
        {
            let mut cursor = arguments.walk();
            references.extend(
                arguments
                    .named_children(&mut cursor)
                    .filter(|child| matches!(child.kind(), "constant" | "scope_resolution")),
            );
        }
        // `Account.find(id)` reaches the class through its name, which is
        // the way a Ruby program names most of the classes it uses.
        if node.kind() == "call"
            && let Some(receiver) = node.child_by_field_name("receiver")
            && matches!(receiver.kind(), "constant" | "scope_resolution")
        {
            references.push(receiver);
        }
        for reference in references {
            let Some(label) = node_text(reference, source) else {
                continue;
            };
            // `Admin::AccountsController` names that class and not the
            // `AccountsController` beside it; the resolver matches the
            // tail as well, so a bare name still finds a nested class.
            let label = label.trim();
            if label.is_empty()
                || !label
                    .chars()
                    .next()
                    .is_some_and(|character| character.is_ascii_uppercase())
            {
                continue;
            }
            facts.type_references.push(ParsedTypeReference {
                label: label.to_string(),
                span: span_for(path, reference),
                parent: current_function.clone(),
            });
        }
    }

    // What a PHP declaration states about the classes it works with: the
    // type of a parameter or property, the type it returns, the class it
    // extends and the interfaces it implements. Laravel builds a service
    // from its constructor's type hints rather than with `new`, so without
    // these koel's 1319 classes had almost nothing pointing at them.
    // `AlbumController::class` is how PHP writes down a class it does not
    // build: Laravel routes name their controller that way, a container
    // names what it binds, and a config file names its providers. koel
    // writes 111 of them in its routes alone, and nothing pointed at the
    // classes they name.
    if language == Language::Php
        && matches!(
            node.kind(),
            "named_type"
                | "base_clause"
                | "class_interface_clause"
                | "class_constant_access_expression"
        )
    {
        let mut cursor = node.walk();
        let names: Vec<Node<'_>> = if node.kind() == "named_type" {
            vec![node]
        } else if node.kind() == "class_constant_access_expression" {
            // `Foo::class` names Foo; `self::class` and `$this::class`
            // name whatever is already being read.
            node.named_child(0)
                .filter(|name| matches!(name.kind(), "name" | "qualified_name"))
                .into_iter()
                .collect()
        } else {
            node.named_children(&mut cursor).collect()
        };
        for name in names {
            if let Some(label) = node_text(name, source) {
                let label = label.trim().trim_start_matches('\\');
                let label = label.rsplit('\\').next().unwrap_or(label).trim();
                // A builtin type is the language's: `string`, `int`, `void`.
                let names_a_class = !label.is_empty()
                    && label
                        .chars()
                        .next()
                        .is_some_and(|first| first.is_ascii_uppercase());
                if names_a_class {
                    facts.type_references.push(ParsedTypeReference {
                        label: label.to_string(),
                        span: span_for(path, name),
                        parent: current_function.clone(),
                    });
                }
            }
        }
    }

    // What one contract states of another: the contract it inherits, the
    // library it uses, the type it holds.
    if language == Language::Solidity
        && node.kind() == "user_defined_type"
        && let Some(label) = node_text(node, source)
        && !label.is_empty()
    {
        facts.type_references.push(ParsedTypeReference {
            label: simple_name(&label).to_string(),
            span: span_for(path, node),
            parent: current_function.clone(),
        });
    }

    // A schema is a graph of its own: a field states the message or the
    // type it carries, and following that name is following the schema.
    if matches!(language, Language::Proto | Language::GraphQl)
        && matches!(node.kind(), "message_or_enum_type" | "named_type")
        && let Some(label) = node_text(node, source)
        && !label.is_empty()
    {
        facts.type_references.push(ParsedTypeReference {
            // `google.protobuf.Timestamp` is that package's message; the
            // name a schema in this repository declares is the last part.
            label: simple_name(&label).to_string(),
            span: span_for(path, node),
            parent: current_function.clone(),
        });
    }

    // What a module reads is the other half of what it declares:
    // `config.programs.git.enable`, and `cfg.enable` where the file bound
    // `cfg = config.programs.git`.
    if language == Language::Nix
        && node.kind() == "select_expression"
        && let Some(label) = nix_option_reference(node, source, config_aliases)
    {
        facts.type_references.push(ParsedTypeReference {
            label,
            span: span_for(path, node),
            parent: current_function.clone(),
        });
    }

    // A configuration is a graph because its declarations name each other:
    // `subnet_id = module.vpc.id` is what ties a resource to a module, and
    // `var.region` is what ties it to an input.
    if language == Language::Hcl
        && node.kind() == "expression"
        && let Some(label) = hcl_reference_address(node, source)
    {
        facts.type_references.push(ParsedTypeReference {
            label,
            span: span_for(path, node),
            parent: current_function.clone(),
        });
    }
}

pub(crate) fn collect_items(
    context: &WalkContext<'_>,
    node: Node<'_>,
    current_function: Option<String>,
    scope: &DefinitionScope,
    facts: &mut CollectedFacts,
    depth: usize,
    deferred: bool,
) {
    let WalkContext {
        language,
        source,
        path,
        config_aliases: _,
    } = *context;
    if depth >= MAX_TREE_DEPTH {
        return;
    }
    if let Some(effect) = classify_effect(language, node, source, path, current_function.as_deref())
    {
        facts.items.push(effect);
    }
    if let Some(control_flow) =
        classify_control_flow(language, node, source, path, current_function.as_deref())
    {
        facts.items.push(control_flow);
    }

    // A call outside any definition still happens: module initialisers,
    // registration calls and whole script bodies run at load time. Those
    // belong to the file, so the parent stays open rather than the call
    // being dropped — unless the call sits in an unnamed callback, which
    // runs when something invokes it and not when the file loads.
    if (current_function.is_some() || !deferred)
        && let Some(call) = classify_call(
            language,
            node,
            source,
            path,
            current_function.as_deref(),
            scope,
        )
    {
        facts.items.push(call);
    }

    collect_reference_facts(context, node, current_function.as_deref(), facts);

    let mut next_function = current_function;
    let mut next_scope: Option<DefinitionScope> = None;
    if let Some(mut item) = classify_node(language, node, source, path) {
        // HCL has no functions, and a schema's fields belong to the type
        // that states them: what a fact sits inside is the declaration that
        // holds it, so `file(..)` in a resource belongs to that resource and
        // `Address address = 3;` belongs to the message.
        if matches!(
            language,
            Language::Hcl | Language::Proto | Language::GraphQl | Language::Solidity
        ) && matches!(item.kind, ParsedItemKind::Type | ParsedItemKind::Module)
        {
            next_function = Some(item.label.clone());
        }
        if matches!(
            item.kind,
            ParsedItemKind::Function | ParsedItemKind::Entrypoint
        ) {
            // A definition nested in another one is visible only inside it —
            // a Haskell `where` binding, a local `fn`, a closure bound to a
            // name. Recording which definition encloses it lets call
            // resolution stop treating 167 unrelated local `f`s as candidates
            // for one call.
            if let Some(enclosing) = next_function.as_deref()
                && !binds_an_outer_name(language, node)
            {
                item.metadata
                    .insert("enclosing_function".to_string(), enclosing.to_string());
            }
            next_function = Some(item.label.clone());
            next_scope = Some(definition_scope(language, node, source));
        }
        facts.items.push(item);
    }

    // A Haskell data constructor is a function — `T_Literal :: Id ->
    // String -> Token` — and code applies it like one. Only the type it
    // belongs to was recorded, so shellcheck's 1474 constructor
    // applications had nothing to point at.
    if language == Language::Haskell {
        facts
            .items
            .extend(haskell_data_constructors(node, source, path));
    }

    let next_scope = next_scope.as_ref().unwrap_or(scope);
    let next_deferred = deferred || is_deferred_body(language, node.kind(), path);
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_items(
            context,
            child,
            next_function.clone(),
            next_scope,
            facts,
            depth + 1,
            next_deferred,
        );
    }
}

/// The node a call invokes, following the same precedence `call_label`
/// uses to read its text. Parentheses around it are unwrapped: an
/// immediately-invoked function is written `(function () { … })()` in
/// most languages, so the literal sits one level down.
fn call_callee<'tree>(language: Language, node: Node<'tree>) -> Option<Node<'tree>> {
    let mut callee = match language {
        Language::Kotlin | Language::Swift | Language::Julia => node.named_child(0),
        // Lua names the field `name`, which `call_label` also falls back to.
        _ => node
            .child_by_field_name("function")
            .or_else(|| node.child_by_field_name("name")),
    }?;
    for _ in 0..MAX_PARENTHESIS_DEPTH {
        if !callee.kind().contains("parenthesized") {
            break;
        }
        let Some(inner) = callee.named_child(0) else {
            break;
        };
        callee = inner;
    }
    Some(callee)
}

/// How many layers of `((f))` to look through before giving up. Deeper
/// than this is not real code, and the loop must end.
const MAX_PARENTHESIS_DEPTH: usize = 8;

/// The constructors a `data` or `newtype` declaration introduces, each
/// carrying the type it builds. Only the subtree under the declaration's
/// constructor field is read, so the names in a `deriving` clause are not
/// mistaken for constructors.
fn haskell_data_constructors(node: Node<'_>, source: &[u8], path: &str) -> Vec<ParsedItem> {
    if !matches!(node.kind(), "data_type" | "newtype") {
        return Vec::new();
    }
    let Some(owner) = node
        .child_by_field_name("name")
        .and_then(|name| node_text(name, source))
    else {
        return Vec::new();
    };
    let Some(constructors) = node
        .child_by_field_name("constructors")
        .or_else(|| node.child_by_field_name("constructor"))
    else {
        return Vec::new();
    };

    let mut items = Vec::new();
    let mut stack = vec![constructors];
    while let Some(current) = stack.pop() {
        if current.kind() == "constructor"
            && let Some(label) = node_text(current, source)
            && !label.is_empty()
        {
            items.push(ParsedItem {
                kind: ParsedItemKind::Function,
                label,
                span: span_for(path, current),
                parent: None,
                metadata: BTreeMap::from([("owner_type".to_string(), owner.clone())]),
            });
            continue;
        }
        let mut cursor = current.walk();
        stack.extend(current.named_children(&mut cursor));
    }
    items.sort_by_key(|item| item.span.start_line);
    items
}

/// A callable with no name of its own: a closure, a lambda, a block passed
/// to a method. Its statements run when something invokes it, so a call
/// inside it belongs to that unnamed function — not to the file that
/// happens to hold it. Named callables never reach here, because a call
/// inside one already carries that name as its parent.
fn is_deferred_body(language: Language, kind: &str, path: &str) -> bool {
    // Ruby writes callbacks as blocks, and only Ruby calls those nodes
    // `block` — elsewhere `block` is an ordinary statement list, and a
    // Python `if __name__ == "__main__":` body is exactly the load-time
    // code this must keep.
    if language == Language::Ruby && matches!(kind, "block" | "do_block") {
        // A spec IS its blocks: `describe .. do it .. do expect(..) end
        // end` is what the file runs, and dropping them left mastodon's
        // 1312 spec files with 1897 calls between them -- so "which tests
        // cover this" had almost nothing to answer with.
        return !is_test_like_source_path(path);
    }
    // Lua splits the two: `function foo() end` is a `function_declaration`,
    // and only the anonymous `function() end` is a `function_definition` —
    // a kind that names the ordinary declaration in Python and C.
    if language == Language::Lua && kind == "function_definition" {
        return true;
    }
    let anonymous = kind.contains("lambda")
        || kind.contains("closure")
        || kind.contains("anonymous")
        || matches!(
            kind,
            "arrow_function" | "function_expression" | "func_literal" | "fn"
        );
    // A JavaScript spec is written the same way as a Ruby one: `describe
    // ('x', () => { it('y', () => { service.load() }) })` puts every call
    // the test makes inside an anonymous function, and koel's 498 spec
    // files made 1456 calls between them.
    if anonymous
        && matches!(
            language,
            Language::JavaScript | Language::TypeScript | Language::Tsx
        )
        && is_test_like_source_path(path)
    {
        return false;
    }
    anonymous
}

/// Whether a C/C++ declaration is named by a word the language reserves,
/// which no real declaration can be.
fn names_a_c_keyword(node: Node<'_>, source: &[u8]) -> bool {
    let Some(name) = first_identifier_in_field(node, "declarator", source)
        .or_else(|| first_identifier(node, source))
    else {
        return false;
    };
    matches!(
        name.as_str(),
        "namespace"
            | "class"
            | "struct"
            | "union"
            | "enum"
            | "template"
            | "typename"
            | "return"
            | "if"
            | "else"
            | "for"
            | "while"
            | "switch"
            | "case"
            | "break"
            | "continue"
            | "const"
            | "static"
            | "inline"
            | "virtual"
            | "public"
            | "private"
            | "protected"
            | "using"
            | "typedef"
            | "sizeof"
            | "delete"
            | "this"
            | "throw"
            | "try"
            | "catch"
            | "extern"
            | "goto"
            | "default"
            | "void"
            | "auto"
            | "constexpr"
            | "noexcept"
    )
}

pub(crate) fn classify_node(
    language: Language,
    node: Node<'_>,
    source: &[u8],
    path: &str,
) -> Option<ParsedItem> {
    // An anonymous node is a literal token, and its kind is the text it
    // holds: Kotlin's `import` keyword answers to the same kind as the
    // import statement around it, and okio filed 2183 facts that were the
    // word `import` and nothing else. A declaration is always a named node.
    if !node.is_named() {
        return None;
    }
    let kind = node.kind();
    let item_kind = match language {
        Language::Rust => match kind {
            "function_item" => ParsedItemKind::Function,
            "struct_item" | "enum_item" | "trait_item" | "type_item" | "union_item" => {
                ParsedItemKind::Type
            }
            "mod_item" => ParsedItemKind::Module,
            "use_declaration" => ParsedItemKind::Import,
            _ => return None,
        },
        Language::Python => match kind {
            "function_definition" => ParsedItemKind::Function,
            "class_definition" => ParsedItemKind::Type,
            "import_statement" | "import_from_statement" => ParsedItemKind::Import,
            _ => return None,
        },
        Language::JavaScript | Language::TypeScript | Language::Tsx => match kind {
            "function_declaration" | "method_definition" | "generator_function_declaration" => {
                ParsedItemKind::Function
            }
            // `const handler = () => {}` and `const f = function () {}` are how
            // most modern JS/TS declares functions. Only named bindings count;
            // an anonymous callback stays anonymous.
            "arrow_function" | "function_expression"
                if js_bound_function_name(node, source).is_some() =>
            {
                ParsedItemKind::Function
            }
            // `export const onMounted = createHook(MOUNTED)` and `const
            // buttonVariants = cva(..)`: a factory hands back a value the
            // module exports and other files call. Vue declares most of
            // its public API that way, and 523 calls into it resolved to
            // nothing because the declaration was not in the graph.
            "variable_declarator" if js_value_declaration_name(node, source).is_some() => {
                ParsedItemKind::Function
            }
            "class_declaration"
            | "interface_declaration"
            | "type_alias_declaration"
            | "enum_declaration" => ParsedItemKind::Type,
            "import_statement" => ParsedItemKind::Import,
            // A route loads its page with `import('./Home.vue')`, and that
            // is an import like any other.
            "call_expression" if js_dynamic_import_specifier(node, source).is_some() => {
                ParsedItemKind::Import
            }
            _ => return None,
        },
        Language::Go => match kind {
            "function_declaration" | "method_declaration" => ParsedItemKind::Function,
            "type_declaration" => ParsedItemKind::Type,
            "import_spec" => ParsedItemKind::Import,
            _ => return None,
        },
        // Objective-C states an interface and then implements it: a class
        // is named once in each, and its methods are named by selector —
        // `initWithBaseURL:sessionConfiguration:` is one name, and it is
        // what a caller writes.
        Language::ObjectiveC => match kind {
            "class_interface" | "class_implementation" | "protocol_declaration" => {
                ParsedItemKind::Type
            }
            "struct_specifier" | "union_specifier" | "enum_specifier" | "type_definition" => {
                ParsedItemKind::Type
            }
            "method_declaration" | "method_definition" => ParsedItemKind::Function,
            "function_definition" => ParsedItemKind::Function,
            "preproc_function_def" => ParsedItemKind::Function,
            "preproc_include" | "module_import" => ParsedItemKind::Import,
            _ => return None,
        },
        Language::C | Language::Cpp => match kind {
            // A macro the parser has never seen turns the code after it into
            // a shape it can recognise: `NLOHMANN_JSON_NAMESPACE_BEGIN` in
            // front of `namespace detail { … }` reads as a function named
            // `namespace` covering 574 lines. No C++ declaration is named by
            // a reserved word, so that is a misparse rather than a fact.
            "function_definition" if names_a_c_keyword(node, source) => return None,
            // `class SPDLOG_API logger { .. }` puts an export macro where
            // the grammar expects the name, and the whole declaration
            // reads as a function returning `class SPDLOG_API` called
            // `logger`. spdlog's central class had no node at all, and
            // every class a library exports this way is written like it.
            "function_definition" if names_an_exported_c_class(node) => ParsedItemKind::Type,
            "function_definition" => ParsedItemKind::Function,
            // `#define serverAssert(x) …` defines something the code calls
            // like a function, and redis calls 7300 of them. An object-like
            // `#define LIMIT 10` is a value, not a callable, and stays out.
            "preproc_function_def" => ParsedItemKind::Function,
            // `struct client *c;` names a type; `struct client { .. }`
            // declares one. Reading both as declarations gave redis 183
            // nodes for `redisCommand` and 3635 types for its 1492 names,
            // so no reference to any of them could choose a target.
            "struct_specifier" | "union_specifier" | "enum_specifier" | "class_specifier"
                if node.child_by_field_name("body").is_none() =>
            {
                return None;
            }
            // `typedef struct client { .. } client;` is one declaration
            // written twice, and the name a program uses is the typedef's.
            "struct_specifier" | "union_specifier" | "enum_specifier"
                if node
                    .parent()
                    .is_some_and(|parent| parent.kind() == "type_definition") =>
            {
                return None;
            }
            "struct_specifier" | "union_specifier" | "enum_specifier" | "class_specifier"
            | "type_definition" => ParsedItemKind::Type,
            "namespace_definition" => ParsedItemKind::Module,
            "preproc_include" => ParsedItemKind::Import,
            _ => return None,
        },
        Language::Php => match kind {
            "function_definition" | "method_declaration" => ParsedItemKind::Function,
            "class_declaration"
            | "interface_declaration"
            | "trait_declaration"
            | "enum_declaration" => ParsedItemKind::Type,
            "namespace_definition" => ParsedItemKind::Module,
            "require_expression"
            | "include_expression"
            | "include_once_expression"
            | "require_once_expression"
            | "namespace_use_declaration" => ParsedItemKind::Import,
            _ => return None,
        },
        Language::Dart => match kind {
            "function_declaration"
            | "method_declaration"
            | "constructor_signature"
            | "factory_constructor_signature" => ParsedItemKind::Function,
            "class_declaration"
            | "mixin_declaration"
            | "extension_declaration"
            | "extension_type_declaration"
            | "enum_declaration"
            | "type_alias" => ParsedItemKind::Type,
            "library_name" => ParsedItemKind::Module,
            "import_or_export" | "part_directive" | "part_of_directive" => ParsedItemKind::Import,
            _ => return None,
        },
        Language::Bash => match kind {
            "function_definition" => ParsedItemKind::Function,
            "command" if command_text_starts_with(source, node, &["source", "."]) => {
                ParsedItemKind::Import
            }
            _ => return None,
        },
        Language::Ruby => match kind {
            "method" | "singleton_method" => ParsedItemKind::Function,
            "class" => ParsedItemKind::Type,
            "module" => ParsedItemKind::Module,
            "call" if ruby_require_call(node, source) => ParsedItemKind::Import,
            _ => return None,
        },
        Language::Java => match kind {
            "method_declaration" | "constructor_declaration" => ParsedItemKind::Function,
            "class_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "record_declaration"
            | "annotation_type_declaration" => ParsedItemKind::Type,
            "import_declaration" => ParsedItemKind::Import,
            _ => return None,
        },
        Language::CSharp => match kind {
            "method_declaration" | "constructor_declaration" | "local_function_statement" => {
                ParsedItemKind::Function
            }
            "class_declaration"
            | "interface_declaration"
            | "struct_declaration"
            | "enum_declaration"
            | "record_declaration" => ParsedItemKind::Type,
            "namespace_declaration" | "file_scoped_namespace_declaration" => ParsedItemKind::Module,
            "using_directive" => ParsedItemKind::Import,
            _ => return None,
        },
        Language::Kotlin => match kind {
            "function_declaration" => ParsedItemKind::Function,
            "class_declaration" | "object_declaration" => ParsedItemKind::Type,
            "import" => ParsedItemKind::Import,
            _ => return None,
        },
        Language::Swift => match kind {
            "function_declaration" | "protocol_function_declaration" | "init_declaration" => {
                ParsedItemKind::Function
            }
            // `extension Session { .. }` adds to a type declared
            // elsewhere; it does not declare one. Alamofire writes three
            // for `Session` alone, so the type its whole API is written
            // around had four declarations and no reference could choose
            // between them.
            "class_declaration"
                if node
                    .child_by_field_name("declaration_kind")
                    .and_then(|kind| node_text(kind, source))
                    .as_deref()
                    .map(str::trim)
                    == Some("extension") =>
            {
                return None;
            }
            // The grammar folds struct/enum into class_declaration.
            "class_declaration" | "protocol_declaration" => ParsedItemKind::Type,
            "import_declaration" => ParsedItemKind::Import,
            _ => return None,
        },
        Language::Scala => match kind {
            "function_definition" | "function_declaration" => ParsedItemKind::Function,
            "class_definition" | "object_definition" | "trait_definition" | "enum_definition" => {
                ParsedItemKind::Type
            }
            "import_declaration" => ParsedItemKind::Import,
            _ => return None,
        },
        Language::Lua => match kind {
            "function_declaration" => ParsedItemKind::Function,
            // `M.handle = function() … end` and `{ init = function() … end }`
            // are how Lua modules are written; the binding names the
            // function even though the literal does not. Without this the
            // whole body has no definition to belong to.
            "function_definition" if lua_bound_function_name(node, source).is_some() => {
                ParsedItemKind::Function
            }
            "function_call" if lua_require_call(node, source) => ParsedItemKind::Import,
            _ => return None,
        },
        // Elixir syntax is uniform: everything is a `call` whose target picks
        // the special form (defmodule/def/import/...); see elixir_call_target.
        Language::Elixir => match elixir_call_target(node, source).as_deref() {
            Some("defmodule") => ParsedItemKind::Module,
            Some(
                "def" | "defp" | "defmacro" | "defmacrop" | "defdelegate" | "defguard"
                | "defguardp",
            ) => ParsedItemKind::Function,
            // `defstruct` states the shape of the module it sits in, and
            // takes that module's name: ecto declared `Ecto.Changeset`
            // twice, so every reference to it was ambiguous and dropped.
            Some("defstruct") => return None,
            Some("defprotocol" | "defimpl") => ParsedItemKind::Type,
            Some("import" | "require" | "use" | "alias") => ParsedItemKind::Import,
            _ => return None,
        },
        Language::Zig => match kind {
            "function_declaration" => ParsedItemKind::Function,
            "builtin_function" if zig_import_builtin(node, source) => ParsedItemKind::Import,
            // A Zig type is a constant bound to a container: `const Server =
            // struct { ... }`. zls declares 260 of them and the graph had
            // no types for the language at all.
            "variable_declaration" if zig_container_declaration(node) => ParsedItemKind::Type,
            _ => return None,
        },
        Language::Haskell => match kind {
            // `function` has argument patterns; `bind` is a plain definition
            // (`main = do ...`), which is still a top-level callable. The same
            // `function` kind also spells a function *type* (`Token -> m ()`)
            // inside a signature — that node has no `name` field, and without
            // this guard its first type identifier was recorded as a function.
            "function" | "bind" if node.child_by_field_name("name").is_some() => {
                ParsedItemKind::Function
            }
            "data_type" | "newtype" | "type_synonym" | "class" => ParsedItemKind::Type,
            "import" => ParsedItemKind::Import,
            _ => return None,
        },
        Language::OCaml => match kind {
            // A let binding is a function only when it takes parameters.
            // `parameter` is a plain child node here, not a named field.
            "let_binding" if ocaml_binding_has_parameter(node) => ParsedItemKind::Function,
            "type_binding" => ParsedItemKind::Type,
            "module_definition" => ParsedItemKind::Module,
            "open_module" | "include_module" => ParsedItemKind::Import,
            _ => return None,
        },
        Language::Erlang => match kind {
            "fun_decl" => ParsedItemKind::Function,
            "module_attribute" => ParsedItemKind::Module,
            // `-record(state, {...})` is the shape a module passes around
            // and `-type`/`-opaque` name one; cowboy declares 87 of them.
            "record_decl" | "type_alias" | "opaque" => ParsedItemKind::Type,
            // `-include("x.hrl")` pulls in a file and `-import(lists, [...])`
            // names a module: cowboy writes 154 of them.
            "pp_include" | "import_attribute" => ParsedItemKind::Import,
            _ => return None,
        },
        // Nix has no functions or types as such; a binding whose value is a
        // lambda is the closest thing to a named callable, and `import` calls
        // pull in other expressions.
        Language::Nix => match kind {
            // What a module offers to configure, which is what a reader of
            // an option-driven configuration is looking for.
            "binding" if nix_option_declaration(node, source) => ParsedItemKind::Type,
            "binding" if nix_binding_is_function(node) => ParsedItemKind::Function,
            // `import ./helper.nix { ... }` is how one Nix file pulls in
            // another; home-manager writes 253 of them and none was a fact.
            "apply_expression" if nix_import_expression(node, source) => ParsedItemKind::Import,
            _ => return None,
        },
        // R declares functions by assigning a lambda: `run <- function(x) ...`.
        Language::R => match kind {
            "binary_operator" if r_assignment_defines_function(node) => ParsedItemKind::Function,
            "call" if r_library_call(node, source) => ParsedItemKind::Import,
            _ => return None,
        },
        Language::Julia => match kind {
            "function_definition" | "macro_definition" => ParsedItemKind::Function,
            // Short form: `square(x) = x * x` parses as an assignment whose
            // left side is a call expression. Without this it was not a
            // definition at all, and its left side counted as a call.
            "assignment" if julia_short_function_definition(node) => ParsedItemKind::Function,
            "struct_definition" | "abstract_definition" | "primitive_definition" => {
                ParsedItemKind::Type
            }
            "module_definition" => ParsedItemKind::Module,
            "using_statement" | "import_statement" => ParsedItemKind::Import,
            // `include("abstractdataframe.jl")` splices a file into the
            // module; DataFrames.jl builds itself from 35 of them.
            "call_expression" if julia_include_call(node, source) => ParsedItemKind::Import,
            _ => return None,
        },
        // HCL declares by block: `resource "aws_instance" "web" { .. }`. A
        // block at the top level of a file declares something the rest of
        // the configuration addresses by name; one nested inside it is that
        // declaration's own settings and not a thing of its own.
        Language::Hcl => match kind {
            "block" if hcl_block_declares(node, source) => {
                if hcl_block_type(node, source).as_deref() == Some("module") {
                    ParsedItemKind::Module
                } else {
                    ParsedItemKind::Type
                }
            }
            // `module "vpc" { source = "../modules/vpc" }` is how one
            // configuration pulls in another.
            "attribute" if hcl_module_source(node, source).is_some() => ParsedItemKind::Import,
            // A `locals` block holds values the rest of the file reads as
            // `local.name`, one declaration each.
            "attribute" if hcl_local_value(node, source) => ParsedItemKind::Type,
            _ => return None,
        },
        // A `.proto` file is a service description: messages are the types
        // it carries, and an `rpc` is a call other code makes across the
        // wire.
        Language::Proto => match kind {
            "message" | "enum" | "service" => ParsedItemKind::Type,
            "rpc" => ParsedItemKind::Function,
            "import" => ParsedItemKind::Import,
            "package" => ParsedItemKind::Module,
            _ => return None,
        },
        // A contract states what anyone can call and what it will emit or
        // refuse with; inheritance and libraries are how one reaches
        // another.
        Language::Solidity => match kind {
            "contract_declaration"
            | "interface_declaration"
            | "library_declaration"
            | "struct_declaration"
            | "enum_declaration"
            | "user_defined_type_definition"
            | "event_definition"
            | "error_declaration" => ParsedItemKind::Type,
            "function_definition"
            | "constructor_definition"
            | "modifier_definition"
            | "fallback_receive_definition" => ParsedItemKind::Function,
            "import_directive" => ParsedItemKind::Import,
            _ => return None,
        },
        // A GraphQL schema states types and the fields that reach them; a
        // document states the operations a client sends.
        Language::GraphQl => match kind {
            "object_type_definition"
            | "interface_type_definition"
            | "input_object_type_definition"
            | "enum_type_definition"
            | "union_type_definition"
            | "scalar_type_definition" => ParsedItemKind::Type,
            "field_definition"
            | "operation_definition"
            | "fragment_definition"
            | "directive_definition" => ParsedItemKind::Function,
            _ => return None,
        },
    };

    let label = item_label(language, item_kind, node, source)?;
    let item_kind = if item_kind == ParsedItemKind::Function && is_entrypoint(language, &label) {
        ParsedItemKind::Entrypoint
    } else {
        item_kind
    };

    let mut metadata = BTreeMap::new();
    if item_kind == ParsedItemKind::Type
        && let Some(base) = base_type_label(language, node, source)
    {
        metadata.insert("extends".to_string(), base);
    }
    if matches!(
        item_kind,
        ParsedItemKind::Function | ParsedItemKind::Entrypoint
    ) {
        if let Some(owner) = enclosing_type_label(language, node, source) {
            metadata.insert("owner_type".to_string(), owner);
        }
        if let Some(visibility) = visibility_label(language, node, source, &label) {
            metadata.insert("visibility".to_string(), visibility.to_string());
        }
        // A function-like macro is called like a function and resolves like
        // one, but it is not one: it has no address, no types and no scope.
        // Saying so lets a reader tell `serverAssert` from a function.
        if node.kind() == "preproc_function_def" {
            metadata.insert("definition_form".to_string(), "macro".to_string());
        }
        // A value a factory built is callable when what it holds is, and a
        // reader should be able to tell it from a function the file spells
        // out.
        if node.kind() == "variable_declarator" {
            metadata.insert("definition_form".to_string(), "value".to_string());
        }
    }

    Some(ParsedItem {
        kind: item_kind,
        label,
        span: span_for(path, node),
        parent: None,
        metadata,
    })
}

/// The bare name of a Go type node, unwrapping the forms a receiver or
/// parameter can take: `Backend`, `*Backend`, `pkg.Backend`, `[]Backend`.
pub(crate) fn go_type_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    go_qualified_type_name(node, source).map(|name| {
        name.rsplit_once('.')
            .map_or(name.clone(), |(_, bare)| bare.to_string())
    })
}

/// Like [`go_type_name`], but keeping the package a type is qualified by
/// (`testing.T`). The package is what says whether a value's methods live in
/// this repository at all: `t.Fatalf()` on a `*testing.T` cannot resolve here,
/// and saying so is more useful than calling it unresolved.
pub(crate) fn go_qualified_type_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    match node.kind() {
        "type_identifier" => node_text(node, source),
        "qualified_type" => {
            let package = named_child_text(node, "package", source);
            let name = named_child_text(node, "name", source)?;
            Some(match package {
                Some(package) => format!("{package}.{name}"),
                None => name,
            })
        }
        "pointer_type" | "slice_type" | "array_type" | "parenthesized_type" => node
            .named_child(0)
            .and_then(|inner| go_qualified_type_name(inner, source)),
        _ => None,
    }
}

/// Whether a definition is part of what the project offers outwards.
///
/// A library has no `main`, so "reachable from an entrypoint" says nothing
/// about it; what it offers is its public surface. Each language states that
/// differently — a keyword, a wrapper, a capital letter, a leading underscore
/// — and only the unambiguous statements are recorded, so a language that says
/// nothing here leaves the question open rather than guessing.
pub(crate) fn visibility_label(
    language: Language,
    node: Node<'_>,
    source: &[u8],
    label: &str,
) -> Option<&'static str> {
    match language {
        // `pub` / `pub(crate)` / `pub(super)`.
        Language::Rust => {
            let mut cursor = node.walk();
            let Some(modifier) = node
                .children(&mut cursor)
                .find(|child| child.kind() == "visibility_modifier")
            else {
                // No `pub` is not an absence of information in Rust: it is
                // private, and the compiler enforces that.
                return Some("private");
            };
            let text = node_text(modifier, source).unwrap_or_default();
            Some(if text.trim() == "pub" {
                "public"
            } else {
                "crate"
            })
        }
        // Go says it with a capital letter, and means it: an identifier
        // starting lowercase cannot be referenced from another package.
        Language::Go => Some(
            if label
                .chars()
                .next()
                .is_some_and(|character| character.is_uppercase())
            {
                "public"
            } else {
                "private"
            },
        ),
        // An `export` wraps the declaration, and `export const f = () =>
        // {}` wraps it twice over: the arrow function sits inside a
        // declarator inside a declaration inside the export. Reading only
        // the immediate parent called 1227 of vue's exported functions
        // private, `isString` among them.
        Language::JavaScript | Language::TypeScript | Language::Tsx => {
            let mut current = node.parent();
            while let Some(parent) = current {
                match parent.kind() {
                    "export_statement" => return Some("public"),
                    "variable_declarator" | "lexical_declaration" | "variable_declaration" => {
                        current = parent.parent();
                    }
                    _ => break,
                }
            }
            Some("private")
        }
        // Convention, but a convention the whole ecosystem reads as a
        // contract.
        Language::Python => Some(if label.starts_with('_') {
            "private"
        } else {
            "public"
        }),
        // Java writes the visibility down or means package-private, and
        // Kotlin means public when it says nothing. Both are libraries'
        // languages, and without this the coverage finding cannot say what
        // okio and gson offer outwards.
        Language::Java => Some(match modifier_visibility(node, source) {
            Some("public") => "public",
            Some("protected") => "protected",
            Some("private") => "private",
            _ => "package",
        }),
        Language::Kotlin => Some(match modifier_visibility(node, source) {
            Some("protected") => "protected",
            Some("private") => "private",
            Some("internal") => "crate",
            _ => "public",
        }),
        // A C# member says nothing when it is private; PHP when it is
        // public; Swift's silence means its module, which is what `crate`
        // says here.
        Language::CSharp => Some(match modifier_visibility(node, source) {
            Some("public") => "public",
            Some("protected") => "protected",
            Some("internal") => "crate",
            _ => "private",
        }),
        Language::Php => Some(match modifier_visibility(node, source) {
            Some("protected") => "protected",
            Some("private") => "private",
            _ => "public",
        }),
        Language::Swift | Language::Scala => Some(match modifier_visibility(node, source) {
            Some("public") => "public",
            Some("protected") => "protected",
            Some("private") | Some("fileprivate") => "private",
            _ if language == Language::Scala => "public",
            _ => "crate",
        }),
        // The keyword these languages open a declaration with is the whole
        // of what they say: `static` gives a C function internal linkage,
        // `local` keeps a Lua function to its file, Elixir writes `defp`,
        // and Zig lets `pub` out.
        Language::C | Language::Cpp => Some(match leading_keyword(node, source).as_deref() {
            Some("static") => "private",
            _ => "public",
        }),
        Language::Lua => Some(
            if leading_keyword(node, source).as_deref() == Some("local") || lua_local_binding(node)
            {
                "private"
            } else {
                "public"
            },
        ),
        Language::Elixir => Some(match leading_keyword(node, source).as_deref() {
            Some("defp") => "private",
            _ => "public",
        }),
        Language::Zig => Some(match leading_keyword(node, source).as_deref() {
            Some("pub") => "public",
            _ => "private",
        }),
        // Ruby says it with a keyword that changes what follows it, and
        // sometimes about one definition or by name.
        Language::Ruby => Some(ruby_visibility(node, source)),
        // Erlang and Haskell write the list at the top of the file: what is
        // not on it cannot be called from another module. A module that
        // gives no list at all gives everything.
        Language::Erlang => Some(match erlang_exported_names(source) {
            Some(exported) if !exported.contains(label) => "private",
            _ => "public",
        }),
        Language::Haskell => Some(match haskell_exported_names(source) {
            Some(exported) if !exported.contains(label) => "private",
            _ => "public",
        }),
        // Solidity writes the visibility of every function down, and it
        // decides who may call: `external` and `public` are the contract's
        // ABI, `internal` reaches derived contracts the way `protected`
        // does, and `private` stops at this contract.
        Language::Solidity => match solidity_visibility(node, source)? {
            "external" | "public" => Some("public"),
            "internal" => Some("protected"),
            "private" => Some("private"),
            _ => None,
        },
        // Dart reads a leading underscore as library-private, as Python
        // reads it as a contract.
        Language::Dart => Some(if label.starts_with('_') {
            "private"
        } else {
            "public"
        }),
        _ => None,
    }
}

/// The word a Solidity declaration spends on visibility. The grammar
/// gives it a node of its own, sitting between the parameters and the
/// body.
fn solidity_visibility(node: Node<'_>, source: &[u8]) -> Option<&'static str> {
    let mut cursor = node.walk();
    let text = node
        .children(&mut cursor)
        .find(|child| child.kind() == "visibility")
        .and_then(|child| node_text(child, source))?;
    ["external", "public", "internal", "private"]
        .into_iter()
        .find(|keyword| text.trim() == *keyword)
}

/// The visibility keyword a declaration carries, if it carries one. Java
/// hangs them off a `modifiers` child and Kotlin off a `visibility_modifier`
/// inside one; both are read from the text so a grammar that renames the
/// inner node still answers.
fn modifier_visibility(node: Node<'_>, source: &[u8]) -> Option<&'static str> {
    let mut cursor = node.walk();
    let text = node
        .children(&mut cursor)
        .filter(|child| {
            matches!(
                child.kind(),
                "modifiers" | "modifier" | "visibility_modifier"
            )
        })
        .filter_map(|child| node_text(child, source))
        .collect::<Vec<_>>()
        .join(" ");
    // `fileprivate` before `private`: Swift writes both, and a
    // `fileprivate` definition is one another file cannot name, which is
    // what `private` records here.
    ["public", "fileprivate", "private", "protected", "internal"]
        .into_iter()
        .find(|keyword| {
            text.split(|character: char| !character.is_alphanumeric())
                .any(|word| word == *keyword)
        })
}

/// What a Ruby class body has said about a definition. `private` on a
/// line of its own changes what follows it, `private def foo` changes that
/// one definition, and `private :foo` names a method written elsewhere in
/// the body -- before or after.
fn ruby_visibility(node: Node<'_>, source: &[u8]) -> &'static str {
    if let Some(argument_list) = node.parent()
        && argument_list.kind() == "argument_list"
        && let Some(call) = argument_list.parent()
        && call.kind() == "call"
        && let Some(level) = named_child_text(call, "method", source)
            .as_deref()
            .and_then(ruby_visibility_keyword)
    {
        return level;
    }

    let Some(body) = node
        .parent()
        .filter(|parent| parent.kind() == "body_statement")
    else {
        return "public";
    };
    let name = named_child_text(node, "name", source);
    let mut level = "public";
    let mut reached = false;
    let mut cursor = body.walk();
    for child in body.named_children(&mut cursor) {
        if child.id() == node.id() {
            reached = true;
            continue;
        }
        match child.kind() {
            // A bare `private` is an identifier, not a call.
            "identifier" if !reached => {
                if let Some(next) = node_text(child, source)
                    .as_deref()
                    .and_then(ruby_visibility_keyword)
                {
                    level = next;
                }
            }
            // `private :foo, :bar` reaches a definition wherever it sits.
            "call" => {
                let Some(keyword) = named_child_text(child, "method", source)
                    .as_deref()
                    .and_then(ruby_visibility_keyword)
                else {
                    continue;
                };
                let Some(arguments) = child.child_by_field_name("arguments") else {
                    continue;
                };
                let mut argument_cursor = arguments.walk();
                let names = arguments
                    .named_children(&mut argument_cursor)
                    .filter(|argument| argument.kind() == "simple_symbol")
                    .filter_map(|argument| node_text(argument, source))
                    .any(|symbol| Some(symbol.trim_start_matches(':')) == name.as_deref());
                if names {
                    return keyword;
                }
            }
            _ => {}
        }
    }
    level
}

/// The visibility a Ruby keyword sets, if it is one.
fn ruby_visibility_keyword(text: &str) -> Option<&'static str> {
    match text.trim() {
        "private" => Some("private"),
        // Ruby's `protected` refuses an outside caller, not a sibling in
        // the hierarchy, and a subclass is written in another file.
        "protected" => Some("protected"),
        "public" => Some("public"),
        _ => None,
    }
}

/// The names an Erlang module lets out, or `None` when it lets out
/// everything: a module with no `-export` attribute at all, or one that
/// says `-compile(export_all)`. `-export_type` lists types, not functions.
fn erlang_exported_names(source: &[u8]) -> Option<BTreeSet<String>> {
    let text = std::str::from_utf8(source).ok()?;
    let mut names = BTreeSet::new();
    let mut declared = false;
    let mut collecting = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('%') {
            continue;
        }
        if trimmed.contains("export_all") {
            return None;
        }
        let body = if collecting {
            trimmed
        } else if let Some(rest) = trimmed.strip_prefix("-export(") {
            declared = true;
            collecting = true;
            rest.trim_start().trim_start_matches('[')
        } else {
            continue;
        };
        let end = body.find(']');
        for entry in body[..end.unwrap_or(body.len())].split(',') {
            let name = entry.split('/').next().unwrap_or_default().trim();
            if !name.is_empty() {
                names.insert(name.to_string());
            }
        }
        if end.is_some() {
            collecting = false;
        }
    }
    declared.then_some(names)
}

/// The names a Haskell module lists after its own, or `None` when it lists
/// nothing and so exports everything. `Type(..)` and `module X` re-exports
/// are left alone: only a plain name answers for a function.
fn haskell_exported_names(source: &[u8]) -> Option<BTreeSet<String>> {
    let text = std::str::from_utf8(source).ok()?;
    let start = if text.starts_with("module ") {
        0
    } else {
        text.find("\nmodule ")? + 1
    };
    let header = &text[start..];
    let open = header.find('(')?;
    if header
        .find(" where")
        .is_some_and(|where_index| where_index < open)
    {
        return None;
    }
    let mut depth = 0usize;
    let mut end = None;
    for (index, character) in header[open..].char_indices() {
        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(open + index);
                    break;
                }
            }
            _ => {}
        }
    }
    let list = &header[open + 1..end?];
    let mut names = BTreeSet::new();
    let mut depth = 0usize;
    for entry in list.split(|character: char| {
        match character {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
        character == ',' && depth == 0
    }) {
        let name = entry
            .split('(')
            .next()
            .unwrap_or_default()
            .trim()
            .rsplit('.')
            .next()
            .unwrap_or_default()
            .trim();
        if !name.is_empty() {
            names.insert(name.to_string());
        }
    }
    Some(names)
}

/// Whether a Lua function is bound by a `local` declaration. `local
/// handler = function() end` keeps it to its file exactly as `local
/// function handler()` does, but the expression itself opens with
/// `function`, so the binding has to be looked at.
fn lua_local_binding(node: Node<'_>) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        match parent.kind() {
            "variable_declaration" => return true,
            "expression_list" | "assignment_statement" => current = parent.parent(),
            _ => return false,
        }
    }
    false
}

/// The word a declaration opens with, which is where several languages put
/// what they let others see.
fn leading_keyword(node: Node<'_>, source: &[u8]) -> Option<String> {
    node_text(node, source)?
        .split_whitespace()
        .next()
        .map(str::to_string)
}

/// The type a method belongs to: the nearest enclosing type declaration (or
/// Rust `impl` block). Recorded as `owner_type` so a qualified call such as
/// `CodeGraph::new` or `Foo.bar` can be matched against the method declared
/// inside that type, which a bare `new` label could never satisfy.
/// What a class inherits from, as the source writes it: `class
/// AlbumController extends Controller` and `class
/// AdditionalFooterTextsController < Admin::SettingsController`. A route
/// whose action the class itself does not declare is served by the one
/// its parent declares, and without this the graph could not say so.
pub(crate) fn base_type_label(language: Language, node: Node<'_>, source: &[u8]) -> Option<String> {
    let text = match language {
        // The grammar gives each of these a child of its own holding the
        // whole clause: `< Foo`, `extends Foo`, `: Foo, IBar`, `(Base)`.
        Language::Ruby => child_kind_text(node, "superclass", source),
        Language::Php => child_kind_text(node, "base_clause", source),
        Language::JavaScript | Language::TypeScript | Language::Tsx => {
            child_kind_text(node, "class_heritage", source)
        }
        Language::Python => node
            .child_by_field_name("superclasses")
            .and_then(|list| node_text(list, source)),
        Language::CSharp => child_kind_text(node, "base_list", source),
        Language::Java | Language::Kotlin => node
            .child_by_field_name("superclass")
            .and_then(|parent| node_text(parent, source)),
        _ => None,
    }?;
    // Whatever states the relation is not part of the name.
    let text = text.trim();
    let text = text
        .strip_prefix("extends")
        .or_else(|| text.strip_prefix('<'))
        .or_else(|| text.strip_prefix(':'))
        .unwrap_or(text)
        .trim_start_matches('(')
        .trim();
    // `extends Foo implements Bar` and `(Base, Mixin)` name more than the
    // parent, and the parent is the first of them.
    let first = text
        .split([',', ')'])
        .next()
        .unwrap_or(text)
        .split_whitespace()
        .next()
        .unwrap_or_default();
    // A generic argument is not part of the name a declaration answers to.
    let first = first.split(['<', '(']).next().unwrap_or(first).trim();
    (!first.is_empty()
        && first.chars().all(|character| {
            character.is_alphanumeric() || matches!(character, '_' | ':' | '.' | '\\')
        }))
    .then(|| first.to_string())
}

pub(crate) fn enclosing_type_label(
    language: Language,
    node: Node<'_>,
    source: &[u8],
) -> Option<String> {
    // Go states the owner in the declaration itself, not in an enclosing
    // block: `func (b *Backend) Configure(...)` is top level, and its receiver
    // names the type. Walking ancestors found nothing, so no Go method knew
    // which type it belonged to.
    if language == Language::Go && node.kind() == "method_declaration" {
        return node
            .child_by_field_name("receiver")
            .and_then(|receiver| receiver.named_child(0))
            .and_then(|declaration| declaration.child_by_field_name("type"))
            .and_then(|type_node| go_type_name(type_node, source));
    }

    // C++ writes a member's definition outside its class and names the
    // owner in the declarator: `void file_helper::open(..)` belongs to
    // `file_helper`, and nothing encloses it to say so.
    if matches!(language, Language::C | Language::Cpp)
        && let Some(owner) = c_qualified_declarator_owner(node, source)
    {
        return Some(owner);
    }

    let mut current = node.parent();
    while let Some(candidate) = current {
        let kind = candidate.kind();
        let owner = match language {
            // An impl block is not a type declaration of its own; its `type`
            // field names the type being implemented.
            Language::Rust if kind == "impl_item" => named_child_text(candidate, "type", source),
            Language::Rust if kind == "trait_item" => named_child_text(candidate, "name", source),
            Language::Python if kind == "class_definition" => {
                named_child_text(candidate, "name", source)
            }
            // Ruby states a constant path: a class inside `module
            // Settings` is `Settings::AccountsController`, which is how
            // mastodon writes the same class when it declares it in one
            // line. Without the modules, two controllers of the same
            // name are one.
            Language::Ruby if matches!(kind, "class" | "module") => {
                ruby_constant_path(candidate, source)
            }
            Language::Java | Language::CSharp | Language::Kotlin | Language::Scala
                if kind.ends_with("_declaration") || kind.ends_with("_definition") =>
            {
                matches!(
                    kind,
                    "class_declaration"
                        | "interface_declaration"
                        | "enum_declaration"
                        | "record_declaration"
                        | "struct_declaration"
                        | "object_declaration"
                        | "class_definition"
                        | "object_definition"
                        | "trait_definition"
                )
                .then(|| named_child_text(candidate, "name", source))
                .flatten()
            }
            Language::Swift if kind == "class_declaration" || kind == "protocol_declaration" => {
                named_child_text(candidate, "name", source)
            }
            // A Dart extension's methods are called on the type it extends, so
            // that type owns them — the extension's own name is not what a
            // call says.
            Language::Dart if kind == "extension_declaration" => {
                named_child_text(candidate, "class", source)
                    .map(|name| simple_name(&name).to_string())
                    .or_else(|| named_child_text(candidate, "name", source))
            }
            Language::Dart
                if matches!(
                    kind,
                    "class_declaration" | "mixin_declaration" | "enum_declaration"
                ) =>
            {
                named_child_text(candidate, "name", source)
            }
            Language::Php | Language::JavaScript | Language::TypeScript | Language::Tsx
                if matches!(kind, "class_declaration" | "interface_declaration") =>
            {
                named_child_text(candidate, "name", source)
            }
            // An Objective-C method belongs to the class, category or
            // protocol that states it, in the header and in the
            // implementation alike.
            Language::ObjectiveC
                if matches!(
                    kind,
                    "class_interface" | "class_implementation" | "protocol_declaration"
                ) =>
            {
                child_kind_text(candidate, "identifier", source)
            }
            // A Solidity function belongs to the contract, interface or
            // library that declares it.
            Language::Solidity
                if matches!(
                    kind,
                    "contract_declaration" | "interface_declaration" | "library_declaration"
                ) =>
            {
                named_child_text(candidate, "name", source)
            }
            // An `rpc` belongs to the service that offers it.
            Language::Proto if kind == "service" => {
                child_kind_text(candidate, "service_name", source)
            }
            // A GraphQL field belongs to the type that has it.
            Language::GraphQl
                if matches!(
                    kind,
                    "object_type_definition"
                        | "interface_type_definition"
                        | "input_object_type_definition"
                ) =>
            {
                child_kind_text(candidate, "name", source)
            }
            _ => None,
        };
        if let Some(owner) = owner {
            return Some(owner);
        }
        current = candidate.parent();
    }
    None
}

/// Whether an OCaml `let_binding` declares parameters (making it a function)
/// rather than binding a value.
pub(crate) fn ocaml_binding_has_parameter(node: Node<'_>) -> bool {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .any(|child| child.kind() == "parameter")
}

pub(crate) fn classify_call(
    language: Language,
    node: Node<'_>,
    source: &[u8],
    path: &str,
    function_name: Option<&str>,
    scope: &DefinitionScope,
) -> Option<ParsedItem> {
    if !is_call_node(language, node, source) {
        return None;
    }

    // `go func() { … }()`, `(() => { … })()`, a C++ `[&](){ … }()` — the
    // callee is a literal with no name of its own, and its text reduced to
    // a label like `func` or to the whole body. terraform alone recorded
    // 179 calls to a function named `func`. The body's own calls already
    // carry their real caller, so there is nothing to record here.
    if let Some(callee) = call_callee(language, node)
        && is_deferred_body(language, callee.kind(), path)
    {
        return None;
    }

    let label = call_label(language, node, source)?;
    // A name cannot hold a block, a statement separator, a line break, a
    // quote, a parenthesis or a space. When one of those survives, the
    // callee was an expression rather than a name -- terraform's
    // `(*StackChangeProgress_Hook)(x)`, nlohmann's `(std::numeric_limits`
    // and `j.template get`, redis's `"/sbin/$sysctl"`, vue's `(transformSrcset
    // as Function)` -- or the parser recovered from a syntax error
    // mid-file, and the label is a fragment of source rather than something
    // to put in a graph. 812 of terraform's call nodes were of that kind.
    if label.is_empty() || label.contains(['{', '}', '(', ')', ';', '\n', '"', '\'', ' ', '\t']) {
        return None;
    }

    // `b.Configure()` says nothing about which `Configure` it means, but the
    // enclosing signature does: `b` is declared there with a type. Only a
    // single receiver counts — in `b.client.Do()` the method belongs to the
    // field's type, which the signature does not name.
    let mut metadata = BTreeMap::new();
    if let Some((receiver, method)) = label.split_once('.')
        && !method.contains('.')
        && let Some(receiver_type) = scope.variable_types.get(receiver)
    {
        metadata.insert("receiver_type".to_string(), receiver_type.clone());
    }
    // `[NSURL URLWithString:url]` names the class it messages, and the
    // receiver is the only thing that tells Foundation's `URLWithString:`
    // from a method a project declares under the same selector.
    if language == Language::ObjectiveC
        && node.kind() == "message_expression"
        && let Some(receiver) = node
            .child_by_field_name("receiver")
            .and_then(|receiver| node_text(receiver, source))
        && receiver
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        metadata.insert("receiver".to_string(), receiver);
    }
    // Ruby drops the receiver from the name being called -- the label of
    // `Rails.application.configure` is `configure` -- so the constant the
    // call is written through is the only thing left that says whose method
    // it means.
    if language == Language::Ruby && node.child_by_field_name("receiver").is_some() {
        match ruby_constant_receiver(node, source) {
            Some(receiver) => {
                metadata.insert("receiver".to_string(), receiver);
            }
            // `accounts.each`, `@definitions.keys`, `base.extend`: the call
            // goes through a value whose class the syntax does not name.
            // Which methods it can mean is a different question from a bare
            // call, which means `self`.
            None => {
                metadata.insert("receiver_form".to_string(), "value".to_string());
            }
        }
    }
    // `done()` where the body wrote `runningCtx, done := context.WithCancel(…)`
    // calls a value, not a definition. Saying so separates a call that has
    // nothing to find from one the resolver failed on: 1499 of terraform's
    // 3536 unresolved bare Go calls are of this kind.
    if !label.contains('.') && scope.local_values.contains(&label) {
        metadata.insert("callee_form".to_string(), "value".to_string());
    }

    Some(ParsedItem {
        kind: ParsedItemKind::Call,
        label,
        span: span_for(path, node),
        parent: function_name.map(str::to_string),
        metadata,
    })
}

/// Variables a definition's own signature declares with a type: a Go
/// receiver and its parameters. Other languages return nothing yet, so the
/// call sites simply carry no receiver type.
/// What a definition's own body says about the names inside it: the types
/// its signature and declarations give them, and the names it binds to
/// values. A call to one of the latter goes through a value rather than to
/// a definition, which is a different thing from a name nothing defines.
#[derive(Default)]
pub(crate) struct DefinitionScope {
    pub(crate) variable_types: BTreeMap<String, String>,
    pub(crate) local_values: BTreeSet<String>,
}

/// Everything a definition's body says about the names inside it. Only Go
/// is read for now — the same extraction the receiver types already use.
pub(crate) fn definition_scope(
    language: Language,
    node: Node<'_>,
    source: &[u8],
) -> DefinitionScope {
    DefinitionScope {
        variable_types: declared_variable_types(language, node, source),
        local_values: if language == Language::Go {
            go_shadowed_names(node, source)
        } else {
            BTreeSet::new()
        },
    }
}

pub(crate) fn declared_variable_types(
    language: Language,
    node: Node<'_>,
    source: &[u8],
) -> BTreeMap<String, String> {
    let mut declared = BTreeMap::new();
    if language != Language::Go {
        return declared;
    }
    for field in ["receiver", "parameters"] {
        let Some(list) = node.child_by_field_name(field) else {
            continue;
        };
        let mut cursor = list.walk();
        for declaration in list.named_children(&mut cursor) {
            if declaration.kind() != "parameter_declaration" {
                continue;
            }
            let Some(type_name) = declaration
                .child_by_field_name("type")
                .and_then(|type_node| go_qualified_type_name(type_node, source))
            else {
                continue;
            };
            // `func f(a, b *Thing)` declares both names with one type.
            let mut names = declaration.walk();
            for child in declaration.named_children(&mut names) {
                if child.kind() == "identifier"
                    && let Some(name) = node_text(child, source)
                {
                    declared.insert(name, type_name.clone());
                }
            }
        }
    }
    // A name the body re-declares is no longer what the signature said: Go
    // code shadows `ctx` constantly, and terraform's `multiPartUploadImpl`
    // takes a `context.Context` and then binds `ctx := &uploadContext{}`.
    // Trusting the signature there would call a local method external.
    for shadowed in go_shadowed_names(node, source) {
        declared.remove(&shadowed);
    }
    // `var diags tfdiags.Diagnostics` states the type outright — 2755 of
    // terraform's declarations do, a thousand of them `diags`, whose methods
    // are the most ambiguous calls in the repository. A declaration that names
    // its type is worth more than the signature it shadows.
    declared.extend(go_declared_var_types(node, source));
    declared.extend(go_composite_literal_types(node, source));
    declared
}

/// Types stated by `name := Type{...}` inside a body. The type is written
/// at the assignment, so the value's methods are that type's methods --
/// 372 of terraform's ambiguous calls have a receiver bound this way.
///
/// `m := map[string]int{}` and `s := []Item{}` state a shape rather than a
/// name, and a value of either has no methods of its own, so only a named
/// type is recorded. A name bound twice to different types is left out
/// rather than guessed at.
fn go_composite_literal_types(node: Node<'_>, source: &[u8]) -> BTreeMap<String, String> {
    let mut declared: BTreeMap<String, String> = BTreeMap::new();
    let mut conflicting = BTreeSet::new();
    let Some(body) = node.child_by_field_name("body") else {
        return declared;
    };
    let mut stack = vec![body];
    while let Some(current) = stack.pop() {
        if current.kind() == "short_var_declaration"
            && let Some(left) = current.child_by_field_name("left")
            && let Some(right) = current.child_by_field_name("right")
        {
            let mut left_cursor = left.walk();
            let names: Vec<Node<'_>> = left.named_children(&mut left_cursor).collect();
            let mut right_cursor = right.walk();
            let values: Vec<Node<'_>> = right.named_children(&mut right_cursor).collect();
            // `a, b := X{}, Y{}` pairs each name with its own value, and
            // `a, b := f()` pairs none of them with anything written down.
            if names.len() == values.len() {
                for (name, value) in names.iter().zip(values.iter()) {
                    if name.kind() != "identifier" {
                        continue;
                    }
                    let (Some(name), Some(type_name)) = (
                        node_text(*name, source),
                        go_composite_literal_type(*value, source),
                    ) else {
                        continue;
                    };
                    if declared
                        .insert(name.clone(), type_name.clone())
                        .is_some_and(|previous| previous != type_name)
                    {
                        conflicting.insert(name);
                    }
                }
            }
        }
        let mut cursor = current.walk();
        stack.extend(current.named_children(&mut cursor));
    }
    for name in conflicting {
        declared.remove(&name);
    }
    declared
}

/// The named type of `Type{...}` or of `&Type{...}`, which is a pointer to
/// the same type and carries the same methods.
fn go_composite_literal_type(value: Node<'_>, source: &[u8]) -> Option<String> {
    let literal = match value.kind() {
        "composite_literal" => value,
        "unary_expression" => value
            .child_by_field_name("operand")
            .filter(|operand| operand.kind() == "composite_literal")?,
        _ => return None,
    };
    let type_node = literal.child_by_field_name("type")?;
    if !matches!(type_node.kind(), "type_identifier" | "qualified_type") {
        return None;
    }
    go_qualified_type_name(type_node, source)
}

/// Types stated by `var name Type` inside a body.
fn go_declared_var_types(node: Node<'_>, source: &[u8]) -> BTreeMap<String, String> {
    let mut declared = BTreeMap::new();
    let Some(body) = node.child_by_field_name("body") else {
        return declared;
    };
    let mut stack = vec![body];
    while let Some(current) = stack.pop() {
        if current.kind() == "var_spec"
            && let Some(name) = current
                .child_by_field_name("name")
                .and_then(|name| node_text(name, source))
            && let Some(type_name) = current
                .child_by_field_name("type")
                .and_then(|type_node| go_qualified_type_name(type_node, source))
        {
            declared.insert(name, type_name);
        }
        let mut cursor = current.walk();
        stack.extend(current.named_children(&mut cursor));
    }
    declared
}

/// Names a function body re-declares with `:=` or `var`, which supersede the
/// signature for the rest of that body.
fn go_shadowed_names(node: Node<'_>, source: &[u8]) -> BTreeSet<String> {
    let mut shadowed = BTreeSet::new();
    let Some(body) = node.child_by_field_name("body") else {
        return shadowed;
    };
    let mut stack = vec![body];
    while let Some(current) = stack.pop() {
        match current.kind() {
            "short_var_declaration" => {
                if let Some(left) = current.child_by_field_name("left") {
                    let mut cursor = left.walk();
                    for name in left.named_children(&mut cursor) {
                        if name.kind() == "identifier"
                            && let Some(text) = node_text(name, source)
                        {
                            shadowed.insert(text);
                        }
                    }
                }
            }
            "var_spec" => {
                if let Some(name) = current
                    .child_by_field_name("name")
                    .and_then(|name| node_text(name, source))
                {
                    shadowed.insert(name);
                }
            }
            _ => {}
        }
        let mut cursor = current.walk();
        stack.extend(current.named_children(&mut cursor));
    }
    shadowed
}

pub(crate) fn classify_effect(
    language: Language,
    node: Node<'_>,
    source: &[u8],
    path: &str,
    function_name: Option<&str>,
) -> Option<ParsedItem> {
    let (kind, label, metadata) = if is_environment_read(language, node, source) {
        effect_label(language, ParsedItemKind::EnvironmentRead, node, source).map(|label| {
            (
                ParsedItemKind::EnvironmentRead,
                label,
                effect_metadata(language, ParsedItemKind::EnvironmentRead, node, source),
            )
        })
    } else if is_config_read(language, node, source) {
        effect_label(language, ParsedItemKind::ConfigRead, node, source).map(|label| {
            (
                ParsedItemKind::ConfigRead,
                label,
                effect_metadata(language, ParsedItemKind::ConfigRead, node, source),
            )
        })
    } else if is_error_construct(language, node, source) {
        effect_label(language, ParsedItemKind::Error, node, source).map(|label| {
            (
                ParsedItemKind::Error,
                label,
                effect_metadata(language, ParsedItemKind::Error, node, source),
            )
        })
    } else {
        None
    }?;

    Some(ParsedItem {
        kind,
        label,
        span: span_for(path, node),
        parent: function_name.map(ToString::to_string),
        metadata,
    })
}

pub(crate) fn classify_control_flow(
    language: Language,
    node: Node<'_>,
    source: &[u8],
    path: &str,
    function_name: Option<&str>,
) -> Option<ParsedItem> {
    let (kind, control_kind) = match language {
        Language::Elixir => elixir_control_flow_fact(node, source)?,
        Language::R => r_control_flow_fact(node, source)?,
        _ => control_flow_fact(language, node)?,
    };
    let mut metadata = BTreeMap::new();
    metadata.insert("control_kind".to_string(), control_kind.to_string());
    metadata.insert("syntax_node".to_string(), node.kind().to_string());
    if let Some(snippet) = short_node_text(node, source) {
        metadata.insert(
            "snippet".to_string(),
            truncate_label(compact_label(snippet), 160),
        );
    }

    Some(ParsedItem {
        kind,
        label: format!("{}: {}", parsed_item_kind_label(kind), control_kind),
        span: span_for(path, node),
        parent: function_name.map(ToString::to_string),
        metadata,
    })
}

pub(crate) fn control_flow_fact(
    language: Language,
    node: Node<'_>,
) -> Option<(ParsedItemKind, &'static str)> {
    let kind = node.kind();
    match language {
        Language::Rust => match kind {
            "if_expression" => Some((ParsedItemKind::Branch, "if")),
            "match_expression" => Some((ParsedItemKind::Branch, "match")),
            "try_expression" => Some((ParsedItemKind::Branch, "try")),
            "for_expression" => Some((ParsedItemKind::Loop, "for")),
            "while_expression" => Some((ParsedItemKind::Loop, "while")),
            "loop_expression" => Some((ParsedItemKind::Loop, "loop")),
            "await_expression" => Some((ParsedItemKind::Async, "await")),
            "async_block" => Some((ParsedItemKind::Async, "async")),
            "return_expression" => Some((ParsedItemKind::Return, "return")),
            _ => None,
        },
        Language::Python => match kind {
            "if_statement" => Some((ParsedItemKind::Branch, "if")),
            "match_statement" => Some((ParsedItemKind::Branch, "match")),
            "try_statement" => Some((ParsedItemKind::Branch, "try")),
            "except_clause" => Some((ParsedItemKind::Branch, "except")),
            "for_statement" => Some((ParsedItemKind::Loop, "for")),
            "while_statement" => Some((ParsedItemKind::Loop, "while")),
            "await" => Some((ParsedItemKind::Async, "await")),
            "async_function_definition" => Some((ParsedItemKind::Async, "function")),
            "async_for_statement" => Some((ParsedItemKind::Async, "for")),
            "async_with_statement" => Some((ParsedItemKind::Async, "with")),
            "return_statement" => Some((ParsedItemKind::Return, "return")),
            _ => None,
        },
        Language::JavaScript | Language::TypeScript | Language::Tsx => match kind {
            "if_statement" => Some((ParsedItemKind::Branch, "if")),
            "switch_statement" => Some((ParsedItemKind::Branch, "switch")),
            "switch_case" => Some((ParsedItemKind::Branch, "case")),
            "try_statement" => Some((ParsedItemKind::Branch, "try")),
            "catch_clause" => Some((ParsedItemKind::Branch, "catch")),
            "ternary_expression" => Some((ParsedItemKind::Branch, "ternary")),
            "for_statement" | "for_in_statement" => Some((ParsedItemKind::Loop, "for")),
            "while_statement" => Some((ParsedItemKind::Loop, "while")),
            "do_statement" => Some((ParsedItemKind::Loop, "do")),
            "await_expression" => Some((ParsedItemKind::Async, "await")),
            "return_statement" => Some((ParsedItemKind::Return, "return")),
            _ => None,
        },
        Language::Go => match kind {
            "if_statement" => Some((ParsedItemKind::Branch, "if")),
            "switch_statement" | "expression_switch_statement" | "type_switch_statement" => {
                Some((ParsedItemKind::Branch, "switch"))
            }
            "select_statement" => Some((ParsedItemKind::Branch, "select")),
            "for_statement" => Some((ParsedItemKind::Loop, "for")),
            "go_statement" => Some((ParsedItemKind::Async, "go")),
            "defer_statement" => Some((ParsedItemKind::Async, "defer")),
            "return_statement" => Some((ParsedItemKind::Return, "return")),
            _ => None,
        },
        Language::C | Language::Cpp | Language::ObjectiveC => match kind {
            "if_statement" => Some((ParsedItemKind::Branch, "if")),
            "switch_statement" => Some((ParsedItemKind::Branch, "switch")),
            "case_statement" => Some((ParsedItemKind::Branch, "case")),
            "for_statement" => Some((ParsedItemKind::Loop, "for")),
            "while_statement" => Some((ParsedItemKind::Loop, "while")),
            "do_statement" => Some((ParsedItemKind::Loop, "do")),
            "try_statement" => Some((ParsedItemKind::Branch, "try")),
            "catch_clause" => Some((ParsedItemKind::Branch, "catch")),
            "return_statement" => Some((ParsedItemKind::Return, "return")),
            _ => None,
        },
        Language::Php => match kind {
            "if_statement" => Some((ParsedItemKind::Branch, "if")),
            "match_expression" => Some((ParsedItemKind::Branch, "match")),
            "switch_statement" => Some((ParsedItemKind::Branch, "switch")),
            "try_statement" => Some((ParsedItemKind::Branch, "try")),
            "catch_clause" => Some((ParsedItemKind::Branch, "catch")),
            "foreach_statement" => Some((ParsedItemKind::Loop, "foreach")),
            "for_statement" => Some((ParsedItemKind::Loop, "for")),
            "while_statement" => Some((ParsedItemKind::Loop, "while")),
            "do_statement" => Some((ParsedItemKind::Loop, "do")),
            "return_statement" => Some((ParsedItemKind::Return, "return")),
            _ => None,
        },
        Language::Dart => match kind {
            "if_statement" => Some((ParsedItemKind::Branch, "if")),
            "switch_statement" | "switch_expression" => Some((ParsedItemKind::Branch, "switch")),
            "try_statement" => Some((ParsedItemKind::Branch, "try")),
            "catch_clause" => Some((ParsedItemKind::Branch, "catch")),
            "for_statement" | "for_element" => Some((ParsedItemKind::Loop, "for")),
            "while_statement" => Some((ParsedItemKind::Loop, "while")),
            "do_statement" => Some((ParsedItemKind::Loop, "do")),
            "await_expression" => Some((ParsedItemKind::Async, "await")),
            "return_statement" => Some((ParsedItemKind::Return, "return")),
            _ => None,
        },
        Language::Bash => match kind {
            "if_statement" => Some((ParsedItemKind::Branch, "if")),
            "case_statement" => Some((ParsedItemKind::Branch, "case")),
            "for_statement" => Some((ParsedItemKind::Loop, "for")),
            "while_statement" => Some((ParsedItemKind::Loop, "while")),
            "until_statement" => Some((ParsedItemKind::Loop, "until")),
            _ => None,
        },
        Language::Ruby => match kind {
            "if" | "unless" => Some((ParsedItemKind::Branch, "if")),
            "case" => Some((ParsedItemKind::Branch, "case")),
            "rescue" => Some((ParsedItemKind::Branch, "rescue")),
            "for" => Some((ParsedItemKind::Loop, "for")),
            "while" => Some((ParsedItemKind::Loop, "while")),
            "until" => Some((ParsedItemKind::Loop, "until")),
            "return" => Some((ParsedItemKind::Return, "return")),
            _ => None,
        },
        Language::Java => match kind {
            "if_statement" => Some((ParsedItemKind::Branch, "if")),
            "switch_expression" => Some((ParsedItemKind::Branch, "switch")),
            "try_statement" => Some((ParsedItemKind::Branch, "try")),
            "catch_clause" => Some((ParsedItemKind::Branch, "catch")),
            "for_statement" | "enhanced_for_statement" => Some((ParsedItemKind::Loop, "for")),
            "while_statement" => Some((ParsedItemKind::Loop, "while")),
            "do_statement" => Some((ParsedItemKind::Loop, "do")),
            "return_statement" => Some((ParsedItemKind::Return, "return")),
            _ => None,
        },
        Language::CSharp => match kind {
            "if_statement" => Some((ParsedItemKind::Branch, "if")),
            "switch_statement" | "switch_expression" => Some((ParsedItemKind::Branch, "switch")),
            "try_statement" => Some((ParsedItemKind::Branch, "try")),
            "catch_clause" => Some((ParsedItemKind::Branch, "catch")),
            "for_statement" | "foreach_statement" => Some((ParsedItemKind::Loop, "for")),
            "while_statement" => Some((ParsedItemKind::Loop, "while")),
            "do_statement" => Some((ParsedItemKind::Loop, "do")),
            "await_expression" => Some((ParsedItemKind::Async, "await")),
            "return_statement" => Some((ParsedItemKind::Return, "return")),
            _ => None,
        },
        Language::Kotlin => match kind {
            "if_expression" => Some((ParsedItemKind::Branch, "if")),
            "when_expression" => Some((ParsedItemKind::Branch, "when")),
            "try_expression" => Some((ParsedItemKind::Branch, "try")),
            "catch_block" => Some((ParsedItemKind::Branch, "catch")),
            "for_statement" => Some((ParsedItemKind::Loop, "for")),
            "while_statement" => Some((ParsedItemKind::Loop, "while")),
            "do_while_statement" => Some((ParsedItemKind::Loop, "do")),
            "return_expression" => Some((ParsedItemKind::Return, "return")),
            _ => None,
        },
        Language::Swift => match kind {
            "if_statement" => Some((ParsedItemKind::Branch, "if")),
            "guard_statement" => Some((ParsedItemKind::Branch, "guard")),
            "switch_statement" => Some((ParsedItemKind::Branch, "switch")),
            "do_statement" => Some((ParsedItemKind::Branch, "do")),
            "catch_block" => Some((ParsedItemKind::Branch, "catch")),
            "for_statement" => Some((ParsedItemKind::Loop, "for")),
            "while_statement" => Some((ParsedItemKind::Loop, "while")),
            "repeat_while_statement" => Some((ParsedItemKind::Loop, "repeat")),
            "await_expression" => Some((ParsedItemKind::Async, "await")),
            "control_transfer_statement" => Some((ParsedItemKind::Return, "return")),
            _ => None,
        },
        Language::Scala => match kind {
            "if_expression" => Some((ParsedItemKind::Branch, "if")),
            "match_expression" => Some((ParsedItemKind::Branch, "match")),
            "try_expression" => Some((ParsedItemKind::Branch, "try")),
            "catch_clause" => Some((ParsedItemKind::Branch, "catch")),
            "for_expression" => Some((ParsedItemKind::Loop, "for")),
            "while_expression" => Some((ParsedItemKind::Loop, "while")),
            "do_while_expression" => Some((ParsedItemKind::Loop, "do")),
            "return_expression" => Some((ParsedItemKind::Return, "return")),
            _ => None,
        },
        Language::Lua => match kind {
            "if_statement" => Some((ParsedItemKind::Branch, "if")),
            "for_statement" => Some((ParsedItemKind::Loop, "for")),
            "while_statement" => Some((ParsedItemKind::Loop, "while")),
            "repeat_statement" => Some((ParsedItemKind::Loop, "repeat")),
            "return_statement" => Some((ParsedItemKind::Return, "return")),
            _ => None,
        },
        Language::Elixir => None, // handled below via elixir_control_flow
        Language::Haskell => match kind {
            "conditional" => Some((ParsedItemKind::Branch, "if")),
            "case" => Some((ParsedItemKind::Branch, "case")),
            _ => None,
        },
        Language::OCaml => match kind {
            "if_expression" => Some((ParsedItemKind::Branch, "if")),
            "match_expression" => Some((ParsedItemKind::Branch, "match")),
            "try_expression" => Some((ParsedItemKind::Branch, "try")),
            "for_expression" => Some((ParsedItemKind::Loop, "for")),
            "while_expression" => Some((ParsedItemKind::Loop, "while")),
            _ => None,
        },
        Language::Erlang => match kind {
            "case_expr" => Some((ParsedItemKind::Branch, "case")),
            "if_expr" => Some((ParsedItemKind::Branch, "if")),
            "try_expr" => Some((ParsedItemKind::Branch, "try")),
            "receive_expr" => Some((ParsedItemKind::Async, "receive")),
            _ => None,
        },
        Language::Nix => match kind {
            "if_expression" => Some((ParsedItemKind::Branch, "if")),
            _ => None,
        },
        Language::Proto | Language::GraphQl => None,
        Language::Solidity => match kind {
            "if_statement" => Some((ParsedItemKind::Branch, "if")),
            "try_statement" => Some((ParsedItemKind::Branch, "try")),
            "catch_clause" => Some((ParsedItemKind::Branch, "catch")),
            "for_statement" => Some((ParsedItemKind::Loop, "for")),
            "while_statement" | "do_while_statement" => Some((ParsedItemKind::Loop, "while")),
            "return_statement" => Some((ParsedItemKind::Return, "return")),
            _ => None,
        },
        // HCL writes its branch as `cond ? a : b` and its loop as a `for`
        // expression over a collection.
        Language::Hcl => match kind {
            "conditional" => Some((ParsedItemKind::Branch, "conditional")),
            "for_expr" => Some((ParsedItemKind::Loop, "for")),
            _ => None,
        },
        Language::R => match kind {
            "if_statement" => Some((ParsedItemKind::Branch, "if")),
            "for_statement" => Some((ParsedItemKind::Loop, "for")),
            "while_statement" => Some((ParsedItemKind::Loop, "while")),
            "repeat_statement" => Some((ParsedItemKind::Loop, "repeat")),
            _ => None,
        },
        Language::Julia => match kind {
            "if_statement" => Some((ParsedItemKind::Branch, "if")),
            "try_statement" => Some((ParsedItemKind::Branch, "try")),
            "catch_clause" => Some((ParsedItemKind::Branch, "catch")),
            "for_statement" => Some((ParsedItemKind::Loop, "for")),
            "while_statement" => Some((ParsedItemKind::Loop, "while")),
            "return_statement" => Some((ParsedItemKind::Return, "return")),
            _ => None,
        },
        Language::Zig => match kind {
            "if_statement" | "if_expression" => Some((ParsedItemKind::Branch, "if")),
            "switch_expression" => Some((ParsedItemKind::Branch, "switch")),
            "for_statement" | "for_expression" => Some((ParsedItemKind::Loop, "for")),
            "while_statement" | "while_expression" => Some((ParsedItemKind::Loop, "while")),
            "defer_statement" => Some((ParsedItemKind::Async, "defer")),
            "return_expression" => Some((ParsedItemKind::Return, "return")),
            _ => None,
        },
    }
}

/// What an Objective-C declaration is called: a class, category or
/// protocol by its first name, a method by its selector.
fn objc_item_label(node: Node<'_>, source: &[u8]) -> Option<String> {
    match node.kind() {
        "class_interface" | "class_implementation" | "protocol_declaration" => {
            child_kind_text(node, "identifier", source)
        }
        "method_declaration" | "method_definition" => objc_selector(node, source),
        _ => None,
    }
    .filter(|label| !label.is_empty())
}

/// The selector a method declares: `initWithBaseURL:sessionConfiguration:`
/// for one that takes arguments, and the bare name for one that does not.
/// The grammar writes the parts as plain identifiers between the
/// parameters, so the name is every identifier the declaration states.
fn objc_selector(node: Node<'_>, source: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    let mut parts = Vec::new();
    let mut takes_arguments = false;
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "identifier" => {
                if let Some(text) = node_text(child, source) {
                    parts.push(text);
                }
            }
            "method_parameter" => takes_arguments = true,
            _ => {}
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(if takes_arguments {
        format!("{}:", parts.join(":"))
    } else {
        parts.join(":")
    })
}

/// The selector a message sends: `[manager GET:path parameters:nil]` calls
/// `GET:parameters:`, which is the name the method declares.
fn objc_message_selector(node: Node<'_>, source: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    let parts = node
        .children_by_field_name("method", &mut cursor)
        .filter_map(|part| node_text(part, source))
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return None;
    }
    // Every argument is written after a colon, so a message with any
    // argument at all names a selector with colons in it.
    let mut cursor = node.walk();
    let receiver = node.child_by_field_name("receiver");
    let takes_arguments = node.named_children(&mut cursor).any(|child| {
        Some(child) != receiver
            && child.kind() != "identifier"
            && !matches!(child.kind(), "comment" | "argument_list")
    }) || node
        .children_by_field_name("method", &mut node.walk())
        .count()
        < {
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .filter(|child| child.kind() == "identifier" && Some(*child) != receiver)
                .count()
        };
    Some(if takes_arguments {
        format!("{}:", parts.join(":"))
    } else {
        parts.join(":")
    })
}

/// The text of the first child of a given kind, for grammars that name
/// their parts by node kind rather than by field.
fn child_kind_text(node: Node<'_>, kind: &str, source: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() == kind)
        .and_then(|child| node_text(child, source))
}

/// What a `.proto` declaration is called: a message, enum, service and rpc
/// each carry their name as a node of its own, and an import states a path.
fn proto_item_label(node: Node<'_>, source: &[u8]) -> Option<String> {
    match node.kind() {
        "message" => child_kind_text(node, "message_name", source),
        "enum" => child_kind_text(node, "enum_name", source),
        "service" => child_kind_text(node, "service_name", source),
        "rpc" => child_kind_text(node, "rpc_name", source),
        "package" => child_kind_text(node, "full_ident", source),
        "import" => node
            .child_by_field_name("path")
            .and_then(|path| node_text(path, source))
            .map(|path| path.trim_matches('"').to_string()),
        _ => None,
    }
    .filter(|label| !label.is_empty())
}

/// What a GraphQL declaration is called. Every definition carries a `name`,
/// and a fragment carries a `fragment_name`.
fn graphql_item_label(node: Node<'_>, source: &[u8]) -> Option<String> {
    child_kind_text(node, "name", source)
        .or_else(|| child_kind_text(node, "fragment_name", source))
        .filter(|label| !label.is_empty())
}

/// The block type: the first word of `resource "aws_instance" "web"`.
fn hcl_block_type(node: Node<'_>, source: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() == "identifier")
        .and_then(|child| node_text(child, source))
}

/// The quoted labels a block carries after its type, unquoted.
fn hcl_block_labels(node: Node<'_>, source: &[u8]) -> Vec<String> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|child| child.kind() == "string_lit")
        .filter_map(|child| node_text(child, source))
        .map(|text| text.trim_matches('"').to_string())
        .filter(|text| !text.is_empty())
        .collect()
}

/// Whether a block stands at the top level of its file. A `lifecycle` or
/// `network_interface` block is how a resource is configured, not another
/// thing the configuration declares.
fn hcl_block_is_top_level(node: Node<'_>) -> bool {
    node.parent()
        .and_then(|body| body.parent())
        .is_some_and(|parent| parent.kind() == "config_file")
}

/// Whether a top-level block declares something addressable. `locals` holds
/// its declarations one level down, and `terraform` states settings for the
/// run rather than anything the configuration refers to.
fn hcl_block_declares(node: Node<'_>, source: &[u8]) -> bool {
    hcl_block_is_top_level(node)
        && !matches!(
            hcl_block_type(node, source).as_deref(),
            Some("locals") | Some("terraform") | None
        )
}

/// Whether an attribute is a value inside a top-level `locals` block, which
/// the rest of the configuration reads as `local.name`. Every other
/// attribute is one setting of the block that holds it.
fn hcl_local_value(node: Node<'_>, source: &[u8]) -> bool {
    node.parent()
        .and_then(|body| body.parent())
        .is_some_and(|block| {
            block.kind() == "block"
                && hcl_block_is_top_level(block)
                && hcl_block_type(block, source).as_deref() == Some("locals")
        })
}

/// The address Terraform writes for a declaration, which is also how the
/// rest of the configuration refers to it: `aws_instance.web`, `var.region`,
/// `module.vpc`, `local.name`.
fn hcl_declaration_label(node: Node<'_>, source: &[u8]) -> Option<String> {
    if node.kind() == "attribute" {
        let name = first_identifier(node, source)?;
        return Some(format!("local.{name}"));
    }
    let block_type = hcl_block_type(node, source)?;
    let labels = hcl_block_labels(node, source);
    let joined = labels.join(".");
    Some(match block_type.as_str() {
        // A resource is addressed by its type and name alone.
        "resource" if !joined.is_empty() => joined,
        "variable" if !joined.is_empty() => format!("var.{joined}"),
        _ if joined.is_empty() => block_type,
        _ => format!("{block_type}.{joined}"),
    })
}

/// The declaration an expression refers to, written the way Terraform
/// addresses it: `module.vpc.id` refers to `module.vpc`, `var.region` to
/// `var.region`, `aws_instance.web.id` to `aws_instance.web`.
fn hcl_reference_address(node: Node<'_>, source: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    let mut children = node.named_children(&mut cursor);
    let head = children
        .next()
        .filter(|child| child.kind() == "variable_expr")?;
    let head = node_text(head.named_child(0)?, source)?;
    let attributes = children
        .filter(|child| child.kind() == "get_attr")
        .filter_map(|child| {
            child
                .named_child(0)
                .and_then(|name| node_text(name, source))
        })
        .collect::<Vec<_>>();
    match head.as_str() {
        // These name the run rather than anything the configuration
        // declares: the current instance, its index, the module's own path.
        "each" | "count" | "self" | "path" | "terraform" => None,
        "data" => {
            (attributes.len() >= 2).then(|| format!("data.{}.{}", attributes[0], attributes[1]))
        }
        _ => attributes.first().map(|name| format!("{head}.{name}")),
    }
}

/// The configuration a `module` block pulls in: the path or registry name
/// its `source` states.
fn hcl_module_source(node: Node<'_>, source: &[u8]) -> Option<String> {
    if node.kind() != "attribute" || first_identifier(node, source).as_deref() != Some("source") {
        return None;
    }
    let block = node.parent().and_then(|body| body.parent())?;
    if hcl_block_type(block, source).as_deref() != Some("module") {
        return None;
    }
    let mut cursor = node.walk();
    let value = node
        .named_children(&mut cursor)
        .find(|child| child.kind() != "identifier")
        .and_then(|child| node_text(child, source))?;
    let value = value.trim().trim_matches('"').to_string();
    (!value.is_empty()).then_some(value)
}

/// Whether this node is the `@` of an Elixir module attribute:
/// `@moduledoc false`, `@spec change(..) :: t`, `@derive Jason.Encoder`.
/// The grammar reads what follows the `@` as a call, and an attribute is a
/// declaration rather than something the module does.
fn elixir_module_attribute(node: Node<'_>, source: &[u8]) -> bool {
    node.kind() == "unary_operator"
        && node
            .child(0)
            .and_then(|operator| node_text(operator, source))
            .as_deref()
            .map(str::trim)
            == Some("@")
}

/// Whether this Elixir call invokes a value rather than a name:
/// `fun.(new, current)` calls whatever `fun` holds, and the target the
/// grammar gives it ends in the dot.
fn elixir_value_invocation(node: Node<'_>, source: &[u8]) -> bool {
    node.child_by_field_name("target")
        .and_then(|target| node_text(target, source))
        .is_some_and(|target| target.trim_end().ends_with('.'))
}

pub(crate) fn is_call_node(language: Language, node: Node<'_>, source: &[u8]) -> bool {
    match language {
        Language::Rust => matches!(node.kind(), "call_expression" | "macro_invocation"),
        Language::Python => node.kind() == "call",
        // `<TailwindIndicator />` is how a JSX runtime calls a component:
        // it compiles to `jsx(TailwindIndicator, props)`. Without reading
        // it, every component in a React project was written and never
        // used -- taxonomy's layout renders eleven of them and reached
        // none.
        // `import('./Home.vue')` is the file loading another module, not a
        // call to a function named `import`.
        Language::JavaScript | Language::TypeScript | Language::Tsx => {
            !js_import_call(node, source)
                && matches!(
                    node.kind(),
                    "call_expression"
                        | "new_expression"
                        | "jsx_opening_element"
                        | "jsx_self_closing_element"
                )
        }
        Language::Go | Language::C | Language::Cpp => node.kind() == "call_expression",
        // `[manager GET:path parameters:nil]` is a call, and its selector
        // is the name being called.
        Language::ObjectiveC => matches!(node.kind(), "call_expression" | "message_expression"),
        // `new SongService($repository)` names the class it builds, which is
        // how a PHP project reaches most of its own types: without it koel
        // had two references into 1319 class nodes, so "what breaks if I
        // change this class" answered with nothing.
        Language::Php => matches!(
            node.kind(),
            "function_call_expression"
                | "scoped_call_expression"
                | "member_call_expression"
                | "object_creation_expression"
        ),
        Language::Dart => matches!(node.kind(), "call_expression" | "constructor_invocation"),
        Language::Bash => {
            node.kind() == "command" && !command_text_starts_with(source, node, &["source", "."])
        }
        Language::Ruby => node.kind() == "call" && !ruby_require_call(node, source),
        Language::Java => {
            matches!(
                node.kind(),
                "method_invocation" | "object_creation_expression"
            )
        }
        Language::CSharp => {
            matches!(
                node.kind(),
                "invocation_expression" | "object_creation_expression"
            )
        }
        Language::Kotlin | Language::Swift => node.kind() == "call_expression",
        Language::Scala => matches!(node.kind(), "call_expression" | "instance_expression"),
        Language::Lua => node.kind() == "function_call" && !lua_require_call(node, source),
        Language::Elixir => {
            node.kind() == "call"
                && elixir_call_target(node, source)
                    .as_deref()
                    .is_some_and(|target| !ELIXIR_SPECIAL_FORMS.contains(&target))
                // `@moduledoc false` and `@spec change(..) :: t` are module
                // attributes: the grammar reads what follows the `@` as a
                // call, and ecto filed 356 calls to things named `doc`,
                // `type` and `spec`.
                && !node
                    .parent()
                    .is_some_and(|parent| elixir_module_attribute(parent, source))
                // `fun.(new, current)` invokes a value the body binds, and
                // the label it produced -- `fun.` -- names nothing. ecto
                // writes 82 of them.
                && !elixir_value_invocation(node, source)
        }
        Language::Zig => node.kind() == "call_expression",
        // `f a b` nests as apply(apply(f, a), b); only the outermost node is
        // one call, so skip an apply whose callee is another apply.
        Language::Haskell => {
            node.kind() == "apply"
                && !node
                    .child_by_field_name("function")
                    .is_some_and(|callee| callee.kind() == "apply")
        }
        Language::OCaml => node.kind() == "application_expression",
        Language::Julia => {
            node.kind() == "call_expression" && !julia_is_short_definition_head(node)
        }
        // A remote call (`os:getenv(..)`) wraps an inner `call`; count the
        // remote node and skip the inner one so one call is one fact.
        Language::Erlang => match node.kind() {
            "remote" => true,
            "call" => !node
                .parent()
                .is_some_and(|parent| parent.kind() == "remote"),
            _ => false,
        },
        // `f a b` nests as apply(apply(f, a), b), as in Haskell.
        Language::Nix => {
            node.kind() == "apply_expression"
                && !node
                    .child_by_field_name("function")
                    .is_some_and(|callee| callee.kind() == "apply_expression")
        }
        Language::R => node.kind() == "call" && !r_library_call(node, source),
        Language::Hcl => node.kind() == "function_call",
        // A schema declares; nothing in it runs.
        Language::Proto | Language::GraphQl => false,
        Language::Solidity => node.kind() == "call_expression",
    }
}

/// `name(args) = body`: an assignment whose left side is a call expression is
/// Julia's short function definition form.
pub(crate) fn julia_short_function_definition(node: Node<'_>) -> bool {
    node.kind() == "assignment"
        && node
            .named_child(0)
            .is_some_and(|left| left.kind() == "call_expression")
}

/// The name a Julia definition binds: the callee of its signature, kept whole
/// so that `Base.names` stays distinct from a local `names`, the way Lua keeps
/// `Plugins:select_by_ca_certificate`.
pub(crate) fn julia_definition_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    let head = julia_definition_head(node, 0)?;
    let callee = head.named_child(0)?;
    node_text(callee, source)
        .map(compact_label)
        .filter(|name| !name.is_empty())
}

/// The signature of a definition can sit under a `where` clause or a return
/// type, so walk down to the call that carries the name.
fn julia_definition_head<'tree>(node: Node<'tree>, depth: usize) -> Option<Node<'tree>> {
    if node.kind() == "call_expression" {
        return Some(node);
    }
    if depth > 3 {
        return None;
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find_map(|child| julia_definition_head(child, depth + 1))
}

/// The call-shaped left side of a short definition names the function being
/// defined; it is not a call to it.
pub(crate) fn julia_is_short_definition_head(node: Node<'_>) -> bool {
    node.parent().is_some_and(|parent| {
        julia_short_function_definition(parent)
            && parent.named_child(0).is_some_and(|left| left == node)
    })
}

/// A Nix binding whose value is a lambda, i.e. a named function.
/// Whether a Nix binding declares a module option: `enable = mkEnableOption
/// "Git"`, `key = mkOption { .. }`. home-manager states 3978 of them, and
/// they are the whole of what its modules offer to configure.
pub(crate) fn nix_option_declaration(node: Node<'_>, source: &[u8]) -> bool {
    let Some(value) = node.child_by_field_name("expression") else {
        return false;
    };
    let mut head = value;
    while head.kind() == "apply_expression" {
        let Some(function) = head.child_by_field_name("function") else {
            break;
        };
        head = function;
    }
    node_text(head, source).is_some_and(|text| {
        matches!(
            simple_name(&text),
            "mkOption" | "mkEnableOption" | "mkPackageOption" | "mkEnableOption'"
        )
    })
}

/// The path a module option is configured under. A declaration nests —
/// `options = { programs.git = { signing = { key = mkOption { .. }; }; }; }`
/// — and the name a user writes is every attribute on the way down.
pub(crate) fn nix_option_path(node: Node<'_>, source: &[u8]) -> Option<String> {
    let mut parts = Vec::new();
    let mut current = Some(node);
    while let Some(ancestor) = current {
        if ancestor.kind() == "binding"
            && let Some(path) = named_child_text(ancestor, "attrpath", source)
        {
            parts.push(path);
        }
        current = ancestor.parent();
    }
    parts.reverse();
    // `options` is where a module states its names rather than part of any
    // of them, and a submodule states more of them under the `type` of the
    // option that holds it: `options.programs.git.includes.type.options
    // .condition` is the option a user writes as
    // `programs.git.includes.condition`.
    let segments: Vec<&str> = parts.iter().flat_map(|part| part.split('.')).collect();
    let mut path: Vec<&str> = Vec::new();
    for (index, segment) in segments.iter().copied().enumerate() {
        // home-manager declares `programs.delta.options` and
        // `home.keyboard.options`: the last segment is the option's own
        // name, whatever it is called.
        if segment == "options" && index + 1 < segments.len() {
            if path
                .last()
                .is_some_and(|last| names_an_option_attribute(last))
            {
                path.pop();
            }
            continue;
        }
        path.push(segment);
    }
    (!path.is_empty()).then(|| path.join("."))
}

/// The names a Nix file binds to a part of the configuration:
/// `cfg = config.programs.git;`. Reading `cfg.enable` is reading
/// `programs.git.enable`, and the whole ecosystem writes it that way.
pub(crate) fn nix_config_aliases(root: Node<'_>, source: &[u8]) -> BTreeMap<String, String> {
    let mut aliases = BTreeMap::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        let mut cursor = node.walk();
        stack.extend(node.named_children(&mut cursor));
        if node.kind() != "binding" {
            continue;
        }
        let (Some(name), Some(value)) = (
            named_child_text(node, "attrpath", source),
            node.child_by_field_name("expression"),
        ) else {
            continue;
        };
        if name.contains('.') || value.kind() != "select_expression" {
            continue;
        }
        if let Some(path) = nix_config_path(value, source) {
            aliases.insert(name, path);
        }
    }
    aliases
}

/// The configuration path a `config.a.b` expression names.
fn nix_config_path(node: Node<'_>, source: &[u8]) -> Option<String> {
    let base = node.child_by_field_name("expression")?;
    (node_text(base, source).as_deref() == Some("config"))
        .then(|| named_child_text(node, "attrpath", source))
        .flatten()
}

/// The option a `select_expression` reads, whether it says so outright or
/// through the name the file gave that part of the configuration.
pub(crate) fn nix_option_reference(
    node: Node<'_>,
    source: &[u8],
    aliases: &BTreeMap<String, String>,
) -> Option<String> {
    if let Some(path) = nix_config_path(node, source) {
        return Some(path);
    }
    let base = node.child_by_field_name("expression")?;
    let name = node_text(base, source)?;
    let prefix = aliases.get(&name)?;
    let path = named_child_text(node, "attrpath", source)?;
    Some(format!("{prefix}.{path}"))
}

/// The attributes `mkOption` itself takes, which a nested declaration sits
/// under without them being part of its name.
fn names_an_option_attribute(segment: &str) -> bool {
    matches!(
        segment,
        "type"
            | "default"
            | "example"
            | "description"
            | "apply"
            | "defaultText"
            | "nullable"
            | "readOnly"
            | "visible"
            | "internal"
            | "freeformType"
    )
}

pub(crate) fn nix_binding_is_function(node: Node<'_>) -> bool {
    node.child_by_field_name("expression")
        .is_some_and(|value| value.kind() == "function_expression")
}

/// An R assignment (`name <- function(..)`) that defines a function.
pub(crate) fn r_assignment_defines_function(node: Node<'_>) -> bool {
    node.child_by_field_name("rhs")
        .is_some_and(|value| value.kind() == "function_definition")
}

/// `library(pkg)` / `require(pkg)`: an import fact, not a call fact.
pub(crate) fn r_library_call(node: Node<'_>, source: &[u8]) -> bool {
    node.kind() == "call"
        && named_child_text(node, "function", source)
            .as_deref()
            .is_some_and(|name| matches!(name, "library" | "require" | "requireNamespace"))
}

/// Elixir special forms parse as ordinary `call` nodes; these targets are
/// definitions, imports, or control flow rather than function calls.
pub(crate) const ELIXIR_SPECIAL_FORMS: &[&str] = &[
    "def",
    "defp",
    "defmodule",
    "defmacro",
    "defmacrop",
    "defimpl",
    "defprotocol",
    "defstruct",
    "defdelegate",
    "defguard",
    "defguardp",
    "import",
    "require",
    "use",
    "alias",
    "if",
    "unless",
    "case",
    "cond",
    "for",
    "with",
    "try",
    "receive",
    "raise",
    "throw",
    "quote",
    "unquote",
];

/// The target text of an Elixir `call` node (an identifier or dotted access).
pub(crate) fn elixir_call_target(node: Node<'_>, source: &[u8]) -> Option<String> {
    if node.kind() != "call" {
        return None;
    }
    named_child_text(node, "target", source)
}

/// A Lua `require("module")` call: an import fact, not a call fact.
pub(crate) fn lua_require_call(node: Node<'_>, source: &[u8]) -> bool {
    node.kind() == "function_call"
        && named_child_text(node, "name", source).as_deref() == Some("require")
}

/// Zig `@import("std")` builtin.
pub(crate) fn zig_import_builtin(node: Node<'_>, source: &[u8]) -> bool {
    node.kind() == "builtin_function"
        && node
            .named_child(0)
            .and_then(|child| node_text(child, source))
            .as_deref()
            == Some("@import")
}

/// A Ruby `require`/`require_relative` call: an import fact, not a call fact.
/// The constant a ruby call is written through: the leading `Foo::Bar` of its
/// receiver, whatever follows. `Rails.application.configure` is written
/// through `Rails`, and `UserSettings::Namespace.new(key).configure` through
/// `UserSettings::Namespace`. A receiver that begins with a value --
/// `account.each`, `@definitions.keys`, `base.extend` -- names no constant.
fn ruby_constant_receiver(node: Node<'_>, source: &[u8]) -> Option<String> {
    let receiver = node.child_by_field_name("receiver")?;
    let text = node_text(receiver, source)?;
    let head: String = text
        .chars()
        .take_while(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | ':'))
        .collect();
    // `::Foo` names the same constant as `Foo`, from the top level out.
    let head = head.trim_start_matches("::").trim_end_matches(':');
    let names_a_constant = !head.is_empty()
        && head.split("::").all(|segment| {
            segment
                .chars()
                .next()
                .is_some_and(|first| first.is_ascii_uppercase())
        });
    names_a_constant.then(|| head.to_string())
}

pub(crate) fn ruby_require_call(node: Node<'_>, source: &[u8]) -> bool {
    node.kind() == "call"
        && named_child_text(node, "method", source)
            .as_deref()
            .is_some_and(|method| matches!(method, "require" | "require_relative"))
        // `require` is Kernel's, and a bare call is the only way to reach
        // it: `params.require(:source)` is Rails asking a request for a
        // parameter, and mastodon writes fifteen of those.
        && node.child_by_field_name("receiver").is_none()
}

pub(crate) fn call_label(language: Language, node: Node<'_>, source: &[u8]) -> Option<String> {
    if language == Language::Bash {
        return node_text(node, source)
            .and_then(|text| text.split_whitespace().next().map(ToString::to_string));
    }

    if language == Language::Ruby
        && let Some(method) = named_child_text(node, "method", source)
    {
        return Some(clean_call_label(&method));
    }

    // A JSX element names the component it renders. A lower-case name is
    // an HTML tag the platform provides, and a name with a dash is a
    // custom element registered at runtime; neither is a component this
    // project declares.
    if matches!(
        node.kind(),
        "jsx_opening_element" | "jsx_self_closing_element"
    ) {
        let name = named_child_text(node, "name", source)?;
        let first = name.chars().next()?;
        if !first.is_ascii_uppercase() || name.contains('-') {
            return None;
        }
        return Some(clean_call_label(&name));
    }

    // These grammars expose no named callee field; the callee is the first
    // named child (an identifier or dotted navigation), so the label is its
    // trailing path segment: `System.getenv(..)` -> `getenv`.
    // PHP writes the class a static call goes through, and the label kept
    // only the method: `Uuid::generate()` and koel's own
    // `TestableIdentifier::generate` looked like one call, and 61 call sites
    // reached a helper they never name. `self`, `static` and `parent` name
    // the class the call is already inside, which the label cannot carry.
    if language == Language::Php
        && node.kind() == "scoped_call_expression"
        && let Some(name) = named_child_text(node, "name", source)
    {
        let scope = named_child_text(node, "scope", source)
            .map(|scope| scope.trim().trim_start_matches('\\').to_string())
            .filter(|scope| {
                !scope.is_empty()
                    && !matches!(scope.as_str(), "self" | "static" | "parent")
                    && scope.chars().all(|character| {
                        character.is_alphanumeric() || matches!(character, '_' | '\\')
                    })
            });
        return Some(match scope {
            Some(scope) => clean_call_label(&format!("{scope}::{name}")),
            None => clean_call_label(&name),
        });
    }

    if matches!(language, Language::Kotlin | Language::Swift)
        && let Some(callee) = node
            .named_child(0)
            .and_then(|child| node_text(child, source))
    {
        return Some(clean_call_label(simple_name(&callee)));
    }

    if language == Language::ObjectiveC
        && node.kind() == "message_expression"
        && let Some(selector) = objc_message_selector(node, source)
    {
        return Some(clean_call_label(&selector));
    }

    if language == Language::Elixir
        && let Some(target) = named_child_text(node, "target", source)
    {
        return Some(clean_call_label(&target));
    }

    // Haskell's callee is an expression tree whose leaf is a `variable` node,
    // which is not one of the identifier kinds first_identifier knows; its
    // text is the applied function's name.
    if language == Language::Haskell
        && let Some(callee) = node.child_by_field_name("function")
    {
        return node_text(callee, source)
            .map(|name| clean_call_label(&name))
            .filter(|name| !name.is_empty());
    }

    // Julia exposes no callee field; it is the first named child.
    if language == Language::Julia
        && let Some(callee) = node
            .named_child(0)
            .and_then(|child| node_text(child, source))
    {
        return Some(clean_call_label(&callee));
    }

    // Erlang: `mod:fun(..)` is a remote node wrapping the inner call.
    if language == Language::Erlang {
        if node.kind() == "remote" {
            let module = named_child_text(node, "module", source)
                .unwrap_or_default()
                .trim_end_matches(':')
                .to_string();
            let function = node
                .child_by_field_name("fun")
                .and_then(|inner| named_child_text(inner, "expr", source))
                .or_else(|| named_child_text(node, "fun", source))
                .unwrap_or_default();
            let label = if module.is_empty() {
                function
            } else {
                format!("{module}:{function}")
            };
            return (!label.is_empty()).then(|| clean_call_label(&label));
        }
        if let Some(expr) = named_child_text(node, "expr", source) {
            return Some(clean_call_label(&expr));
        }
    }

    if let Some(function) = named_child_text(node, "function", source) {
        return Some(clean_call_label(&function));
    }

    if let Some(name) = named_child_text(node, "name", source) {
        return Some(clean_call_label(&name));
    }

    first_identifier(node, source).map(|name| clean_call_label(&name))
}

/// Control-flow facts for Elixir, whose if/case/cond/for/with parse as calls.
/// R writes its early exit as a call -- `return(x)` -- so the node kind
/// alone cannot see it, the way Elixir's control flow cannot be seen
/// without reading the call's target. dplyr writes 171 of them.
pub(crate) fn r_control_flow_fact(
    node: Node<'_>,
    source: &[u8],
) -> Option<(ParsedItemKind, &'static str)> {
    if node.kind() == "call"
        && node
            .child_by_field_name("function")
            .and_then(|function| node_text(function, source))
            .as_deref()
            == Some("return")
    {
        return Some((ParsedItemKind::Return, "return"));
    }
    control_flow_fact(Language::R, node)
}

pub(crate) fn elixir_control_flow_fact(
    node: Node<'_>,
    source: &[u8],
) -> Option<(ParsedItemKind, &'static str)> {
    match elixir_call_target(node, source).as_deref() {
        Some("if" | "unless") => Some((ParsedItemKind::Branch, "if")),
        Some("case") => Some((ParsedItemKind::Branch, "case")),
        Some("cond") => Some((ParsedItemKind::Branch, "cond")),
        Some("with") => Some((ParsedItemKind::Branch, "with")),
        Some("try" | "receive") => Some((ParsedItemKind::Branch, "try")),
        Some("for") => Some((ParsedItemKind::Loop, "for")),
        _ => None,
    }
}

/// Labels for Elixir definition calls: the module alias or the function-head
/// target inside the arguments (`defmodule Billing.Invoice`, `def total(..)`).
pub(crate) fn elixir_item_label(node: Node<'_>, source: &[u8]) -> Option<String> {
    // `defstruct [:name, :age]` names nothing: the struct is the module
    // that declares it, which is how Elixir refers to it (`%Ecto.Query{}`).
    // Ecto writes 25 of them and every one was dropped for want of a name.
    if elixir_call_target(node, source).as_deref() == Some("defstruct") {
        return elixir_enclosing_module(node, source);
    }
    let mut cursor = node.walk();
    let arguments = node
        .named_children(&mut cursor)
        .find(|child| child.kind() == "arguments")?;
    let first = arguments.named_child(0)?;
    elixir_definition_head(first, source)
}

/// The module a node is written inside, by the name it is declared with.
fn elixir_enclosing_module(node: Node<'_>, source: &[u8]) -> Option<String> {
    let mut current = node.parent();
    while let Some(parent) = current {
        if elixir_call_target(parent, source).as_deref() == Some("defmodule") {
            let mut cursor = parent.walk();
            let arguments = parent
                .named_children(&mut cursor)
                .find(|child| child.kind() == "arguments")?;
            return elixir_definition_head(arguments.named_child(0)?, source);
        }
        current = parent.parent();
    }
    None
}

/// The name inside a definition's first argument. `def foo(x) when guard` puts
/// the head on the left of the `when` operator, so a guarded clause — a common
/// Elixir idiom — needs one more hop to reach the name.
fn elixir_definition_head(node: Node<'_>, source: &[u8]) -> Option<String> {
    match node.kind() {
        "alias" | "identifier" => node_text(node, source),
        "call" => named_child_text(node, "target", source),
        "binary_operator" => node
            .named_child(0)
            .and_then(|left| elixir_definition_head(left, source)),
        _ => None,
    }
}

/// The name a `const x = factory(..)` declaration binds, when the value is
/// what a factory hands back. `export const onMounted = createHook(MOUNTED)`
/// is how vue declares most of its public API, and `const buttonVariants =
/// cva(..)` how a component library declares its variants: other files
/// import the name and call it. A declaration whose value is a literal, an
/// object or another name is not callable and is left alone.
pub(crate) fn js_value_declaration_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    let value = node.child_by_field_name("value")?;
    if value.kind() != "call_expression" {
        return None;
    }
    // Only what the module declares: `const rows = getRows()` inside a
    // function body is a local variable, and reading those as declarations
    // added 5442 nodes to vue alone.
    let mut ancestor = node.parent();
    while let Some(parent) = ancestor {
        if matches!(
            parent.kind(),
            "function_declaration"
                | "function_expression"
                | "arrow_function"
                | "method_definition"
                | "generator_function_declaration"
                | "class_body"
        ) {
            return None;
        }
        ancestor = parent.parent();
    }
    let name = node.child_by_field_name("name")?;
    // A destructured binding names several things at once, and which of
    // them the call hands back is not written down.
    if name.kind() != "identifier" {
        return None;
    }
    node_text(name, source).filter(|label| !label.is_empty())
}

/// The name a JS/TS function expression inherits from the binding it is
/// assigned to: `const f = () => {}`, `{ key: () => {} }`, `obj.m = () => {}`.
/// Returns None for a truly anonymous expression such as a bare callback.
/// The name a Lua binding gives an anonymous function: the field in
/// `{ init = function() end }`, the variable in `M.bar = function() end`.
/// A multiple assignment (`local a, b = function() end, 2`) is left alone —
/// matching a value to its name by position would be a guess.
pub(crate) fn lua_bound_function_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    let parent = node.parent()?;
    match parent.kind() {
        "field" => named_child_text(parent, "name", source),
        "expression_list" if parent.named_child_count() == 1 => {
            let assignment = parent.parent()?;
            if assignment.kind() != "assignment_statement" {
                return None;
            }
            let variables = assignment.named_child(0)?;
            if variables.kind() != "variable_list" || variables.named_child_count() != 1 {
                return None;
            }
            node_text(variables.named_child(0)?, source)
        }
        _ => None,
    }
}

/// Whether this definition is assigned to a name that lives outside the
/// function holding it. `let x` at module level and `x = i => {...}` inside
/// a function is one binding the whole module can call: vue writes
/// `installWithProxy` that way inside `registerRuntimeCompiler`, and
/// `finishComponentSetup` calls it. A declaration -- `const x = () => {}`
/// -- is the local case and keeps its scope.
pub(crate) fn binds_an_outer_name(language: Language, node: Node<'_>) -> bool {
    matches!(
        language,
        Language::JavaScript | Language::TypeScript | Language::Tsx
    ) && matches!(node.kind(), "arrow_function" | "function_expression")
        && node
            .parent()
            .is_some_and(|parent| parent.kind() == "assignment_expression")
}

/// Whether this is `import(..)`: the dynamic import, which reads as a
/// call to a function named `import` and is nothing of the kind.
pub(crate) fn js_import_call(node: Node<'_>, source: &[u8]) -> bool {
    node.kind() == "call_expression"
        && node
            .child_by_field_name("function")
            .and_then(|function| node_text(function, source))
            .as_deref()
            .map(str::trim)
            == Some("import")
}

/// What `import('./Home.vue')` loads. A router writes one per page, and
/// it is the only edge that reaches a lazily loaded one -- koel filed 168
/// of them as calls to a function named `import` and reached none of the
/// files. A specifier built at runtime -- `import(path)` -- names nothing
/// to resolve.
pub(crate) fn js_dynamic_import_specifier(node: Node<'_>, source: &[u8]) -> Option<String> {
    if !js_import_call(node, source) {
        return None;
    }
    let arguments = node.child_by_field_name("arguments")?;
    let mut cursor = arguments.walk();
    let first = arguments.named_children(&mut cursor).next()?;
    if !matches!(first.kind(), "string" | "template_string") {
        return None;
    }
    let text = node_text(first, source)?;
    let specifier = text
        .trim()
        .trim_matches(|character| matches!(character, '\'' | '"' | '`'));
    // `import(`./pages/${name}.vue`)` is a path the program builds.
    if specifier.is_empty() || specifier.contains(['$', '{', '\n']) {
        return None;
    }
    Some(specifier.to_string())
}

pub(crate) fn js_bound_function_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    let bound = node.parent().and_then(|parent| match parent.kind() {
        "variable_declarator" => named_child_text(parent, "name", source),
        "pair" => named_child_text(parent, "key", source),
        "assignment_expression" => named_child_text(parent, "left", source),
        _ => None,
    });
    // `.then(function onAdapterResolution(response) {...})` and
    // `new Promise(function dispatchXhrRequest(resolve) {...})` are
    // functions with names of their own and nothing to bind them to. A
    // binding still wins, since that is the name callers use.
    bound
        .or_else(|| named_child_text(node, "name", source))
        // An object key may be written as a string -- `{ 'onUpdate:folderId':
        // () => {} }` -- and the quotes are the syntax, not the name.
        .map(|name| {
            name.trim()
                .trim_matches(|character| matches!(character, '\'' | '"' | '`'))
                .to_string()
        })
        .filter(|name| !name.is_empty())
}

pub(crate) fn item_label(
    language: Language,
    kind: ParsedItemKind,
    node: Node<'_>,
    source: &[u8],
) -> Option<String> {
    // A function expression takes the name it is bound to, or the one it
    // was given.
    if matches!(
        language,
        Language::JavaScript | Language::TypeScript | Language::Tsx
    ) {
        if matches!(node.kind(), "arrow_function" | "function_expression") {
            return js_bound_function_name(node, source);
        }
        if node.kind() == "variable_declarator" {
            return js_value_declaration_name(node, source);
        }
        // A dynamic import states what it loads the way a static one does,
        // so every reader of an import label reads this one too.
        if node.kind() == "call_expression" {
            return js_dynamic_import_specifier(node, source)
                .map(|specifier| format!("import(\"{specifier}\")"));
        }
    }

    // A Ruby class states a constant path: `module Admin; class
    // AccountsController` is `Admin::AccountsController`, which is the
    // name its own methods already carry as their owner and the name a
    // route means. Without the modules, mastodon's two
    // `AccountsController` classes were one name for two classes.
    if language == Language::Ruby && matches!(node.kind(), "class" | "module") {
        return ruby_constant_path(node, source);
    }

    // `class SPDLOG_API logger { .. }` puts an export macro where the
    // grammar expects the name, so spdlog's central class -- and every
    // class a library exports this way -- had no node at all. The name is
    // the last type identifier before the body.
    if matches!(language, Language::C | Language::Cpp)
        && matches!(
            node.kind(),
            "class_specifier" | "struct_specifier" | "union_specifier"
        )
        && node.child_by_field_name("name").is_none()
        && node.child_by_field_name("body").is_some()
    {
        let mut cursor = node.walk();
        let candidates: Vec<Node<'_>> = node
            .named_children(&mut cursor)
            .take_while(|child| child.kind() != "field_declaration_list")
            .filter(|child| matches!(child.kind(), "type_identifier" | "identifier"))
            .collect();
        let name = candidates
            .last()
            .and_then(|child| node_text(*child, source))
            .filter(|label| !label.is_empty());
        if name.is_some() {
            return name;
        }
    }

    if language == Language::Lua && node.kind() == "function_definition" {
        return lua_bound_function_name(node, source);
    }

    if language == Language::Nix && kind == ParsedItemKind::Type {
        return nix_option_path(node, source);
    }

    // `import {Ownable} from "../access/Ownable.sol";` names a file; the
    // names it brings in are that file's, not this one's.
    if language == Language::Solidity && kind == ParsedItemKind::Import {
        return node
            .child_by_field_name("source")
            .and_then(|path| node_text(path, source))
            .map(|path| path.trim_matches(['"', '\'']).to_string())
            .filter(|path| !path.is_empty());
    }

    // A selector is the whole name of a method — `GET:parameters:success:`
    // — and it is what a caller writes at the call site.
    if language == Language::ObjectiveC
        && matches!(kind, ParsedItemKind::Function | ParsedItemKind::Type)
        && let Some(label) = objc_item_label(node, source)
    {
        return Some(label);
    }

    if language == Language::Proto {
        return proto_item_label(node, source);
    }

    if language == Language::GraphQl {
        return graphql_item_label(node, source);
    }

    if language == Language::Hcl {
        return match kind {
            ParsedItemKind::Import => hcl_module_source(node, source),
            _ => hcl_declaration_label(node, source),
        };
    }

    if kind == ParsedItemKind::Import {
        return node_text(node, source).map(compact_label);
    }

    // Julia writes the defined name as the callee of its signature, so the
    // first identifier of `function Base.names(df, cols)` is the module it
    // extends. DataFrames labels 536 methods `Base` without this.
    if language == Language::Julia
        && matches!(kind, ParsedItemKind::Function | ParsedItemKind::Entrypoint)
        && let Some(name) = julia_definition_name(node, source)
    {
        return Some(name);
    }

    if language == Language::Dart
        && matches!(kind, ParsedItemKind::Function | ParsedItemKind::Entrypoint)
        && let Some(name) = descendant_field_text(node, "name", source, 0)
    {
        return Some(name);
    }

    // `-type opts() :: ...` carries its parameter list in the name node; a
    // reader calls the type `opts`, as a record is called `state`.
    if language == Language::Erlang
        && matches!(node.kind(), "type_alias" | "opaque")
        && let Some(name) = named_child_text(node, "name", source)
    {
        return Some(
            name.split_once('(')
                .map(|(head, _)| head.trim().to_string())
                .unwrap_or(name),
        );
    }

    if let Some(name) = named_child_text(node, "name", source) {
        return Some(name);
    }

    match language {
        // Erlang keeps the name on the function clause, not on the declaration.
        Language::Erlang => node
            .child_by_field_name("clause")
            .and_then(|clause| named_child_text(clause, "name", source))
            .or_else(|| named_child_text(node, "name", source)),
        // Nix names a binding through its attribute path.
        Language::Nix => named_child_text(node, "attrpath", source),
        // R assigns the lambda to the left-hand identifier.
        Language::R => named_child_text(node, "lhs", source),
        // OCaml names a binding through its `pattern` field.
        Language::OCaml => named_child_text(node, "pattern", source)
            .or_else(|| named_child_text(node, "name", source))
            .or_else(|| first_identifier(node, source)),
        Language::Elixir => elixir_item_label(node, source),
        Language::C | Language::Cpp => {
            let declared = (kind == ParsedItemKind::Function)
                .then(|| c_declared_function_name(node, source))
                .flatten();
            declared
                .or_else(|| first_identifier_in_field(node, "declarator", source))
                .or_else(|| first_identifier(node, source))
        }
        Language::Go if kind == ParsedItemKind::Function => {
            named_child_text(node, "name", source).or_else(|| first_identifier(node, source))
        }
        Language::Bash => first_identifier(node, source),
        _ => first_identifier(node, source),
    }
}

/// The constant path a Ruby class or module states, modules included:
/// `module Settings; class AccountsController` is
/// `Settings::AccountsController`.
fn ruby_constant_path(node: Node<'_>, source: &[u8]) -> Option<String> {
    let mut parts = vec![named_child_text(node, "name", source)?];
    let mut current = node.parent();
    while let Some(candidate) = current {
        if matches!(candidate.kind(), "class" | "module")
            && let Some(name) = named_child_text(candidate, "name", source)
        {
            parts.push(name);
        }
        current = candidate.parent();
    }
    parts.reverse();
    Some(parts.join("::"))
}

/// The type an out-of-line C++ definition belongs to, read from the
/// scope of its qualified declarator. `sinks::base_sink<Mutex>::log`
/// belongs to `base_sink`, and a namespace on its own does not name a
/// type, so only the scope nearest the name is read.
fn c_qualified_declarator_owner(node: Node<'_>, source: &[u8]) -> Option<String> {
    let mut stack = vec![node];
    while let Some(current) = stack.pop() {
        if current.kind() == "function_declarator"
            && let Some(declarator) = current.child_by_field_name("declarator")
        {
            return c_qualified_owner(declarator, source);
        }
        let mut cursor = current.walk();
        let children: Vec<Node<'_>> = current.children(&mut cursor).collect();
        stack.extend(children.into_iter().rev());
    }
    None
}

fn c_qualified_owner(declarator: Node<'_>, source: &[u8]) -> Option<String> {
    let mut current = declarator;
    let mut owner = None;
    loop {
        match current.kind() {
            "qualified_identifier" => {
                let scope = current.child_by_field_name("scope");
                let name = current.child_by_field_name("name")?;
                // The scope nearest the name is the type; anything further
                // out is the namespace it sits in.
                if let Some(scope) = scope {
                    owner = c_type_name_of(scope, source).or(owner);
                }
                current = name;
            }
            "pointer_declarator" | "reference_declarator" | "parenthesized_declarator" => {
                current = current.child_by_field_name("declarator")?;
            }
            _ => break,
        }
    }
    owner
}

/// The type a scope names, unwrapping the template arguments a C++
/// definition repeats: `base_sink<Mutex>` is `base_sink`.
fn c_type_name_of(node: Node<'_>, source: &[u8]) -> Option<String> {
    let node = match node.kind() {
        "template_type" => node.child_by_field_name("name")?,
        _ => node,
    };
    node_text(node, source)
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty() && !name.contains(char::is_whitespace))
}

/// The name a declarator declares, past the parts that decorate it.
/// `void sinks::base_sink<Mutex>::log(..)` declares `log`; reading the
/// first identifier of the qualified name called it `base_sink`, and
/// spdlog had 27 functions named `registry` and 17 named `ansicolor_sink`
/// after the classes they belong to.
fn c_declarator_name(declarator: Node<'_>, source: &[u8]) -> Option<String> {
    let mut current = declarator.child_by_field_name("declarator")?;
    loop {
        let next = match current.kind() {
            "qualified_identifier" | "template_function" => current.child_by_field_name("name"),
            "pointer_declarator" | "reference_declarator" | "parenthesized_declarator" => {
                current.child_by_field_name("declarator")
            }
            _ => None,
        };
        match next {
            Some(next) => current = next,
            None => break,
        }
    }
    node_text(current, source)
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty() && !name.contains(char::is_whitespace))
}

/// The name a C or C++ definition attaches to its parameter list.
/// spdlog writes `SPDLOG_INLINE bool is_color_terminal() SPDLOG_NOEXCEPT`,
/// and a macro after the parameters is read by the grammar as the
/// declarator: seventeen of spdlog's functions were called
/// `SPDLOG_INLINE` and `is_color_terminal` was in the graph under no name
/// at all.
fn c_declared_function_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    let mut stack = vec![node];
    let mut called: Option<String> = None;
    while let Some(current) = stack.pop() {
        if current.kind() == "function_declarator"
            && let Some(name) = c_declarator_name(current, source)
        {
            return Some(name);
        }
        // The macro makes the grammar give up: what it recovers is the
        // name followed by the parameters, read as a call. That is still
        // the name the definition declares, so it answers when nothing
        // parsed cleanly.
        if current.kind() == "init_declarator"
            && current
                .child_by_field_name("value")
                .is_some_and(|value| value.kind() == "argument_list")
            && called.is_none()
        {
            called = first_identifier_in_field(current, "declarator", source);
        }
        let mut cursor = current.walk();
        let children: Vec<Node<'_>> = current.children(&mut cursor).collect();
        stack.extend(children.into_iter().rev());
    }
    called
}

pub(crate) fn descendant_field_text(
    node: Node<'_>,
    field: &str,
    source: &[u8],
    depth: usize,
) -> Option<String> {
    if depth > 4 {
        return None;
    }
    if let Some(child) = node.child_by_field_name(field) {
        return node_text(child, source);
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find_map(|child| descendant_field_text(child, field, source, depth + 1))
}

pub(crate) fn is_entrypoint(language: Language, label: &str) -> bool {
    match language {
        Language::Rust | Language::Go | Language::C | Language::Cpp | Language::ObjectiveC => {
            label == "main"
        }
        Language::Python => label == "main" || label == "__main__",
        Language::JavaScript | Language::TypeScript | Language::Tsx | Language::Php => {
            label.eq_ignore_ascii_case("main")
        }
        Language::Dart => label == "main",
        Language::Bash => label == "main",
        Language::Ruby | Language::Java => label == "main",
        Language::CSharp => label.eq_ignore_ascii_case("main"),
        Language::Kotlin | Language::Swift | Language::Scala => label == "main",
        Language::Lua | Language::Elixir | Language::Zig => label == "main",
        Language::Haskell | Language::OCaml | Language::Julia => label == "main",
        Language::Erlang | Language::Nix | Language::R => label == "main",
        // A configuration or a schema has no entrypoint: nothing in it
        // starts running, and a contract is started by whoever calls it.
        Language::Hcl | Language::Proto | Language::GraphQl | Language::Solidity => false,
    }
}

pub(crate) fn span_for(path: &str, node: Node<'_>) -> SourceSpan {
    let start = node.start_position();
    let end = node.end_position();
    SourceSpan {
        path: path.to_string(),
        start_line: start.row as u32 + 1,
        start_column: start.column as u32 + 1,
        end_line: end.row as u32 + 1,
        end_column: end.column as u32 + 1,
    }
}

pub(crate) fn dedupe_items(items: &mut Vec<ParsedItem>) {
    items.sort_by(|left, right| {
        (
            left.span.start_line,
            left.span.start_column,
            left.kind as u8,
            &left.label,
        )
            .cmp(&(
                right.span.start_line,
                right.span.start_column,
                right.kind as u8,
                &right.label,
            ))
    });
    items.dedup_by(|left, right| {
        left.kind == right.kind
            && left.label == right.label
            && left.span.start_line == right.span.start_line
            && left.span.start_column == right.span.start_column
    });
}

pub(crate) fn dedupe_type_references(references: &mut Vec<ParsedTypeReference>) {
    references.sort_by(|left, right| {
        (
            left.span.start_line,
            left.span.start_column,
            &left.label,
            &left.parent,
        )
            .cmp(&(
                right.span.start_line,
                right.span.start_column,
                &right.label,
                &right.parent,
            ))
    });
    references.dedup_by(|left, right| {
        left.label == right.label
            && left.parent == right.parent
            && left.span.start_line == right.span.start_line
            && left.span.start_column == right.span.start_column
    });
}

/// Whether this application is a Nix `import`: the function of the
/// innermost application is the `import` builtin, however many arguments
/// follow it (`import ./x.nix { ... }` applies twice).
fn nix_import_expression(node: Node<'_>, source: &[u8]) -> bool {
    let mut function = node.child_by_field_name("function");
    while let Some(inner) = function {
        if inner.kind() == "apply_expression" {
            function = inner.child_by_field_name("function");
            continue;
        }
        return node_text(inner, source).as_deref().map(str::trim) == Some("import");
    }
    false
}

/// Whether this Zig declaration binds a container -- a struct, an enum, an
/// error set or a union -- which is how the language declares a type.
fn zig_container_declaration(node: Node<'_>) -> bool {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).any(|child| {
        matches!(
            child.kind(),
            "struct_declaration"
                | "enum_declaration"
                | "union_declaration"
                | "error_set_declaration"
                | "opaque_declaration"
        )
    })
}

/// `include("file.jl")`: the Julia call that splices another file in.
fn julia_include_call(node: Node<'_>, source: &[u8]) -> bool {
    node.child(0)
        .and_then(|callee| node_text(callee, source))
        .as_deref()
        .map(str::trim)
        == Some("include")
}
