//! Tree walking and fact classification: parse a source file and turn
//! syntax nodes into structural, call, entrypoint, and control-flow items.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use codegraph_core::SourceSpan;
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

    let tree = parser
        .parse(source_text, None)
        .ok_or(ParseError::ParseFailed { language })?;
    let root = tree.root_node();
    let mut facts = CollectedFacts::default();
    collect_items(
        &WalkContext {
            language,
            source: source_text.as_bytes(),
            path: &path.to_string_lossy(),
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
    dedupe_items(&mut facts.items);
    dedupe_type_references(&mut facts.type_references);

    Ok(ParsedFile {
        language,
        items: facts.items,
        type_references: facts.type_references,
        quoted_line_ranges: quoted_line_ranges(root, source_text),
        has_error_nodes: root.has_error(),
    })
}

/// The lines covered by string literals and comments, merged. A detector
/// that scans text rather than syntax needs them: `@app.route("/")` in a
/// docstring is an example, not a route the program serves, and one such
/// line in flask claimed about 140 functions as its handler.
fn quoted_line_ranges(root: Node<'_>, source: &str) -> Vec<(u32, u32)> {
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
        if kind.contains("string") || kind.contains("comment") || kind.contains("heredoc") {
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

    let mut next_function = current_function;
    let mut next_scope: Option<DefinitionScope> = None;
    if let Some(mut item) = classify_node(language, node, source, path) {
        if matches!(
            item.kind,
            ParsedItemKind::Function | ParsedItemKind::Entrypoint
        ) {
            // A definition nested in another one is visible only inside it —
            // a Haskell `where` binding, a local `fn`, a closure bound to a
            // name. Recording which definition encloses it lets call
            // resolution stop treating 167 unrelated local `f`s as candidates
            // for one call.
            if let Some(enclosing) = next_function.as_deref() {
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
    let next_deferred = deferred || is_deferred_body(language, node.kind());
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
fn is_deferred_body(language: Language, kind: &str) -> bool {
    // Ruby writes callbacks as blocks, and only Ruby calls those nodes
    // `block` — elsewhere `block` is an ordinary statement list, and a
    // Python `if __name__ == "__main__":` body is exactly the load-time
    // code this must keep.
    if language == Language::Ruby && matches!(kind, "block" | "do_block") {
        return true;
    }
    // Lua splits the two: `function foo() end` is a `function_declaration`,
    // and only the anonymous `function() end` is a `function_definition` —
    // a kind that names the ordinary declaration in Python and C.
    if language == Language::Lua && kind == "function_definition" {
        return true;
    }
    kind.contains("lambda")
        || kind.contains("closure")
        || kind.contains("anonymous")
        || matches!(
            kind,
            "arrow_function" | "function_expression" | "func_literal" | "fn"
        )
}

pub(crate) fn classify_node(
    language: Language,
    node: Node<'_>,
    source: &[u8],
    path: &str,
) -> Option<ParsedItem> {
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
            "class_declaration"
            | "interface_declaration"
            | "type_alias_declaration"
            | "enum_declaration" => ParsedItemKind::Type,
            "import_statement" => ParsedItemKind::Import,
            _ => return None,
        },
        Language::Go => match kind {
            "function_declaration" | "method_declaration" => ParsedItemKind::Function,
            "type_declaration" => ParsedItemKind::Type,
            "import_spec" => ParsedItemKind::Import,
            _ => return None,
        },
        Language::C | Language::Cpp => match kind {
            "function_definition" => ParsedItemKind::Function,
            // `#define serverAssert(x) …` defines something the code calls
            // like a function, and redis calls 7300 of them. An object-like
            // `#define LIMIT 10` is a value, not a callable, and stays out.
            "preproc_function_def" => ParsedItemKind::Function,
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
            Some("defprotocol" | "defimpl" | "defstruct") => ParsedItemKind::Type,
            Some("import" | "require" | "use" | "alias") => ParsedItemKind::Import,
            _ => return None,
        },
        Language::Zig => match kind {
            "function_declaration" => ParsedItemKind::Function,
            "builtin_function" if zig_import_builtin(node, source) => ParsedItemKind::Import,
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
            _ => return None,
        },
        // Nix has no functions or types as such; a binding whose value is a
        // lambda is the closest thing to a named callable, and `import` calls
        // pull in other expressions.
        Language::Nix => match kind {
            "binding" if nix_binding_is_function(node) => ParsedItemKind::Function,
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
            Some("private") | Some("protected") => "private",
            _ => "package",
        }),
        Language::Kotlin => Some(match modifier_visibility(node, source) {
            Some("private") | Some("protected") => "private",
            Some("internal") => "crate",
            _ => "public",
        }),
        // A C# member says nothing when it is private; PHP when it is
        // public; Swift's silence means its module, which is what `crate`
        // says here.
        Language::CSharp => Some(match modifier_visibility(node, source) {
            Some("public") => "public",
            Some("internal") => "crate",
            _ => "private",
        }),
        Language::Php => Some(match modifier_visibility(node, source) {
            Some("private") | Some("protected") => "private",
            _ => "public",
        }),
        Language::Swift | Language::Scala => Some(match modifier_visibility(node, source) {
            Some("public") => "public",
            Some("private") | Some("protected") => "private",
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
    ["public", "private", "protected", "internal"]
        .into_iter()
        .find(|keyword| {
            text.split(|character: char| !character.is_alphanumeric())
                .any(|word| word == *keyword)
        })
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
            Language::Ruby if matches!(kind, "class" | "module") => {
                named_child_text(candidate, "name", source)
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
        && is_deferred_body(language, callee.kind())
    {
        return None;
    }

    let label = call_label(language, node, source)?;
    // A name cannot hold a block, a statement separator or a line break.
    // When one of those survives, the callee was an expression rather than
    // a name — or the parser recovered from a syntax error mid-file — and
    // the label is a fragment of source, not something to put in a graph.
    if label.is_empty() || label.contains(['{', '}', ';', '\n']) {
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
    let (kind, control_kind) = if language == Language::Elixir {
        elixir_control_flow_fact(node, source)?
    } else {
        control_flow_fact(language, node)?
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
        Language::C | Language::Cpp => match kind {
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

pub(crate) fn is_call_node(language: Language, node: Node<'_>, source: &[u8]) -> bool {
    match language {
        Language::Rust => matches!(node.kind(), "call_expression" | "macro_invocation"),
        Language::Python => node.kind() == "call",
        Language::JavaScript | Language::TypeScript | Language::Tsx => {
            matches!(node.kind(), "call_expression" | "new_expression")
        }
        Language::Go | Language::C | Language::Cpp => node.kind() == "call_expression",
        Language::Php => matches!(
            node.kind(),
            "function_call_expression" | "scoped_call_expression" | "member_call_expression"
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

/// The call-shaped left side of a short definition names the function being
/// defined; it is not a call to it.
pub(crate) fn julia_is_short_definition_head(node: Node<'_>) -> bool {
    node.parent().is_some_and(|parent| {
        julia_short_function_definition(parent)
            && parent.named_child(0).is_some_and(|left| left == node)
    })
}

/// A Nix binding whose value is a lambda, i.e. a named function.
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
pub(crate) fn ruby_require_call(node: Node<'_>, source: &[u8]) -> bool {
    node.kind() == "call"
        && named_child_text(node, "method", source)
            .as_deref()
            .is_some_and(|method| matches!(method, "require" | "require_relative"))
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

    // These grammars expose no named callee field; the callee is the first
    // named child (an identifier or dotted navigation), so the label is its
    // trailing path segment: `System.getenv(..)` -> `getenv`.
    if matches!(language, Language::Kotlin | Language::Swift)
        && let Some(callee) = node
            .named_child(0)
            .and_then(|child| node_text(child, source))
    {
        return Some(clean_call_label(simple_name(&callee)));
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
    let mut cursor = node.walk();
    let arguments = node
        .named_children(&mut cursor)
        .find(|child| child.kind() == "arguments")?;
    let first = arguments.named_child(0)?;
    elixir_definition_head(first, source)
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
    bound.or_else(|| named_child_text(node, "name", source))
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
    ) && matches!(node.kind(), "arrow_function" | "function_expression")
    {
        return js_bound_function_name(node, source);
    }

    if language == Language::Lua && node.kind() == "function_definition" {
        return lua_bound_function_name(node, source);
    }

    if kind == ParsedItemKind::Import {
        return node_text(node, source).map(compact_label);
    }

    if language == Language::Dart
        && matches!(kind, ParsedItemKind::Function | ParsedItemKind::Entrypoint)
        && let Some(name) = descendant_field_text(node, "name", source, 0)
    {
        return Some(name);
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
        Language::C | Language::Cpp => first_identifier_in_field(node, "declarator", source)
            .or_else(|| first_identifier(node, source)),
        Language::Go if kind == ParsedItemKind::Function => {
            named_child_text(node, "name", source).or_else(|| first_identifier(node, source))
        }
        Language::Bash => first_identifier(node, source),
        _ => first_identifier(node, source),
    }
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
        Language::Rust | Language::Go | Language::C | Language::Cpp => label == "main",
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
