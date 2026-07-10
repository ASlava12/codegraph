//! File-level detectors dispatched per source file: script entrypoints,
//! framework routes and configs, CommonJS requires, and Dart/native
//! platform channels.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use codegraph_core::{Confidence, EdgeKind, NodeId, NodeKind, SourceSpan};
use codegraph_parser::Language;

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

    let entrypoint_id = context.graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        format!("script:{label}"),
        None,
        metadata,
    );
    add_edge_once(
        &mut context.graph,
        file_id,
        entrypoint_id,
        EdgeKind::Contains,
        Confidence::Exact,
    );
    let root_id = context.graph.root;
    add_edge_once(
        &mut context.graph,
        root_id,
        entrypoint_id,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );
    add_entrypoint_reference(
        &mut context.graph,
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
    local_functions: &BTreeMap<String, NodeId>,
) {
    for route in framework_routes(language, source) {
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
            &mut context.graph,
            file_id,
            entrypoint_id,
            EdgeKind::Contains,
            Confidence::Syntactic,
        );
        let root_id = context.graph.root;
        add_edge_once(
            &mut context.graph,
            root_id,
            entrypoint_id,
            EdgeKind::Entrypoint,
            Confidence::Syntactic,
        );
        add_entrypoint_reference(
            &mut context.graph,
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
                    &mut context.graph,
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

pub(crate) fn framework_routes(language: Language, source: &str) -> Vec<FrameworkRoute> {
    match language {
        Language::Python => python_framework_routes(source),
        Language::JavaScript | Language::TypeScript | Language::Tsx => js_framework_routes(source),
        Language::Rust => rust_framework_routes(source),
        Language::Go => go_framework_routes(source),
        Language::Php => php_framework_routes(source),
        Language::C | Language::Cpp | Language::Dart | Language::Bash => Vec::new(),
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

    for (index, line) in source.lines().enumerate() {
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
            &mut context.graph,
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
            let rest = &line[start..];
            let module = first_quoted_value_after(rest, "require(")?;
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
            &mut context.graph,
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
                &mut context.graph,
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
            &mut context.graph,
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
        Some(Language::Go) => configs.extend(go_framework_configs(source)),
        Some(Language::Php) => configs.extend(php_framework_configs(source)),
        Some(Language::Bash) => configs.extend(bash_framework_configs(source)),
        Some(Language::Dart) => configs.extend(dart_framework_configs(source)),
        Some(Language::C | Language::Cpp) | None => {}
    }

    configs.into_iter().collect()
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
