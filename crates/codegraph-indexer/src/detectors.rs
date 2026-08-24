//! File-level detectors dispatched per source file: script entrypoints,
//! framework routes and configs, CommonJS requires, and Dart/native
//! platform channels.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use codegraph_core::{Confidence, EdgeKind, NodeId, NodeKind, SourceSpan};
use codegraph_parser::{Language, ParsedFile};

#[allow(unused_imports)]
use crate::*;

pub(crate) fn index_manifest_facts(
    context: &mut IndexContext,
    file_id: NodeId,
    path: &Path,
    label: &str,
    source: &str,
) {
    index_manifest_dependencies(context, file_id, path, source);
    index_manifest_entrypoints(context, file_id, path, label, source);
    index_pubspec_assets(context, file_id, path, label, source);
    index_makefile_entrypoints(context, file_id, path, label, source);
    index_dockerfile_entrypoints(context, file_id, path, label, source);
    index_compose_entrypoints(context, file_id, path, label, source);
    index_github_actions_workflow_entrypoints(context, file_id, path, label, source);
    index_gitlab_ci_entrypoints(context, file_id, path, label, source);
    index_kubernetes_manifest_facts(context, file_id, path, label, source);
    index_properties_settings(context, file_id, label, source);
    index_published_paths(context, file_id, path, source);
}

/// What a package ships. npm's `files` field states it exactly --
/// openzeppelin publishes `/contracts/**/*.sol` and nothing else -- and
/// without it the graph reads a repository's own build tooling as code
/// that ships, so a dev dependency imported there looks like a mistake.
pub(crate) fn index_published_paths(
    context: &mut IndexContext,
    file_id: NodeId,
    path: &Path,
    source: &str,
) {
    if path.file_name().and_then(|name| name.to_str()) != Some("package.json") {
        return;
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(source) else {
        return;
    };
    let Some(files) = value.get("files").and_then(|files| files.as_array()) else {
        return;
    };
    let published: Vec<&str> = files
        .iter()
        .filter_map(|entry| entry.as_str())
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .collect();
    if published.is_empty() {
        return;
    }
    add_file_metadata(
        &mut context.graph,
        file_id,
        "published_paths",
        published.join("\n"),
    );
}

/// A `.properties` file states settings the program reads by name:
/// spring-petclinic writes `spring.sql.init.schema-locations=classpath*:db/
/// ${database}/schema.sql`, which declares one setting and reads another.
/// Fifty such files across the corpus held 702 of them and the graph held
/// none.
pub(crate) fn index_properties_settings(
    context: &mut IndexContext,
    file_id: NodeId,
    label: &str,
    source: &str,
) {
    if !label.ends_with(".properties") || names_a_resource_bundle(label) {
        return;
    }
    for (index, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() || key.contains(char::is_whitespace) {
            continue;
        }
        let line_number = index as u32 + 1;
        let span = SourceSpan {
            path: label.to_string(),
            start_line: line_number,
            start_column: 0,
            end_line: line_number,
            end_column: line.chars().count() as u32,
        };
        let setting = shared_effect_entity(
            context,
            "config",
            NodeKind::Config,
            key,
            span.clone(),
            BTreeMap::from([
                ("item_kind".to_string(), "setting".to_string()),
                ("source".to_string(), "properties".to_string()),
            ]),
        );
        let value = value.trim();
        let mut metadata = BTreeMap::from([
            ("item_kind".to_string(), "setting".to_string()),
            ("source".to_string(), "properties".to_string()),
            ("file".to_string(), label.to_string()),
            ("line".to_string(), line_number.to_string()),
        ]);
        if !value.is_empty() {
            metadata.insert(
                "value".to_string(),
                value.chars().take(120).collect::<String>(),
            );
        }
        context.graph.add_edge_with_metadata(
            file_id,
            setting,
            EdgeKind::Defines,
            Confidence::Exact,
            metadata,
        );
        // `${database}` in a value is this file reading another setting.
        for referenced in placeholder_names(value) {
            if referenced == key {
                continue;
            }
            let read = shared_effect_entity(
                context,
                "config",
                NodeKind::Config,
                &referenced,
                span.clone(),
                BTreeMap::from([
                    ("item_kind".to_string(), "setting".to_string()),
                    ("source".to_string(), "properties".to_string()),
                ]),
            );
            add_edge_once_with_metadata(
                context,
                file_id,
                read,
                EdgeKind::ReadsConfig,
                Confidence::Exact,
                BTreeMap::from([
                    ("item_kind".to_string(), "config_read".to_string()),
                    ("source".to_string(), "properties".to_string()),
                    ("file".to_string(), label.to_string()),
                    ("line".to_string(), line_number.to_string()),
                ]),
            );
        }
    }
}

/// Whether a `.properties` file holds translations rather than settings.
/// Java writes a resource bundle as one file per locale —
/// `messages_de.properties` beside `messages.properties` — usually under a
/// directory that says so, and its keys are a program's words rather than
/// its configuration.
fn names_a_resource_bundle(label: &str) -> bool {
    let normalized = label.to_ascii_lowercase();
    if normalized.split('/').any(|segment| {
        matches!(
            segment,
            "messages" | "i18n" | "locale" | "locales" | "translations" | "lang"
        )
    }) {
        return true;
    }
    let stem = normalized
        .rsplit('/')
        .next()
        .unwrap_or(&normalized)
        .trim_end_matches(".properties");
    // `_de`, `_pt_br`: a locale suffix is two letters, and a country adds
    // two more.
    let mut parts = stem.rsplit('_');
    let last = parts.next().unwrap_or_default();
    let previous = parts.next().unwrap_or_default();
    let looks_like_locale = |part: &str| {
        part.len() == 2
            && part
                .chars()
                .all(|character| character.is_ascii_alphabetic())
    };
    stem.contains('_')
        && (looks_like_locale(last) || (looks_like_locale(previous) && last.len() <= 3))
}

/// The names a value asks to be filled in: `${database}` and
/// `${DB_HOST:localhost}`, whose default is not part of the name.
fn placeholder_names(value: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = value;
    while let Some(start) = rest.find("${") {
        rest = &rest[start + 2..];
        let Some(end) = rest.find('}') else {
            break;
        };
        let inner = &rest[..end];
        rest = &rest[end + 1..];
        let name = inner.split(':').next().unwrap_or(inner).trim();
        if !name.is_empty() && !name.contains(char::is_whitespace) {
            names.push(name.to_string());
        }
    }
    names
}

pub(crate) fn index_script_entrypoint(
    context: &mut IndexContext,
    file_id: NodeId,
    label: &str,
    source: &str,
) -> Option<NodeId> {
    let (interpreter, language) = shebang_interpreter(source)?;
    let mut metadata = BTreeMap::new();
    metadata.insert("item_kind".to_string(), "script_entrypoint".to_string());
    metadata.insert("entrypoint_kind".to_string(), "script".to_string());
    metadata.insert("source".to_string(), "shebang".to_string());
    metadata.insert("target".to_string(), label.to_string());
    metadata.insert("interpreter".to_string(), interpreter.to_string());
    metadata.insert("language".to_string(), language.to_string());

    // The shebang is what makes the file a program, and it is the first
    // line of it: a reader following this entrypoint lands there.
    let shebang = source.lines().next().unwrap_or_default();
    let entrypoint_id = context.graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        format!("script:{label}"),
        Some(SourceSpan {
            path: label.to_string(),
            start_line: 1,
            start_column: 0,
            end_line: 1,
            end_column: shebang.chars().count() as u32,
        }),
        metadata,
    );
    add_edge_once(
        context,
        file_id,
        entrypoint_id,
        EdgeKind::Contains,
        Confidence::Exact,
    );
    let root_id = context.graph.root;
    add_edge_once(
        context,
        root_id,
        entrypoint_id,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );
    add_entrypoint_reference(
        context,
        entrypoint_id,
        file_id,
        "entrypoint_file",
        "shebang_path",
        Confidence::Exact,
        None,
    );

    Some(entrypoint_id)
}

pub(crate) fn index_framework_routes(
    context: &mut IndexContext,
    file_id: NodeId,
    label: &str,
    language: Language,
    source: &str,
    parsed: &ParsedFile,
    local_functions: &BTreeMap<String, NodeId>,
) {
    for route in framework_routes(language, source) {
        // The detectors read text, so they cannot tell a route from an
        // example of one. flask documents `@app.route("/")` inside a
        // docstring, and that line claimed about 140 functions as its
        // handler and made every file holding it look like a served
        // entrypoint.
        if parsed.line_is_quoted(route.line) {
            continue;
        }
        // A route names a path. `app.get('json escape')` reads a setting,
        // and express reads eleven of them in `lib/response.js` alone; all
        // 1324 real routes across the corpora start with `/`, a wildcard, a
        // regex anchor, or a parameter.
        if !names_a_route_path(&route.path) {
            continue;
        }
        let mut metadata = BTreeMap::new();
        metadata.insert("item_kind".to_string(), "framework_route".to_string());
        metadata.insert("entrypoint_kind".to_string(), "route".to_string());
        metadata.insert("source".to_string(), "framework".to_string());
        metadata.insert("language".to_string(), language.to_string());
        metadata.insert("framework".to_string(), route.framework.clone());
        metadata.insert("method".to_string(), route.method.clone());
        metadata.insert("path".to_string(), route.path.clone());
        metadata.insert("target".to_string(), label.to_string());
        metadata.insert("line".to_string(), route.line.to_string());
        if let Some(handler) = route.handler.as_deref() {
            metadata.insert("handler".to_string(), handler.to_string());
        }

        let entrypoint_id = context.graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            format!("route {} {}", route.method, route.path),
            Some(line_span(label, source, route.line)),
            metadata,
        );
        add_edge_once(
            context,
            file_id,
            entrypoint_id,
            EdgeKind::Contains,
            Confidence::Syntactic,
        );
        let root_id = context.graph.root;
        add_edge_once(
            context,
            root_id,
            entrypoint_id,
            EdgeKind::Entrypoint,
            Confidence::Syntactic,
        );
        add_entrypoint_reference(
            context,
            entrypoint_id,
            file_id,
            "entrypoint_file",
            "framework_route_file",
            Confidence::Syntactic,
            None,
        );

        if let Some(handler) = route.handler.as_deref() {
            if let Some(handler_id) = resolve_local_function(local_functions, handler) {
                add_entrypoint_reference(
                    context,
                    entrypoint_id,
                    handler_id,
                    "entrypoint_function",
                    "framework_route_handler",
                    Confidence::Syntactic,
                    Some(handler),
                );
            } else {
                // The handler may live in another module of the same crate;
                // retry against the global function registry after the scan.
                context.pending_route_handlers.push(PendingRouteHandler {
                    entrypoint: entrypoint_id,
                    handler: handler.to_string(),
                });
            }
        }
    }
}

/// Whether a string reads as the path half of a route declaration.
fn names_a_route_path(path: &str) -> bool {
    path.starts_with(['/', '*', '^', ':'])
}

pub(crate) fn framework_routes(language: Language, source: &str) -> Vec<FrameworkRoute> {
    match language {
        Language::Python => python_framework_routes(source),
        Language::JavaScript | Language::TypeScript | Language::Tsx => js_framework_routes(source),
        Language::Rust => rust_framework_routes(source),
        Language::Go => go_framework_routes(source),
        Language::Php => php_framework_routes(source),
        Language::Ruby => ruby_framework_routes(source),
        Language::Java | Language::Kotlin => jvm_framework_routes(source),
        Language::CSharp => csharp_framework_routes(source),
        Language::C
        | Language::Cpp
        | Language::ObjectiveC
        | Language::Dart
        | Language::Hcl
        | Language::Solidity
        | Language::Proto
        | Language::GraphQl
        | Language::Bash
        | Language::Swift
        | Language::Scala
        | Language::Lua
        | Language::Elixir
        | Language::Zig
        | Language::Haskell
        | Language::OCaml
        | Language::Julia
        | Language::Erlang
        | Language::Nix
        | Language::R => Vec::new(),
    }
}

/// Whether a line is a comment in a curly-brace language: `//`, or a line
/// of a block comment as it is usually written.
fn line_is_a_javascript_comment(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*')
}

/// Whether a block comment is open after this line. Only the last `/*` or
/// `*/` on the line decides, which is what a source file's comments look
/// like.
fn block_comment_continues(line: &str, was_open: bool) -> bool {
    match (line.rfind("/*"), line.rfind("*/")) {
        (Some(open), Some(close)) => open > close,
        (Some(_), None) => true,
        (None, Some(_)) => false,
        (None, None) => was_open,
    }
}

pub(crate) fn index_commonjs_require_imports(
    context: &mut IndexContext,
    file_id: NodeId,
    label: &str,
    language: Language,
    source: &str,
) {
    if !matches!(
        language,
        Language::JavaScript | Language::TypeScript | Language::Tsx
    ) {
        return;
    }

    let mut in_block_comment = false;
    for (index, line) in source.lines().enumerate() {
        // Express documents `app.engine('ejs', require('ejs').__express)` in
        // a comment above the method, and reading it as an import made the
        // project depend on a package it only mentions.
        let was_in_block_comment = in_block_comment;
        in_block_comment = block_comment_continues(line, in_block_comment);
        if was_in_block_comment || line_is_a_javascript_comment(line) {
            continue;
        }
        let Some(require_call) = commonjs_require_call(line) else {
            continue;
        };

        let mut metadata = BTreeMap::new();
        metadata.insert("language".to_string(), language.to_string());
        metadata.insert("parser".to_string(), "syntax-pattern".to_string());
        metadata.insert("item_kind".to_string(), "import".to_string());
        metadata.insert("import_style".to_string(), "commonjs".to_string());

        let local_import = local_import_target(language, label, &require_call, &[], &[]);
        if let Some(local_import) = local_import.as_ref() {
            metadata.insert("import_scope".to_string(), "local".to_string());
            metadata.insert("import_target".to_string(), local_import.target.clone());
            metadata.insert("resolution".to_string(), "pending".to_string());
        }

        let import_id = context.graph.add_node_with_metadata(
            NodeKind::ExternalDependency,
            require_call,
            Some(line_span(label, source, index as u32 + 1)),
            metadata,
        );
        add_edge_once(
            context,
            file_id,
            import_id,
            EdgeKind::Imports,
            Confidence::Syntactic,
        );
        if let Some(local_import) = local_import {
            context.pending_local_imports.push(PendingLocalImport {
                import_node: import_id,
                target: local_import.target,
                candidates: local_import.candidates,
                mark_unresolved: true,
                allow_suffix_fallback: true,
            });
        }
    }
}

pub(crate) fn commonjs_require_call(line: &str) -> Option<String> {
    let mut search_start = 0;
    while let Some(offset) = line[search_start..].find("require(") {
        let start = search_start + offset;
        let before = line[..start].chars().next_back();
        if before.is_none_or(|character| !is_identifier_or_member_character(character)) {
            // `require(resolve(`package.json`))` asks for whatever that
            // call returns. Reaching past it for the first quote inside
            // reported vue as importing a package called `package.json`.
            let argument = line[start + "require(".len()..].trim_start();
            let module = argument
                .starts_with(['"', '\'', '`'])
                .then(|| first_quoted_value(argument))
                .flatten()?;
            // `require('../src/commands/' + name)` builds its path as it
            // runs, so the literal in front is a prefix rather than a file:
            // redis writes one, and the graph went looking for
            // `src/commands.js`.
            let after = argument[module.len() + 2..].trim_start();
            if !after.starts_with(')') {
                return None;
            }
            return Some(format!("require(\"{module}\")"));
        }
        search_start = start + "require(".len();
    }
    None
}

pub(crate) fn index_dart_platform_channels(
    context: &mut IndexContext,
    file_id: NodeId,
    label: &str,
    source: &str,
) {
    for channel in dart_platform_channels(source) {
        let mut metadata = BTreeMap::new();
        metadata.insert("item_kind".to_string(), "platform_channel".to_string());
        metadata.insert("source".to_string(), "dart".to_string());
        metadata.insert("language".to_string(), "dart".to_string());
        metadata.insert("framework".to_string(), "flutter".to_string());
        metadata.insert("channel_kind".to_string(), channel.channel_kind.clone());
        metadata.insert("channel_name".to_string(), channel.name.clone());
        metadata.insert("line".to_string(), channel.line.to_string());
        let channel_id = context.graph.add_node_with_metadata(
            NodeKind::ExternalDependency,
            format!("flutter {} channel:{}", channel.channel_kind, channel.name),
            Some(line_span(label, source, channel.line)),
            metadata,
        );
        let mut edge_metadata = BTreeMap::new();
        edge_metadata.insert("source".to_string(), "dart".to_string());
        edge_metadata.insert("relation".to_string(), "platform_channel".to_string());
        edge_metadata.insert("channel_kind".to_string(), channel.channel_kind);
        add_edge_once_with_metadata(
            context,
            file_id,
            channel_id,
            EdgeKind::References,
            Confidence::Syntactic,
            edge_metadata,
        );
    }
}

/// Platform for a native Flutter host source file, by extension.
pub(crate) fn native_platform_for_path(label: &str) -> Option<&'static str> {
    let extension = label.rsplit('.').next()?;
    match extension {
        "kt" | "kts" | "java" => Some("android"),
        "swift" | "m" | "mm" => Some("ios"),
        _ => None,
    }
}

/// Collect Flutter channel registrations from native Android/iOS sources
/// (Kotlin/Java/Swift/Objective-C are not parsed languages, so this is a
/// deterministic line scan for the channel constructors).
pub(crate) fn index_native_platform_channels(
    context: &mut IndexContext,
    file_id: NodeId,
    label: &str,
    source: &str,
) {
    let Some(platform) = native_platform_for_path(label) else {
        return;
    };
    for (index, line) in source.lines().enumerate() {
        // Flutter-prefixed constructors first: `MethodChannel(` is a
        // substring of `FlutterMethodChannel(`, and one line registers at
        // most one channel.
        for (constructor, channel_kind) in [
            ("FlutterMethodChannel(", "method"),
            ("FlutterEventChannel(", "event"),
            ("FlutterBasicMessageChannel(", "basic_message"),
            ("MethodChannel(", "method"),
            ("EventChannel(", "event"),
            ("BasicMessageChannel(", "basic_message"),
        ] {
            if let Some(name) = first_quoted_value_after(line, constructor) {
                context
                    .pending_native_channel_handlers
                    .push(PendingNativeChannelHandler {
                        file: file_id,
                        label: label.to_string(),
                        name,
                        channel_kind: channel_kind.to_string(),
                        platform,
                        line: index as u32 + 1,
                    });
                break;
            }
        }
    }
}

/// Match native channel registrations to Dart channel declarations by
/// channel name and kind: link the native handler file to the channel node
/// and record the handler path on the channel for insight checks.
pub(crate) fn resolve_pending_native_channel_handlers(context: &mut IndexContext) {
    if context.pending_native_channel_handlers.is_empty() {
        return;
    }
    let mut channels: BTreeMap<(String, String), Vec<NodeId>> = BTreeMap::new();
    for node in &context.graph.nodes {
        if node.metadata.get("item_kind").map(String::as_str) == Some("platform_channel")
            && node.metadata.get("source").map(String::as_str) == Some("dart")
            && let (Some(name), Some(kind)) = (
                node.metadata.get("channel_name"),
                node.metadata.get("channel_kind"),
            )
        {
            channels
                .entry((name.clone(), kind.clone()))
                .or_default()
                .push(node.id);
        }
    }
    let pending = std::mem::take(&mut context.pending_native_channel_handlers);
    for handler in pending {
        let Some(channel_ids) = channels.get(&(handler.name.clone(), handler.channel_kind.clone()))
        else {
            continue;
        };
        for channel_id in channel_ids {
            let mut metadata = BTreeMap::new();
            metadata.insert("source".to_string(), "native".to_string());
            metadata.insert(
                "relation".to_string(),
                "platform_channel_handler".to_string(),
            );
            metadata.insert("platform".to_string(), handler.platform.to_string());
            metadata.insert("line".to_string(), handler.line.to_string());
            add_edge_once_with_metadata(
                context,
                handler.file,
                *channel_id,
                EdgeKind::References,
                Confidence::Heuristic,
                metadata,
            );
            let key = format!("native_handler_{}", handler.platform);
            if let Some(node) = context
                .graph
                .nodes
                .iter_mut()
                .find(|node| node.id == *channel_id)
                && !node.metadata.contains_key(&key)
            {
                node.metadata.insert(key, handler.label.clone());
            }
        }
    }
}

pub(crate) fn dart_platform_channels(source: &str) -> Vec<DartPlatformChannel> {
    let mut channels = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        for (constructor, channel_kind) in [
            ("MethodChannel(", "method"),
            ("EventChannel(", "event"),
            ("BasicMessageChannel(", "basic_message"),
        ] {
            if let Some(name) = first_quoted_value_after(line, constructor) {
                channels.push(DartPlatformChannel {
                    name,
                    channel_kind: channel_kind.to_string(),
                    line: line_number,
                });
            }
        }
    }
    channels
}

pub(crate) fn is_identifier_or_member_character(character: char) -> bool {
    character == '_' || character == '$' || character == '.' || character.is_ascii_alphanumeric()
}

pub(crate) fn index_framework_configs(
    context: &mut IndexContext,
    file_id: NodeId,
    label: &str,
    language: Option<Language>,
    source: &str,
) {
    for config in framework_configs(label, language, source) {
        let mut metadata = BTreeMap::new();
        metadata.insert("item_kind".to_string(), "framework_config".to_string());
        metadata.insert("source".to_string(), "framework".to_string());
        metadata.insert("framework".to_string(), config.framework.clone());
        metadata.insert("config_kind".to_string(), config.config_kind.clone());
        metadata.insert("target".to_string(), label.to_string());
        metadata.insert("line".to_string(), config.line.to_string());
        if let Some(language) = language {
            metadata.insert("language".to_string(), language.to_string());
        }
        if let Some(value) = config.value.as_deref() {
            metadata.insert("value".to_string(), value.to_string());
        }

        let config_id = context.graph.add_node_with_metadata(
            NodeKind::Config,
            config.label,
            Some(line_span(label, source, config.line)),
            metadata,
        );
        let mut edge_metadata = BTreeMap::new();
        edge_metadata.insert("source".to_string(), "framework".to_string());
        edge_metadata.insert("framework".to_string(), config.framework);
        edge_metadata.insert("config_kind".to_string(), config.config_kind);
        add_edge_once_with_metadata(
            context,
            file_id,
            config_id,
            EdgeKind::ReadsConfig,
            Confidence::Syntactic,
            edge_metadata,
        );
    }
}

pub(crate) fn framework_configs(
    label: &str,
    language: Option<Language>,
    source: &str,
) -> Vec<FrameworkConfig> {
    let mut configs = BTreeSet::new();
    configs.extend(file_framework_configs(label));

    match language {
        Some(Language::Python) => configs.extend(python_framework_configs(source)),
        Some(Language::JavaScript | Language::TypeScript | Language::Tsx) => {
            configs.extend(js_framework_configs(source))
        }
        Some(Language::Rust) => configs.extend(rust_framework_configs(source)),
        // A configuration language states its settings outright, and a
        // schema states shapes; nothing in either is a framework's own file.
        Some(Language::Hcl | Language::Proto | Language::GraphQl | Language::Solidity) => {}
        // An Objective-C project states its routes in code the same way a C
        // one does: nothing here reads them from a framework's own file.
        Some(Language::ObjectiveC) => {}
        Some(Language::Go) => configs.extend(go_framework_configs(source)),
        Some(Language::Php) => configs.extend(php_framework_configs(source)),
        Some(Language::Bash) => configs.extend(bash_framework_configs(source)),
        Some(Language::Dart) => configs.extend(dart_framework_configs(source)),
        Some(
            Language::C
            | Language::Cpp
            | Language::Ruby
            | Language::Java
            | Language::CSharp
            | Language::Kotlin
            | Language::Swift
            | Language::Scala
            | Language::Lua
            | Language::Elixir
            | Language::Zig
            | Language::Haskell
            | Language::OCaml
            | Language::Julia
            | Language::Erlang
            | Language::Nix
            | Language::R,
        )
        | None => {}
    }

    configs.into_iter().collect()
}

/// The span from `start` through `end`, for a block written across lines --
/// a workflow job holds its steps, and a reader asking to see the job wants
/// all of them.
pub(crate) fn block_span(path: &str, source: &str, start: u32, end: u32) -> SourceSpan {
    let first = line_span(path, source, start);
    let last = line_span(path, source, end.max(start));
    SourceSpan {
        end_line: last.start_line,
        end_column: last.end_column,
        ..first
    }
}

pub(crate) fn line_span(path: &str, source: &str, line: u32) -> SourceSpan {
    let line = line.max(1);
    let line_text = source
        .lines()
        .nth(line.saturating_sub(1) as usize)
        .unwrap_or("");
    SourceSpan {
        path: path.to_string(),
        start_line: line,
        start_column: 1,
        end_line: line,
        end_column: line_text.chars().count() as u32 + 1,
    }
}
