use codegraph_core::{CodeGraph, Confidence, EdgeKind, NodeId, NodeKind, SourceSpan};
use codegraph_parser::{Language, ParsedItemKind, parse_source};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use walkdir::{DirEntry, WalkDir};

#[derive(Debug, Error)]
pub enum IndexError {
    #[error("failed to walk project tree at {path}: {source}")]
    Walk {
        path: PathBuf,
        #[source]
        source: walkdir::Error,
    },
}

#[derive(Debug, Clone)]
pub struct IndexOptions {
    pub include_hidden: bool,
    pub include_ignored: bool,
    pub max_file_size: u64,
    pub ignored_names: BTreeSet<String>,
}

struct IndexContext {
    graph: CodeGraph,
    function_symbols: BTreeMap<String, Vec<NodeId>>,
    file_nodes: BTreeMap<String, NodeId>,
    external_dependencies: BTreeMap<String, NodeId>,
    cargo_workspace_dependencies: BTreeMap<String, Option<String>>,
    pending_calls: Vec<PendingCall>,
    pending_entrypoint_targets: Vec<PendingEntrypointTarget>,
}

struct PendingCall {
    caller: NodeId,
    label: String,
    span: SourceSpan,
    language: String,
}

struct PendingEntrypointTarget {
    entrypoint: NodeId,
    manifest_label: String,
    target: String,
    ecosystem: String,
    entrypoint_kind: String,
}

struct EntrypointTargetCandidate {
    path: String,
    symbol: Option<String>,
    file_confidence: Confidence,
    function_confidence: Confidence,
    resolution: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ManifestDependency {
    name: String,
    kind: String,
    ecosystem: String,
    version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ManifestEntrypoint {
    label: String,
    kind: String,
    ecosystem: String,
    target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FrameworkRoute {
    framework: String,
    method: String,
    path: String,
    handler: Option<String>,
    line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct FrameworkConfig {
    framework: String,
    label: String,
    config_kind: String,
    value: Option<String>,
    line: u32,
}

impl Default for IndexOptions {
    fn default() -> Self {
        Self {
            include_hidden: false,
            include_ignored: false,
            max_file_size: 2 * 1024 * 1024,
            ignored_names: default_ignored_names(),
        }
    }
}

pub fn scan_project(
    root: impl AsRef<Path>,
    options: &IndexOptions,
) -> Result<CodeGraph, IndexError> {
    let root = root.as_ref();
    let root_label = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(".");
    let cargo_workspace_dependencies = cargo_workspace_dependencies(root);
    let mut context = IndexContext {
        graph: CodeGraph::new(root_label),
        function_symbols: BTreeMap::new(),
        file_nodes: BTreeMap::new(),
        external_dependencies: BTreeMap::new(),
        cargo_workspace_dependencies,
        pending_calls: Vec::new(),
        pending_entrypoint_targets: Vec::new(),
    };

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|entry| should_enter(entry, options))
    {
        let entry = entry.map_err(|source| IndexError::Walk {
            path: root.to_path_buf(),
            source,
        })?;
        let path = entry.path();

        if path == root {
            continue;
        }

        let Ok(relative_path) = path.strip_prefix(root) else {
            continue;
        };
        let label = relative_path.to_string_lossy().replace('\\', "/");

        if entry.file_type().is_dir() {
            let id = context.graph.add_node(NodeKind::Directory, label);
            context.graph.add_edge(
                context.graph.root,
                id,
                EdgeKind::Contains,
                Confidence::Exact,
            );
            continue;
        }

        if entry.file_type().is_file() && is_probably_source_file(path, options.max_file_size) {
            index_file(&mut context, path, &label);
        }
    }

    resolve_pending_calls(&mut context);
    resolve_pending_entrypoint_targets(&mut context);

    Ok(context.graph)
}

fn index_file(context: &mut IndexContext, path: &Path, label: &str) {
    let mut metadata = BTreeMap::new();
    let source_bytes = fs::read(path)
        .map_err(|error| {
            metadata.insert("read_error".to_string(), error.to_string());
        })
        .ok();
    let language = Language::detect(path).or_else(|| {
        source_bytes
            .as_deref()
            .and_then(|source| std::str::from_utf8(source).ok())
            .and_then(shebang_language)
    });

    if let Some(language) = language {
        metadata.insert("language".to_string(), language.to_string());
    }

    let parse_result = language.and_then(|language| {
        source_bytes
            .as_ref()
            .map(|source| (language, parse_source(label, source, language)))
    });

    let file_id = context
        .graph
        .add_node_with_metadata(NodeKind::File, label, None, metadata);
    context.file_nodes.insert(label.to_string(), file_id);
    context.graph.add_edge(
        context.graph.root,
        file_id,
        EdgeKind::Contains,
        Confidence::Exact,
    );

    let source_text = fs::read_to_string(path).ok();
    let mut script_entrypoint = None;
    if let Some(source) = source_text.as_deref() {
        script_entrypoint = index_script_entrypoint(context, file_id, label, source);
        index_manifest_facts(context, file_id, path, label, source);
        index_framework_configs(context, file_id, label, language, source);
    }

    if let Some((language, parse_result)) = parse_result {
        match parse_result {
            Ok(parsed) => {
                if parsed.has_error_nodes {
                    add_file_metadata(&mut context.graph, file_id, "syntax_errors", "true");
                }

                let mut local_functions = BTreeMap::new();
                for item in parsed.items.iter().filter(|item| is_symbol_item(item.kind)) {
                    let node_kind = match item.kind {
                        ParsedItemKind::Function | ParsedItemKind::Entrypoint => NodeKind::Function,
                        ParsedItemKind::Type => NodeKind::Type,
                        ParsedItemKind::Module => NodeKind::Module,
                        ParsedItemKind::Import => NodeKind::ExternalDependency,
                        ParsedItemKind::Call
                        | ParsedItemKind::EnvironmentRead
                        | ParsedItemKind::ConfigRead
                        | ParsedItemKind::Error => {
                            unreachable!("non-symbol facts are processed separately")
                        }
                    };
                    let mut item_metadata = BTreeMap::new();
                    item_metadata.insert("language".to_string(), language.to_string());
                    item_metadata.insert("parser".to_string(), "tree-sitter".to_string());
                    item_metadata.insert(
                        "item_kind".to_string(),
                        parsed_item_kind_name(item.kind).to_string(),
                    );

                    let item_id = context.graph.add_node_with_metadata(
                        node_kind,
                        item.label.clone(),
                        Some(item.span.clone()),
                        item_metadata,
                    );
                    let edge_kind = match item.kind {
                        ParsedItemKind::Import => EdgeKind::Imports,
                        _ => EdgeKind::Contains,
                    };
                    context
                        .graph
                        .add_edge(file_id, item_id, edge_kind, Confidence::Syntactic);

                    if item.kind == ParsedItemKind::Entrypoint {
                        context.graph.add_edge(
                            context.graph.root,
                            item_id,
                            EdgeKind::Entrypoint,
                            Confidence::Syntactic,
                        );
                    }

                    if matches!(
                        item.kind,
                        ParsedItemKind::Function | ParsedItemKind::Entrypoint
                    ) {
                        register_function_symbol(
                            &mut context.function_symbols,
                            &item.label,
                            item_id,
                        );
                        register_local_function(&mut local_functions, &item.label, item_id);
                    }
                }

                if let Some(entrypoint_id) = script_entrypoint
                    && let Some(main_id) = resolve_local_function(&local_functions, "main")
                {
                    add_entrypoint_reference(
                        &mut context.graph,
                        entrypoint_id,
                        main_id,
                        "entrypoint_function",
                        "shebang_main",
                        Confidence::Syntactic,
                        Some("main"),
                    );
                }

                if let Some(source) = source_text.as_deref() {
                    index_framework_routes(
                        context,
                        file_id,
                        label,
                        language,
                        source,
                        &local_functions,
                    );
                }

                for item in parsed.items.iter().filter(|item| is_effect_item(item.kind)) {
                    let source_id = item
                        .parent
                        .as_deref()
                        .and_then(|parent| resolve_local_function(&local_functions, parent))
                        .unwrap_or(file_id);
                    let node_kind = match item.kind {
                        ParsedItemKind::EnvironmentRead => NodeKind::Environment,
                        ParsedItemKind::ConfigRead => NodeKind::Config,
                        ParsedItemKind::Error => NodeKind::Unknown,
                        _ => unreachable!("only effect facts are processed here"),
                    };
                    let edge_kind = match item.kind {
                        ParsedItemKind::EnvironmentRead => EdgeKind::ReadsEnvironment,
                        ParsedItemKind::ConfigRead => EdgeKind::ReadsConfig,
                        ParsedItemKind::Error => EdgeKind::MayError,
                        _ => unreachable!("only effect facts are processed here"),
                    };
                    let mut item_metadata = BTreeMap::new();
                    item_metadata.insert("language".to_string(), language.to_string());
                    item_metadata.insert("parser".to_string(), "tree-sitter".to_string());
                    item_metadata.insert(
                        "item_kind".to_string(),
                        parsed_item_kind_name(item.kind).to_string(),
                    );
                    if let Some(parent) = item.parent.as_deref() {
                        item_metadata.insert("parent".to_string(), parent.to_string());
                    }

                    let item_id = context.graph.add_node_with_metadata(
                        node_kind,
                        item.label.clone(),
                        Some(item.span.clone()),
                        item_metadata,
                    );
                    add_edge_once(
                        &mut context.graph,
                        source_id,
                        item_id,
                        edge_kind,
                        Confidence::Heuristic,
                    );
                }

                for item in parsed
                    .items
                    .iter()
                    .filter(|item| item.kind == ParsedItemKind::Call)
                {
                    let Some(parent) = item.parent.as_deref() else {
                        continue;
                    };
                    let Some(caller) = resolve_local_function(&local_functions, parent) else {
                        continue;
                    };
                    context.pending_calls.push(PendingCall {
                        caller,
                        label: item.label.clone(),
                        span: item.span.clone(),
                        language: language.to_string(),
                    });
                }
            }
            Err(error) => add_file_metadata(
                &mut context.graph,
                file_id,
                "parse_error",
                error.to_string(),
            ),
        }
    }
}

fn index_manifest_facts(
    context: &mut IndexContext,
    file_id: NodeId,
    path: &Path,
    label: &str,
    source: &str,
) {
    index_manifest_dependencies(context, file_id, path, source);
    index_manifest_entrypoints(context, file_id, path, label, source);
}

fn index_script_entrypoint(
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

fn index_framework_routes(
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
            None,
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

        if let Some(handler) = route.handler.as_deref()
            && let Some(handler_id) = resolve_local_function(local_functions, handler)
        {
            add_entrypoint_reference(
                &mut context.graph,
                entrypoint_id,
                handler_id,
                "entrypoint_function",
                "framework_route_handler",
                Confidence::Syntactic,
                Some(handler),
            );
        }
    }
}

fn framework_routes(language: Language, source: &str) -> Vec<FrameworkRoute> {
    match language {
        Language::Python => python_framework_routes(source),
        Language::JavaScript | Language::TypeScript | Language::Tsx => js_framework_routes(source),
        Language::Rust => rust_framework_routes(source),
        Language::Go => go_framework_routes(source),
        Language::Php => php_framework_routes(source),
        Language::C | Language::Cpp | Language::Bash => Vec::new(),
    }
}

fn index_framework_configs(
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

        let config_id =
            context
                .graph
                .add_node_with_metadata(NodeKind::Config, config.label, None, metadata);
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

fn framework_configs(
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
        Some(Language::C | Language::Cpp) | None => {}
    }

    configs.into_iter().collect()
}

fn index_manifest_dependencies(
    context: &mut IndexContext,
    file_id: NodeId,
    path: &Path,
    source: &str,
) {
    let dependencies = manifest_dependencies(path, source, &context.cargo_workspace_dependencies);
    for dependency in dependencies {
        let package_name = canonical_package_name(&dependency.ecosystem, &dependency.name);
        let package_id = package_id(&dependency.ecosystem, &package_name);
        let dependency_id = if let Some(id) = context.external_dependencies.get(&package_id) {
            *id
        } else {
            let mut metadata = BTreeMap::new();
            metadata.insert("item_kind".to_string(), "dependency".to_string());
            metadata.insert("ecosystem".to_string(), dependency.ecosystem.clone());
            metadata.insert("package_id".to_string(), package_id.clone());
            metadata.insert("source".to_string(), "manifest".to_string());
            if package_name != dependency.name {
                metadata.insert("declared_name".to_string(), dependency.name.clone());
            }
            let id = context.graph.add_node_with_metadata(
                NodeKind::ExternalDependency,
                package_name,
                None,
                metadata,
            );
            context.external_dependencies.insert(package_id, id);
            id
        };

        let mut edge_metadata = BTreeMap::new();
        edge_metadata.insert("dependency_kind".to_string(), dependency.kind);
        edge_metadata.insert("source".to_string(), "manifest".to_string());
        if let Some(version) = dependency.version {
            edge_metadata.insert("dependency_version".to_string(), version);
        }
        add_edge_once_with_metadata(
            &mut context.graph,
            file_id,
            dependency_id,
            EdgeKind::DependsOn,
            Confidence::Exact,
            edge_metadata,
        );
    }
}

fn index_manifest_entrypoints(
    context: &mut IndexContext,
    file_id: NodeId,
    path: &Path,
    label: &str,
    source: &str,
) {
    for entrypoint in manifest_entrypoints(path, source) {
        let mut metadata = BTreeMap::new();
        metadata.insert("item_kind".to_string(), "manifest_entrypoint".to_string());
        metadata.insert("entrypoint_kind".to_string(), entrypoint.kind.clone());
        metadata.insert("ecosystem".to_string(), entrypoint.ecosystem.clone());
        metadata.insert("source".to_string(), "manifest".to_string());
        if let Some(target) = entrypoint.target.as_deref() {
            metadata.insert("target".to_string(), target.to_string());
        }

        let entrypoint_id = context.graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            entrypoint.label,
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
        if let Some(target) = entrypoint.target {
            context
                .pending_entrypoint_targets
                .push(PendingEntrypointTarget {
                    entrypoint: entrypoint_id,
                    manifest_label: label.to_string(),
                    target,
                    ecosystem: entrypoint.ecosystem,
                    entrypoint_kind: entrypoint.kind,
                });
        }
    }
}

fn manifest_dependencies(
    path: &Path,
    source: &str,
    cargo_workspace_dependencies: &BTreeMap<String, Option<String>>,
) -> Vec<ManifestDependency> {
    match path.file_name().and_then(|name| name.to_str()) {
        Some("Cargo.toml") => cargo_dependencies(source, cargo_workspace_dependencies),
        Some("package.json") => package_json_dependencies(source),
        Some("go.mod") => go_mod_dependencies(source),
        Some("requirements.txt") => requirements_dependencies(source),
        Some("pyproject.toml") => pyproject_dependencies(source),
        Some("composer.json") => composer_dependencies(source),
        _ => Vec::new(),
    }
}

fn manifest_entrypoints(path: &Path, source: &str) -> Vec<ManifestEntrypoint> {
    match path.file_name().and_then(|name| name.to_str()) {
        Some("Cargo.toml") => cargo_entrypoints(path, source),
        Some("package.json") => package_json_entrypoints(source),
        Some("go.mod") => go_mod_entrypoints(path, source),
        Some("pyproject.toml") => pyproject_entrypoints(source),
        Some("composer.json") => composer_entrypoints(source),
        Some("CMakeLists.txt") => cmake_entrypoints(source),
        _ => Vec::new(),
    }
}

fn cargo_entrypoints(path: &Path, source: &str) -> Vec<ManifestEntrypoint> {
    let Ok(value) = toml::from_str::<toml::Value>(source) else {
        return Vec::new();
    };
    let mut entrypoints = Vec::new();

    if let Some(package_name) = value
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(|name| name.as_str())
        && path
            .parent()
            .map(|parent| parent.join("src").join("main.rs").is_file())
            .unwrap_or(false)
    {
        entrypoints.push(manifest_entrypoint(
            format!("cargo bin:{package_name}"),
            "binary",
            "cargo",
            Some("src/main.rs".to_string()),
        ));
    }

    collect_cargo_target_entrypoints(&value, "bin", "binary", &mut entrypoints);
    collect_cargo_target_entrypoints(&value, "example", "example", &mut entrypoints);
    entrypoints
}

fn collect_cargo_target_entrypoints(
    value: &toml::Value,
    table_name: &str,
    entrypoint_kind: &str,
    entrypoints: &mut Vec<ManifestEntrypoint>,
) {
    let Some(targets) = value.get(table_name).and_then(|value| value.as_array()) else {
        return;
    };

    for target in targets {
        let Some(name) = target
            .get("name")
            .and_then(|name| name.as_str())
            .map(str::to_string)
        else {
            continue;
        };
        let target_path = target
            .get("path")
            .and_then(|path| path.as_str())
            .map(str::to_string);
        entrypoints.push(manifest_entrypoint(
            format!("cargo {entrypoint_kind}:{name}"),
            entrypoint_kind,
            "cargo",
            target_path,
        ));
    }
}

fn package_json_entrypoints(source: &str) -> Vec<ManifestEntrypoint> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(source) else {
        return Vec::new();
    };
    let mut entrypoints = Vec::new();
    let Some(scripts) = value.get("scripts").and_then(|value| value.as_object()) else {
        return entrypoints;
    };

    for (name, command) in scripts {
        entrypoints.push(manifest_entrypoint(
            format!("npm script:{name}"),
            "script",
            "npm",
            command.as_str().map(str::to_string),
        ));
    }
    entrypoints
}

fn go_mod_entrypoints(path: &Path, source: &str) -> Vec<ManifestEntrypoint> {
    let Some(module) = go_module_name(source) else {
        return Vec::new();
    };
    let Some(root) = path.parent() else {
        return Vec::new();
    };

    let mut entrypoints = Vec::new();
    if root.join("main.go").is_file() {
        entrypoints.push(manifest_entrypoint(
            format!("go module:{module}"),
            "module",
            "go",
            Some("main.go".to_string()),
        ));
    }

    let cmd_dir = root.join("cmd");
    if let Ok(commands) = fs::read_dir(&cmd_dir) {
        for command in commands.flatten() {
            let command_path = command.path();
            if !command_path.join("main.go").is_file() {
                continue;
            }
            let Some(name) = command_path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            entrypoints.push(manifest_entrypoint(
                format!("go command:{name}"),
                "command",
                "go",
                Some(format!("cmd/{name}/main.go")),
            ));
        }
    }

    entrypoints
}

fn pyproject_entrypoints(source: &str) -> Vec<ManifestEntrypoint> {
    let Ok(value) = toml::from_str::<toml::Value>(source) else {
        return Vec::new();
    };
    let mut entrypoints = Vec::new();

    if let Some(project) = value.get("project") {
        collect_toml_entrypoint_keys(
            project,
            "scripts",
            "console_script",
            "python",
            &mut entrypoints,
        );
        collect_toml_entrypoint_keys(
            project,
            "gui-scripts",
            "gui_script",
            "python",
            &mut entrypoints,
        );
    }

    if let Some(poetry) = value.get("tool").and_then(|value| value.get("poetry")) {
        collect_toml_entrypoint_keys(
            poetry,
            "scripts",
            "poetry_script",
            "python",
            &mut entrypoints,
        );
    }

    entrypoints
}

fn composer_entrypoints(source: &str) -> Vec<ManifestEntrypoint> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(source) else {
        return Vec::new();
    };
    let mut entrypoints = Vec::new();

    if let Some(scripts) = value.get("scripts").and_then(|value| value.as_object()) {
        for (name, command) in scripts {
            let target = command.as_str().map(str::to_string).or_else(|| {
                command.as_array().map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str())
                        .collect::<Vec<_>>()
                        .join(" && ")
                })
            });
            entrypoints.push(manifest_entrypoint(
                format!("composer script:{name}"),
                "script",
                "composer",
                target,
            ));
        }
    }

    if let Some(bins) = value.get("bin").and_then(|value| value.as_array()) {
        for bin in bins {
            if let Some(path) = bin.as_str() {
                entrypoints.push(manifest_entrypoint(
                    format!("composer bin:{path}"),
                    "binary",
                    "composer",
                    Some(path.to_string()),
                ));
            }
        }
    }

    entrypoints
}

fn cmake_entrypoints(source: &str) -> Vec<ManifestEntrypoint> {
    cmake_command_bodies(source, "add_executable")
        .into_iter()
        .filter_map(|body| {
            let args = cmake_command_args(&body);
            let name = args.first()?.trim();
            if name.is_empty()
                || args.iter().any(|arg| arg.eq_ignore_ascii_case("IMPORTED"))
                || args
                    .get(1)
                    .is_some_and(|arg| arg.eq_ignore_ascii_case("ALIAS"))
            {
                return None;
            }

            let target = args
                .iter()
                .skip(1)
                .find(|arg| is_cmake_source_argument(arg))
                .cloned();
            Some(manifest_entrypoint(
                format!("cmake executable:{name}"),
                "executable",
                "cmake",
                target,
            ))
        })
        .collect()
}

fn cargo_dependencies(
    source: &str,
    cargo_workspace_dependencies: &BTreeMap<String, Option<String>>,
) -> Vec<ManifestDependency> {
    let Ok(value) = toml::from_str::<toml::Value>(source) else {
        return Vec::new();
    };
    let mut dependencies = Vec::new();
    collect_toml_table_keys(
        &value,
        "dependencies",
        "runtime",
        "cargo",
        &mut dependencies,
        Some(cargo_workspace_dependencies),
    );
    collect_toml_table_keys(
        &value,
        "dev-dependencies",
        "dev",
        "cargo",
        &mut dependencies,
        Some(cargo_workspace_dependencies),
    );
    collect_toml_table_keys(
        &value,
        "build-dependencies",
        "build",
        "cargo",
        &mut dependencies,
        Some(cargo_workspace_dependencies),
    );

    if let Some(targets) = value.get("target").and_then(|value| value.as_table()) {
        for target in targets.values() {
            collect_toml_table_keys(
                target,
                "dependencies",
                "runtime",
                "cargo",
                &mut dependencies,
                Some(cargo_workspace_dependencies),
            );
            collect_toml_table_keys(
                target,
                "dev-dependencies",
                "dev",
                "cargo",
                &mut dependencies,
                Some(cargo_workspace_dependencies),
            );
            collect_toml_table_keys(
                target,
                "build-dependencies",
                "build",
                "cargo",
                &mut dependencies,
                Some(cargo_workspace_dependencies),
            );
        }
    }

    dependencies
}

fn pyproject_dependencies(source: &str) -> Vec<ManifestDependency> {
    let Ok(value) = toml::from_str::<toml::Value>(source) else {
        return Vec::new();
    };
    let mut dependencies = Vec::new();

    if let Some(project) = value.get("project") {
        if let Some(values) = project
            .get("dependencies")
            .and_then(|value| value.as_array())
        {
            for value in values {
                if let Some((name, version)) = value
                    .as_str()
                    .and_then(package_name_and_version_from_requirement)
                {
                    dependencies.push(manifest_dependency(name, "runtime", "python", version));
                }
            }
        }
        if let Some(optional) = project
            .get("optional-dependencies")
            .and_then(|value| value.as_table())
        {
            for values in optional.values() {
                if let Some(values) = values.as_array() {
                    for value in values {
                        if let Some((name, version)) = value
                            .as_str()
                            .and_then(package_name_and_version_from_requirement)
                        {
                            dependencies
                                .push(manifest_dependency(name, "optional", "python", version));
                        }
                    }
                }
            }
        }
    }

    if let Some(poetry) = value.get("tool").and_then(|value| value.get("poetry")) {
        collect_toml_table_keys(
            poetry,
            "dependencies",
            "runtime",
            "python",
            &mut dependencies,
            None,
        );
        collect_toml_table_keys(
            poetry,
            "dev-dependencies",
            "dev",
            "python",
            &mut dependencies,
            None,
        );
    }

    dependencies
}

fn package_json_dependencies(source: &str) -> Vec<ManifestDependency> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(source) else {
        return Vec::new();
    };
    let mut dependencies = Vec::new();
    collect_json_object_keys(&value, "dependencies", "runtime", "npm", &mut dependencies);
    collect_json_object_keys(&value, "devDependencies", "dev", "npm", &mut dependencies);
    collect_json_object_keys(&value, "peerDependencies", "peer", "npm", &mut dependencies);
    collect_json_object_keys(
        &value,
        "optionalDependencies",
        "optional",
        "npm",
        &mut dependencies,
    );
    dependencies
}

fn composer_dependencies(source: &str) -> Vec<ManifestDependency> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(source) else {
        return Vec::new();
    };
    let mut dependencies = Vec::new();
    collect_json_object_keys(&value, "require", "runtime", "composer", &mut dependencies);
    collect_json_object_keys(&value, "require-dev", "dev", "composer", &mut dependencies);
    dependencies.retain(|dependency| dependency.name != "php");
    dependencies
}

fn go_mod_dependencies(source: &str) -> Vec<ManifestDependency> {
    let mut dependencies = Vec::new();
    let mut in_require_block = false;
    for raw_line in source.lines() {
        let line = raw_line.split("//").next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line == "require (" {
            in_require_block = true;
            continue;
        }
        if in_require_block && line == ")" {
            in_require_block = false;
            continue;
        }
        let requirement = if in_require_block {
            line
        } else if let Some(rest) = line.strip_prefix("require ") {
            rest.trim()
        } else {
            continue;
        };
        let mut parts = requirement.split_whitespace();
        if let Some(name) = parts.next() {
            let version = parts.next().map(str::to_string);
            dependencies.push(manifest_dependency(
                name.to_string(),
                "runtime",
                "go",
                version,
            ));
        }
    }
    dependencies
}

fn go_module_name(source: &str) -> Option<String> {
    source.lines().find_map(|line| {
        let line = line.split("//").next().unwrap_or("").trim();
        line.strip_prefix("module ")
            .map(str::trim)
            .filter(|module| !module.is_empty())
            .map(str::to_string)
    })
}

fn shebang_interpreter(source: &str) -> Option<(&'static str, &'static str)> {
    let line = source.lines().next()?.trim();
    let command = line.strip_prefix("#!")?.trim();
    if command.is_empty() {
        return None;
    }

    let mut parts = command.split_whitespace();
    let executable = parts.next()?.rsplit('/').next().unwrap_or("");
    let interpreter = if executable == "env" {
        parts
            .find(|part| !part.starts_with('-') && !part.contains('='))
            .unwrap_or("")
    } else {
        executable
    };
    let interpreter = interpreter
        .rsplit('/')
        .next()
        .unwrap_or(interpreter)
        .split_once('.')
        .map_or(interpreter, |(base, _)| base);

    match interpreter {
        "bash" => Some(("bash", "bash")),
        "sh" => Some(("sh", "bash")),
        "zsh" => Some(("zsh", "bash")),
        "ksh" => Some(("ksh", "bash")),
        "python" | "python2" | "python3" => Some(("python", "python")),
        "node" | "nodejs" => Some(("node", "javascript")),
        "php" => Some(("php", "php")),
        _ => None,
    }
}

fn shebang_language(source: &str) -> Option<Language> {
    match shebang_interpreter(source)?.1 {
        "bash" => Some(Language::Bash),
        "python" => Some(Language::Python),
        "javascript" => Some(Language::JavaScript),
        "php" => Some(Language::Php),
        _ => None,
    }
}

fn file_framework_configs(label: &str) -> Vec<FrameworkConfig> {
    let mut configs = Vec::new();
    let file_name = Path::new(label)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(label);
    let lower_label = label.to_ascii_lowercase();
    let lower_name = file_name.to_ascii_lowercase();

    if lower_name == "settings.py" {
        configs.push(framework_config(
            "django",
            format!("django settings:{label}"),
            "settings_module",
            Some(label.to_string()),
            1,
        ));
    }

    for (prefix, framework, kind) in [
        ("next.config.", "nextjs", "config_file"),
        ("vite.config.", "vite", "config_file"),
        ("nuxt.config.", "nuxt", "config_file"),
        ("webpack.config.", "webpack", "config_file"),
        ("svelte.config.", "sveltekit", "config_file"),
    ] {
        if lower_name.starts_with(prefix) {
            configs.push(framework_config(
                framework,
                format!("{framework} config:{label}"),
                kind,
                Some(label.to_string()),
                1,
            ));
        }
    }

    if lower_label.starts_with("config/") && lower_label.ends_with(".php") {
        configs.push(framework_config(
            "laravel",
            format!("laravel config:{label}"),
            "config_file",
            Some(label.to_string()),
            1,
        ));
    }

    if lower_label.starts_with("config/packages/")
        && (lower_label.ends_with(".yaml")
            || lower_label.ends_with(".yml")
            || lower_label.ends_with(".xml")
            || lower_label.ends_with(".php"))
    {
        configs.push(framework_config(
            "symfony",
            format!("symfony config:{label}"),
            "config_file",
            Some(label.to_string()),
            1,
        ));
    }

    configs
}

fn python_framework_configs(source: &str) -> Vec<FrameworkConfig> {
    let mut configs = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();

        if lower.contains(".config.from_pyfile(")
            && let Some(value) = first_quoted_value(trimmed)
        {
            configs.push(framework_config(
                "flask",
                format!("flask config:{value}"),
                "config_file",
                Some(value),
                line_number,
            ));
        }

        if lower.contains(".config.from_object(")
            && let Some(value) = first_quoted_value(trimmed)
        {
            configs.push(framework_config(
                "flask",
                format!("flask config object:{value}"),
                "config_object",
                Some(value),
                line_number,
            ));
        }

        if lower.contains("settingsconfigdict(")
            && lower.contains("env_file")
            && let Some(value) = first_quoted_value(trimmed)
        {
            configs.push(framework_config(
                "pydantic",
                format!("pydantic env file:{value}"),
                "env_file",
                Some(value),
                line_number,
            ));
        }

        if lower.starts_with("class ")
            && lower.contains("basesettings")
            && let Some(class_name) = trimmed
                .strip_prefix("class ")
                .and_then(|rest| rest.split_once('(').map(|(name, _)| name.trim()))
                .filter(|name| !name.is_empty())
        {
            configs.push(framework_config(
                "pydantic",
                format!("pydantic settings:{class_name}"),
                "settings_class",
                Some(class_name.to_string()),
                line_number,
            ));
        }
    }
    configs
}

fn js_framework_configs(source: &str) -> Vec<FrameworkConfig> {
    let mut configs = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();

        if lower.contains("dotenv.config(") {
            let value = first_quoted_value(trimmed).unwrap_or_else(|| ".env".to_string());
            configs.push(framework_config(
                "dotenv",
                format!("dotenv config:{value}"),
                "env_file",
                Some(value),
                line_number,
            ));
        }

        if let Some(setting) = express_setting(trimmed) {
            configs.push(framework_config(
                "express",
                format!("express setting:{setting}"),
                "setting",
                Some(setting),
                line_number,
            ));
        }
    }
    configs
}

fn rust_framework_configs(source: &str) -> Vec<FrameworkConfig> {
    let mut configs = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();

        if lower.contains("dotenv") && lower.contains("dotenv(") {
            configs.push(framework_config(
                "dotenv",
                "dotenv config:.env".to_string(),
                "env_file",
                Some(".env".to_string()),
                line_number,
            ));
        }

        if lower.contains("environment::with_prefix(")
            && let Some(value) = first_quoted_value_after(trimmed, "Environment::with_prefix(")
        {
            configs.push(framework_config(
                "config-rs",
                format!("config-rs env prefix:{value}"),
                "env_prefix",
                Some(value),
                line_number,
            ));
        }
    }
    configs
}

fn go_framework_configs(source: &str) -> Vec<FrameworkConfig> {
    let mut configs = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();

        if lower.contains("viper.setconfigname(")
            && let Some(value) = first_quoted_value_after(trimmed, "SetConfigName(")
        {
            configs.push(framework_config(
                "viper",
                format!("viper config:{value}"),
                "config_name",
                Some(value),
                line_number,
            ));
        }

        if lower.contains("viper.addconfigpath(")
            && let Some(value) = first_quoted_value_after(trimmed, "AddConfigPath(")
        {
            configs.push(framework_config(
                "viper",
                format!("viper config path:{value}"),
                "config_path",
                Some(value),
                line_number,
            ));
        }

        if lower.contains("godotenv.load(") {
            let value =
                first_quoted_value_after(trimmed, "Load(").unwrap_or_else(|| ".env".to_string());
            configs.push(framework_config(
                "godotenv",
                format!("godotenv config:{value}"),
                "env_file",
                Some(value),
                line_number,
            ));
        }
    }
    configs
}

fn php_framework_configs(source: &str) -> Vec<FrameworkConfig> {
    let mut configs = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();

        if lower.contains("config(")
            && let Some(value) = first_quoted_value_after(trimmed, "config(")
        {
            configs.push(framework_config(
                "laravel",
                format!("laravel config key:{value}"),
                "config_key",
                Some(value),
                line_number,
            ));
        }

        if lower.contains("->configure(")
            && let Some(value) = first_quoted_value_after(trimmed, "->configure(")
        {
            configs.push(framework_config(
                "lumen",
                format!("lumen config:{value}"),
                "config_file",
                Some(value),
                line_number,
            ));
        }
    }
    configs
}

fn bash_framework_configs(source: &str) -> Vec<FrameworkConfig> {
    let mut configs = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        let trimmed = line.trim();
        if let Some(value) = sourced_shell_config(trimmed) {
            configs.push(framework_config(
                "shell",
                format!("shell config:{value}"),
                "source_file",
                Some(value),
                line_number,
            ));
        }
    }
    configs
}

fn framework_config(
    framework: &str,
    label: String,
    config_kind: &str,
    value: Option<String>,
    line: u32,
) -> FrameworkConfig {
    FrameworkConfig {
        framework: framework.to_string(),
        label,
        config_kind: config_kind.to_string(),
        value,
        line,
    }
}

fn express_setting(line: &str) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    let setting_index = lower
        .find(".set(")
        .or_else(|| lower.find("app.set("))
        .or_else(|| lower.find("server.set("))?;
    let receiver = line[..setting_index]
        .rsplit(|character: char| {
            !(character.is_ascii_alphanumeric() || matches!(character, '_' | '$'))
        })
        .next()
        .unwrap_or("")
        .trim_start_matches('$');
    if !["app", "server", "router"].iter().any(|allowed| {
        receiver.eq_ignore_ascii_case(allowed)
            || lower[setting_index..].starts_with(&format!("{allowed}.set("))
    }) {
        return None;
    }
    first_quoted_value(line)
}

fn sourced_shell_config(line: &str) -> Option<String> {
    let without_comment = line.split('#').next().unwrap_or("").trim();
    let rest = without_comment
        .strip_prefix("source ")
        .or_else(|| without_comment.strip_prefix(". "))?
        .trim();
    let value = rest
        .split_whitespace()
        .next()
        .map(|value| value.trim_matches(['"', '\'']).to_string())?;
    if value.contains("env") || value.contains("config") || value.ends_with(".conf") {
        Some(value)
    } else {
        None
    }
}

fn python_framework_routes(source: &str) -> Vec<FrameworkRoute> {
    let mut routes = Vec::new();
    let mut pending = Vec::new();

    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        let trimmed = line.trim();
        if trimmed.starts_with('@') {
            if let Some(mut route) = route_from_python_decorator(trimmed, line_number) {
                route.handler = None;
                pending.push(route);
            }
            continue;
        }

        if let Some(function) = trimmed
            .strip_prefix("def ")
            .and_then(|rest| rest.split_once('(').map(|(name, _)| name.trim()))
            .filter(|name| !name.is_empty())
        {
            for mut route in pending.drain(..) {
                route.handler = Some(function.to_string());
                routes.push(route);
            }
        } else if !trimmed.is_empty() && !trimmed.starts_with('#') {
            pending.clear();
        }
    }

    routes
}

fn route_from_python_decorator(line: &str, line_number: u32) -> Option<FrameworkRoute> {
    let lower = line.to_ascii_lowercase();
    if !(lower.contains(".route(")
        || route_methods()
            .iter()
            .any(|method| lower.contains(&format!(".{}(", method.to_ascii_lowercase()))))
    {
        return None;
    }
    let path = first_quoted_value(line)?;
    let method = route_methods()
        .iter()
        .find(|method| lower.contains(&format!(".{}(", method.to_ascii_lowercase())))
        .copied()
        .or_else(|| method_from_python_route_methods(line))
        .unwrap_or("ROUTE")
        .to_string();
    let framework = if method != "ROUTE" || lower.contains("fastapi") || lower.contains("router.") {
        "fastapi"
    } else {
        "flask"
    };

    Some(FrameworkRoute {
        framework: framework.to_string(),
        method,
        path,
        handler: None,
        line: line_number,
    })
}

fn method_from_python_route_methods(line: &str) -> Option<&'static str> {
    let lower = line.to_ascii_uppercase();
    route_methods()
        .iter()
        .find(|method| {
            lower.contains(&format!("\"{method}\"")) || lower.contains(&format!("'{method}'"))
        })
        .copied()
}

fn js_framework_routes(source: &str) -> Vec<FrameworkRoute> {
    source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            route_from_call_line(
                line,
                index as u32 + 1,
                "express",
                &["app", "router", "server", "routes"],
            )
        })
        .collect()
}

fn rust_framework_routes(source: &str) -> Vec<FrameworkRoute> {
    source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let line_number = index as u32 + 1;
            let trimmed = line.trim();
            if !trimmed.contains(".route(") {
                return None;
            }
            let path = first_quoted_value(trimmed)?;
            let method = route_methods()
                .iter()
                .find(|method| trimmed.contains(&format!("{}(", method.to_ascii_lowercase())))
                .copied()
                .unwrap_or("ROUTE")
                .to_string();
            let handler = handler_from_rust_route(trimmed);
            Some(FrameworkRoute {
                framework: "axum".to_string(),
                method,
                path,
                handler,
                line: line_number,
            })
        })
        .collect()
}

fn go_framework_routes(source: &str) -> Vec<FrameworkRoute> {
    source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let line_number = index as u32 + 1;
            let trimmed = line.trim();
            if trimmed.contains("HandleFunc(") {
                let path = first_quoted_value(trimmed)?;
                let handler = handler_after_first_comma(trimmed);
                return Some(FrameworkRoute {
                    framework: "net/http".to_string(),
                    method: "ROUTE".to_string(),
                    path,
                    handler,
                    line: line_number,
                });
            }
            route_from_call_line(
                trimmed,
                line_number,
                "go-router",
                &["r", "router", "engine", "api", "group", "v1", "v2"],
            )
        })
        .collect()
}

fn php_framework_routes(source: &str) -> Vec<FrameworkRoute> {
    let mut routes = Vec::new();
    let mut pending = Vec::new();

    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        let trimmed = line.trim();
        if trimmed.starts_with("#[") && trimmed.contains("Route(") {
            if let Some(path) = first_quoted_value(trimmed) {
                pending.push(FrameworkRoute {
                    framework: "php-attribute".to_string(),
                    method: method_from_php_route(trimmed)
                        .unwrap_or("ROUTE")
                        .to_string(),
                    path,
                    handler: None,
                    line: line_number,
                });
            }
            continue;
        }
        if let Some(function) = trimmed
            .strip_prefix("function ")
            .and_then(|rest| rest.split_once('(').map(|(name, _)| name.trim()))
            .filter(|name| !name.is_empty())
        {
            for mut route in pending.drain(..) {
                route.handler = Some(function.to_string());
                routes.push(route);
            }
        } else if !trimmed.is_empty() && !trimmed.starts_with("//") {
            pending.clear();
        }
    }

    routes
}

fn route_from_call_line(
    line: &str,
    line_number: u32,
    framework: &str,
    allowed_receivers: &[&str],
) -> Option<FrameworkRoute> {
    let lower = line.to_ascii_lowercase();
    let method = route_methods()
        .iter()
        .find(|method| {
            let method = method.to_ascii_lowercase();
            route_receiver_matches(&lower, &method, allowed_receivers)
        })
        .copied()?;
    let path = first_quoted_value(line)?;
    let handler = handler_after_first_comma(line);
    Some(FrameworkRoute {
        framework: framework.to_string(),
        method: method.to_string(),
        path,
        handler,
        line: line_number,
    })
}

fn route_receiver_matches(line: &str, method: &str, allowed_receivers: &[&str]) -> bool {
    let Some(method_index) = line
        .find(&format!(".{method}("))
        .or_else(|| line.find(&format!("->{method}(")))
    else {
        return false;
    };
    let receiver = line[..method_index]
        .rsplit(|character: char| {
            !(character.is_ascii_alphanumeric() || matches!(character, '_' | '$'))
        })
        .next()
        .unwrap_or("")
        .trim_start_matches('$');
    allowed_receivers
        .iter()
        .any(|allowed| receiver.eq_ignore_ascii_case(allowed))
}

fn handler_from_rust_route(line: &str) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    for method in route_methods() {
        let needle = format!("{}(", method.to_ascii_lowercase());
        if let Some(start) = lower.find(&needle) {
            let rest = &line[start + needle.len()..];
            let handler = rest
                .split([',', ')'])
                .next()
                .map(|value| value.trim().trim_start_matches("move ").trim())
                .filter(|value| !value.is_empty())?;
            return Some(handler.to_string());
        }
    }
    None
}

fn handler_after_first_comma(line: &str) -> Option<String> {
    let handler = line
        .split_once(',')
        .map(|(_, rest)| rest.trim())
        .and_then(|rest| rest.split([',', ')']).next())
        .map(|value| {
            value
                .trim()
                .trim_start_matches('&')
                .trim_start_matches("::")
                .trim_matches(['"', '\'', '`'])
                .to_string()
        })
        .filter(|value| !value.is_empty())?;
    if handler.starts_with('|') || handler.starts_with("function") || handler.starts_with("async") {
        None
    } else {
        Some(handler)
    }
}

fn method_from_php_route(line: &str) -> Option<&'static str> {
    let upper = line.to_ascii_uppercase();
    route_methods()
        .iter()
        .find(|method| {
            upper.contains(&format!("\"{method}\"")) || upper.contains(&format!("'{method}'"))
        })
        .copied()
}

fn first_quoted_value(value: &str) -> Option<String> {
    let quote_index = value.find(['"', '\'', '`'])?;
    let quote = value[quote_index..].chars().next()?;
    let rest = &value[quote_index + quote.len_utf8()..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

fn first_quoted_value_after(value: &str, needle: &str) -> Option<String> {
    let lower_value = value.to_ascii_lowercase();
    let lower_needle = needle.to_ascii_lowercase();
    let start = lower_value.find(&lower_needle)?;
    first_quoted_value(&value[start + needle.len()..])
}

fn route_methods() -> &'static [&'static str] {
    &["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "HEAD"]
}

fn requirements_dependencies(source: &str) -> Vec<ManifestDependency> {
    source
        .lines()
        .filter_map(|line| {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() || line.starts_with('-') {
                return None;
            }
            package_name_and_version_from_requirement(line)
                .map(|(name, version)| manifest_dependency(name, "runtime", "python", version))
        })
        .collect()
}

fn collect_toml_table_keys(
    value: &toml::Value,
    table_name: &str,
    dependency_kind: &str,
    ecosystem: &str,
    dependencies: &mut Vec<ManifestDependency>,
    cargo_workspace_dependencies: Option<&BTreeMap<String, Option<String>>>,
) {
    let Some(table) = value.get(table_name).and_then(|value| value.as_table()) else {
        return;
    };
    for (name, value) in table {
        let package_name = value
            .as_table()
            .and_then(|table| table.get("package"))
            .and_then(|value| value.as_str())
            .unwrap_or(name)
            .to_string();
        let version = dependency_version_from_toml_value(
            name,
            value,
            ecosystem,
            cargo_workspace_dependencies,
        );
        dependencies.push(manifest_dependency(
            package_name,
            dependency_kind,
            ecosystem,
            version,
        ));
    }
}

fn collect_toml_entrypoint_keys(
    value: &toml::Value,
    table_name: &str,
    entrypoint_kind: &str,
    ecosystem: &str,
    entrypoints: &mut Vec<ManifestEntrypoint>,
) {
    let Some(table) = value.get(table_name).and_then(|value| value.as_table()) else {
        return;
    };
    for (name, target) in table {
        entrypoints.push(manifest_entrypoint(
            format!("{ecosystem} {entrypoint_kind}:{name}"),
            entrypoint_kind,
            ecosystem,
            target.as_str().map(str::to_string),
        ));
    }
}

fn collect_json_object_keys(
    value: &serde_json::Value,
    object_name: &str,
    dependency_kind: &str,
    ecosystem: &str,
    dependencies: &mut Vec<ManifestDependency>,
) {
    let Some(object) = value.get(object_name).and_then(|value| value.as_object()) else {
        return;
    };
    for (name, value) in object {
        let version = value.as_str().map(str::to_string);
        dependencies.push(manifest_dependency(
            name.clone(),
            dependency_kind,
            ecosystem,
            version,
        ));
    }
}

fn cargo_workspace_dependencies(root: &Path) -> BTreeMap<String, Option<String>> {
    let Ok(source) = fs::read_to_string(root.join("Cargo.toml")) else {
        return BTreeMap::new();
    };
    let Ok(value) = toml::from_str::<toml::Value>(&source) else {
        return BTreeMap::new();
    };
    let Some(table) = value
        .get("workspace")
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(|dependencies| dependencies.as_table())
    else {
        return BTreeMap::new();
    };

    table
        .iter()
        .map(|(name, value)| (name.clone(), direct_toml_dependency_version(value)))
        .collect()
}

fn dependency_version_from_toml_value(
    name: &str,
    value: &toml::Value,
    ecosystem: &str,
    cargo_workspace_dependencies: Option<&BTreeMap<String, Option<String>>>,
) -> Option<String> {
    if ecosystem == "cargo"
        && value
            .as_table()
            .and_then(|table| table.get("workspace"))
            .and_then(|value| value.as_bool())
            .is_some_and(|enabled| enabled)
    {
        return cargo_workspace_dependencies
            .and_then(|dependencies| dependencies.get(name))
            .cloned()
            .flatten();
    }
    direct_toml_dependency_version(value)
}

fn direct_toml_dependency_version(value: &toml::Value) -> Option<String> {
    if let Some(version) = value.as_str() {
        return Some(version.to_string());
    }
    let table = value.as_table()?;
    table
        .get("version")
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn package_name_and_version_from_requirement(
    requirement: &str,
) -> Option<(String, Option<String>)> {
    let trimmed = requirement.trim();
    let end = trimmed
        .find(|character: char| {
            matches!(
                character,
                '<' | '>' | '=' | '!' | '~' | '[' | ';' | ',' | ' '
            )
        })
        .unwrap_or(trimmed.len());
    let name = trimmed[..end].trim();
    if name.is_empty() {
        None
    } else {
        let version = trimmed[end..].trim();
        Some((
            name.to_string(),
            (!version.is_empty()).then(|| version.to_string()),
        ))
    }
}

fn package_id(ecosystem: &str, package_name: &str) -> String {
    format!("{ecosystem}:{package_name}")
}

fn canonical_package_name(ecosystem: &str, name: &str) -> String {
    let trimmed = name.trim();
    match ecosystem {
        "python" => {
            let mut normalized = String::new();
            let mut previous_separator = false;
            for character in trimmed.chars() {
                if matches!(character, '-' | '_' | '.') {
                    if !previous_separator {
                        normalized.push('-');
                    }
                    previous_separator = true;
                } else {
                    normalized.extend(character.to_lowercase());
                    previous_separator = false;
                }
            }
            normalized
        }
        "cargo" | "npm" | "composer" => trimmed.to_ascii_lowercase(),
        "go" => trimmed.to_string(),
        _ => trimmed.to_string(),
    }
}

fn manifest_dependency(
    name: impl Into<String>,
    dependency_kind: impl Into<String>,
    ecosystem: impl Into<String>,
    version: Option<String>,
) -> ManifestDependency {
    ManifestDependency {
        name: name.into(),
        kind: dependency_kind.into(),
        ecosystem: ecosystem.into(),
        version,
    }
}

fn manifest_entrypoint(
    label: impl Into<String>,
    entrypoint_kind: impl Into<String>,
    ecosystem: impl Into<String>,
    target: Option<String>,
) -> ManifestEntrypoint {
    ManifestEntrypoint {
        label: label.into(),
        kind: entrypoint_kind.into(),
        ecosystem: ecosystem.into(),
        target,
    }
}

fn resolve_pending_calls(context: &mut IndexContext) {
    let pending_calls = std::mem::take(&mut context.pending_calls);

    for call in pending_calls {
        let targets = resolve_function_targets(&context.function_symbols, &call.label);
        if targets.is_empty() {
            let mut metadata = BTreeMap::new();
            metadata.insert("language".to_string(), call.language);
            metadata.insert("parser".to_string(), "tree-sitter".to_string());
            metadata.insert("item_kind".to_string(), "call".to_string());
            metadata.insert("resolution".to_string(), "unresolved".to_string());
            let call_id = context.graph.add_node_with_metadata(
                NodeKind::ExternalDependency,
                call.label,
                Some(call.span),
                metadata,
            );
            add_edge_once(
                &mut context.graph,
                call.caller,
                call_id,
                EdgeKind::Calls,
                Confidence::Heuristic,
            );
            continue;
        }

        for target in targets {
            add_edge_once(
                &mut context.graph,
                call.caller,
                target,
                EdgeKind::Calls,
                Confidence::Heuristic,
            );
        }
    }
}

fn resolve_pending_entrypoint_targets(context: &mut IndexContext) {
    let pending_targets = std::mem::take(&mut context.pending_entrypoint_targets);

    for pending in pending_targets {
        for candidate in entrypoint_target_candidates(&pending) {
            if let Some(file_id) = context.file_nodes.get(&candidate.path).copied() {
                add_entrypoint_reference(
                    &mut context.graph,
                    pending.entrypoint,
                    file_id,
                    "entrypoint_file",
                    candidate.resolution,
                    candidate.file_confidence,
                    None,
                );
            }

            let Some(symbol) = candidate.symbol.as_deref() else {
                continue;
            };
            let function_targets =
                function_targets_in_file(&context.graph, &candidate.path, symbol);
            for target in function_targets {
                add_entrypoint_reference(
                    &mut context.graph,
                    pending.entrypoint,
                    target,
                    "entrypoint_function",
                    candidate.resolution,
                    candidate.function_confidence,
                    Some(symbol),
                );
            }
        }
    }
}

fn entrypoint_target_candidates(
    pending: &PendingEntrypointTarget,
) -> Vec<EntrypointTargetCandidate> {
    match pending.ecosystem.as_str() {
        "cargo" => manifest_path_candidate(
            pending,
            &pending.target,
            Some("main".to_string()),
            Confidence::Exact,
            Confidence::Syntactic,
            "manifest_path",
        )
        .into_iter()
        .collect(),
        "python" => python_entrypoint_candidates(pending),
        "go" => manifest_path_candidate(
            pending,
            &pending.target,
            Some("main".to_string()),
            Confidence::Exact,
            Confidence::Syntactic,
            "manifest_path",
        )
        .into_iter()
        .collect(),
        "cmake" => manifest_path_candidate(
            pending,
            &pending.target,
            Some("main".to_string()),
            Confidence::Exact,
            Confidence::Syntactic,
            "manifest_path",
        )
        .into_iter()
        .collect(),
        "npm" => command_path_candidate(pending)
            .map(|path| EntrypointTargetCandidate {
                path,
                symbol: None,
                file_confidence: Confidence::Heuristic,
                function_confidence: Confidence::Heuristic,
                resolution: "command_path",
            })
            .into_iter()
            .collect(),
        "composer" if pending.entrypoint_kind == "binary" => manifest_path_candidate(
            pending,
            &pending.target,
            None,
            Confidence::Exact,
            Confidence::Exact,
            "manifest_path",
        )
        .into_iter()
        .collect(),
        "composer" => command_path_candidate(pending)
            .map(|path| EntrypointTargetCandidate {
                path,
                symbol: None,
                file_confidence: Confidence::Heuristic,
                function_confidence: Confidence::Heuristic,
                resolution: "command_path",
            })
            .into_iter()
            .collect(),
        _ => Vec::new(),
    }
}

fn manifest_path_candidate(
    pending: &PendingEntrypointTarget,
    target: &str,
    symbol: Option<String>,
    file_confidence: Confidence,
    function_confidence: Confidence,
    resolution: &'static str,
) -> Option<EntrypointTargetCandidate> {
    normalize_manifest_relative_path(&pending.manifest_label, target).map(|path| {
        EntrypointTargetCandidate {
            path,
            symbol,
            file_confidence,
            function_confidence,
            resolution,
        }
    })
}

fn python_entrypoint_candidates(
    pending: &PendingEntrypointTarget,
) -> Vec<EntrypointTargetCandidate> {
    let Some((module, symbol)) = pending.target.split_once(':') else {
        return Vec::new();
    };
    let module = module.trim();
    let symbol = simple_symbol_name(symbol.trim());
    if module.is_empty() || symbol.is_empty() {
        return Vec::new();
    }

    let module_path = module.replace('.', "/");
    [
        format!("{module_path}.py"),
        format!("{module_path}/__init__.py"),
    ]
    .into_iter()
    .filter_map(|path| {
        manifest_path_candidate(
            pending,
            &path,
            Some(symbol.clone()),
            Confidence::Heuristic,
            Confidence::Heuristic,
            "python_module",
        )
    })
    .collect()
}

fn command_path_candidate(pending: &PendingEntrypointTarget) -> Option<String> {
    command_source_path_candidate(&pending.target)
        .and_then(|path| normalize_manifest_relative_path(&pending.manifest_label, &path))
}

fn command_source_path_candidate(command: &str) -> Option<String> {
    split_command_tokens(command)
        .into_iter()
        .find(|token| is_command_path_candidate(token))
}

fn cmake_command_bodies(source: &str, command_name: &str) -> Vec<String> {
    let source = strip_cmake_comments(source);
    let lowered = source.to_ascii_lowercase();
    let needle = command_name.to_ascii_lowercase();
    let mut bodies = Vec::new();
    let mut search_from = 0;

    while let Some(offset) = lowered[search_from..].find(&needle) {
        let start = search_from + offset;
        let before = source[..start].chars().next_back();
        let after_name = start + needle.len();
        let after = source[after_name..].chars().next();
        if before.is_some_and(is_cmake_ident_char) || after.is_some_and(is_cmake_ident_char) {
            search_from = after_name;
            continue;
        }

        let Some(open_offset) = source[after_name..].find('(') else {
            break;
        };
        let open = after_name + open_offset;
        if !source[after_name..open]
            .chars()
            .all(|character| character.is_whitespace())
        {
            search_from = after_name;
            continue;
        }

        let Some((body, close)) = cmake_parenthesized_body(&source, open) else {
            break;
        };
        bodies.push(body);
        search_from = close + 1;
    }

    bodies
}

fn strip_cmake_comments(source: &str) -> String {
    let mut stripped = String::with_capacity(source.len());
    let mut quote = None;
    let mut chars = source.chars().peekable();
    while let Some(character) = chars.next() {
        match quote {
            Some(current_quote) if character == current_quote => {
                quote = None;
                stripped.push(character);
            }
            Some(_) => stripped.push(character),
            None if character == '"' || character == '\'' => {
                quote = Some(character);
                stripped.push(character);
            }
            None if character == '#' => {
                for next in chars.by_ref() {
                    if next == '\n' {
                        stripped.push('\n');
                        break;
                    }
                }
            }
            None => stripped.push(character),
        }
    }
    stripped
}

fn cmake_parenthesized_body(source: &str, open: usize) -> Option<(String, usize)> {
    let mut depth = 0usize;
    let mut quote = None;
    let body_start = open + '('.len_utf8();

    for (index, character) in source[open..].char_indices() {
        let absolute = open + index;
        match quote {
            Some(current_quote) if character == current_quote => quote = None,
            Some(_) => {}
            None if character == '"' || character == '\'' => quote = Some(character),
            None if character == '(' => depth += 1,
            None if character == ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some((source[body_start..absolute].to_string(), absolute));
                }
            }
            None => {}
        }
    }

    None
}

fn cmake_command_args(body: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote = None;

    for character in body.chars() {
        match quote {
            Some(current_quote) if character == current_quote => quote = None,
            Some(_) => current.push(character),
            None if character == '"' || character == '\'' => quote = Some(character),
            None if character.is_whitespace() || character == ';' => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            None => current.push(character),
        }
    }
    if !current.is_empty() {
        args.push(current);
    }

    args
}

fn is_cmake_source_argument(value: &str) -> bool {
    let value = value.trim();
    !value.starts_with('$') && !value.starts_with('<') && has_known_source_extension(value)
}

fn is_cmake_ident_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

fn split_command_tokens(command: &str) -> Vec<String> {
    command
        .split(|character: char| {
            character.is_whitespace() || matches!(character, '"' | '\'' | '`' | ';' | '&' | '|')
        })
        .filter_map(clean_command_token)
        .collect()
}

fn clean_command_token(token: &str) -> Option<String> {
    let token = token
        .trim()
        .trim_matches(|character: char| matches!(character, '(' | ')' | '[' | ']' | '{' | '}'))
        .trim_matches(|character: char| matches!(character, ',' | ':'));
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

fn is_command_path_candidate(token: &str) -> bool {
    if token.starts_with('-')
        || token.starts_with('$')
        || token.contains("://")
        || token.contains('=')
        || token.contains('*')
    {
        return false;
    }
    token.contains('/') || has_known_source_extension(token)
}

fn has_known_source_extension(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension,
                "rs" | "py"
                    | "pyw"
                    | "js"
                    | "mjs"
                    | "cjs"
                    | "ts"
                    | "mts"
                    | "cts"
                    | "tsx"
                    | "go"
                    | "c"
                    | "h"
                    | "cc"
                    | "cpp"
                    | "cxx"
                    | "hpp"
                    | "hh"
                    | "hxx"
                    | "php"
                    | "phtml"
                    | "sh"
                    | "bash"
                    | "zsh"
                    | "ksh"
            )
        })
}

fn normalize_manifest_relative_path(manifest_label: &str, target: &str) -> Option<String> {
    let target = target.trim().trim_matches('"').trim_matches('\'');
    if target.is_empty()
        || target.starts_with('-')
        || target.starts_with('$')
        || target.contains("://")
    {
        return None;
    }
    let target_path = Path::new(target);
    if target_path.is_absolute() {
        return None;
    }

    let base = Path::new(manifest_label)
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    let joined = base.map_or_else(|| PathBuf::from(target), |base| base.join(target));
    normalize_relative_path(&joined)
}

fn normalize_relative_path(path: &Path) -> Option<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                parts.pop()?;
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => return None,
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

fn function_targets_in_file(graph: &CodeGraph, path: &str, symbol: &str) -> Vec<NodeId> {
    graph
        .nodes
        .iter()
        .filter(|node| {
            node.kind == NodeKind::Function
                && node.span.as_ref().is_some_and(|span| span.path == path)
                && function_symbol_matches(&node.label, symbol)
        })
        .map(|node| node.id)
        .collect()
}

fn function_symbol_matches(label: &str, symbol: &str) -> bool {
    symbol_keys(label)
        .into_iter()
        .any(|key| key == symbol || simple_symbol_name(&key) == symbol)
}

fn add_entrypoint_reference(
    graph: &mut CodeGraph,
    source: NodeId,
    target: NodeId,
    relation: &str,
    resolution: &str,
    confidence: Confidence,
    target_symbol: Option<&str>,
) {
    let mut metadata = BTreeMap::new();
    metadata.insert("relation".to_string(), relation.to_string());
    let fact_source = if resolution.starts_with("shebang") {
        "shebang"
    } else if resolution.starts_with("framework") {
        "framework"
    } else {
        "manifest"
    };
    metadata.insert("source".to_string(), fact_source.to_string());
    metadata.insert("resolution".to_string(), resolution.to_string());
    if let Some(target_symbol) = target_symbol {
        metadata.insert("target_symbol".to_string(), target_symbol.to_string());
    }
    add_edge_once_with_metadata(
        graph,
        source,
        target,
        EdgeKind::References,
        confidence,
        metadata,
    );
}

fn register_function_symbol(symbols: &mut BTreeMap<String, Vec<NodeId>>, label: &str, id: NodeId) {
    for key in symbol_keys(label) {
        let values = symbols.entry(key).or_default();
        if !values.contains(&id) {
            values.push(id);
        }
    }
}

fn register_local_function(symbols: &mut BTreeMap<String, NodeId>, label: &str, id: NodeId) {
    for key in symbol_keys(label) {
        symbols.entry(key).or_insert(id);
    }
}

fn resolve_local_function(symbols: &BTreeMap<String, NodeId>, label: &str) -> Option<NodeId> {
    symbol_keys(label)
        .into_iter()
        .find_map(|key| symbols.get(&key).copied())
}

fn resolve_function_targets(symbols: &BTreeMap<String, Vec<NodeId>>, label: &str) -> Vec<NodeId> {
    let mut targets = Vec::new();
    for key in symbol_keys(label) {
        if let Some(ids) = symbols.get(&key) {
            for id in ids {
                if !targets.contains(id) {
                    targets.push(*id);
                }
            }
        }
    }
    targets
}

fn symbol_keys(label: &str) -> Vec<String> {
    let compact = label.trim().trim_end_matches('!').to_string();
    let simple = simple_symbol_name(&compact);
    if compact == simple {
        vec![compact]
    } else {
        vec![compact, simple]
    }
}

fn simple_symbol_name(label: &str) -> String {
    label
        .rsplit([':', '.', '\\', '>'])
        .find(|part| !part.is_empty() && *part != "-")
        .unwrap_or(label)
        .trim()
        .to_string()
}

fn add_edge_once(
    graph: &mut CodeGraph,
    source: NodeId,
    target: NodeId,
    kind: EdgeKind,
    confidence: Confidence,
) {
    add_edge_once_with_metadata(graph, source, target, kind, confidence, BTreeMap::new());
}

fn add_edge_once_with_metadata(
    graph: &mut CodeGraph,
    source: NodeId,
    target: NodeId,
    kind: EdgeKind,
    confidence: Confidence,
    metadata: BTreeMap<String, String>,
) {
    if graph
        .edges
        .iter()
        .any(|edge| edge.source == source && edge.target == target && edge.kind == kind)
    {
        return;
    }
    graph.add_edge_with_metadata(source, target, kind, confidence, metadata);
}

fn add_file_metadata(
    graph: &mut CodeGraph,
    file_id: codegraph_core::NodeId,
    key: &str,
    value: impl Into<String>,
) {
    if let Some(node) = graph.nodes.iter_mut().find(|node| node.id == file_id) {
        node.metadata.insert(key.to_string(), value.into());
    }
}

fn parsed_item_kind_name(kind: ParsedItemKind) -> &'static str {
    match kind {
        ParsedItemKind::Function => "function",
        ParsedItemKind::Type => "type",
        ParsedItemKind::Module => "module",
        ParsedItemKind::Import => "import",
        ParsedItemKind::Entrypoint => "entrypoint",
        ParsedItemKind::Call => "call",
        ParsedItemKind::EnvironmentRead => "environment_read",
        ParsedItemKind::ConfigRead => "config_read",
        ParsedItemKind::Error => "error",
    }
}

fn is_symbol_item(kind: ParsedItemKind) -> bool {
    matches!(
        kind,
        ParsedItemKind::Function
            | ParsedItemKind::Entrypoint
            | ParsedItemKind::Type
            | ParsedItemKind::Module
            | ParsedItemKind::Import
    )
}

fn is_effect_item(kind: ParsedItemKind) -> bool {
    matches!(
        kind,
        ParsedItemKind::EnvironmentRead | ParsedItemKind::ConfigRead | ParsedItemKind::Error
    )
}

fn should_enter(entry: &DirEntry, options: &IndexOptions) -> bool {
    if !options.include_hidden && is_hidden(entry) {
        return false;
    }

    if !options.include_ignored && is_ignored_name(entry, &options.ignored_names) {
        return false;
    }

    true
}

fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .is_some_and(|name| name.starts_with('.') && name != ".")
}

fn is_ignored_name(entry: &DirEntry, ignored_names: &BTreeSet<String>) -> bool {
    entry
        .file_name()
        .to_str()
        .is_some_and(|name| ignored_names.contains(name))
}

fn default_ignored_names() -> BTreeSet<String> {
    [
        ".git",
        ".codegraph",
        ".hg",
        ".svn",
        "target",
        "node_modules",
        "dist",
        "build",
        ".next",
        ".turbo",
        ".venv",
        "__pycache__",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

fn is_probably_source_file(path: &Path, max_file_size: u64) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    metadata.len() <= max_file_size
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn scan_project_skips_default_ignored_directories() {
        let root = temp_project_root();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("target")).unwrap();
        fs::create_dir_all(root.join(".codegraph")).unwrap();
        fs::write(root.join("src").join("main.rs"), "fn main() {}\n").unwrap();
        fs::write(root.join("target").join("debug.log"), "noise\n").unwrap();
        fs::write(root.join(".codegraph").join("graph.json"), "{}\n").unwrap();

        let graph = scan_project(&root, &IndexOptions::default()).unwrap();
        let labels: Vec<_> = graph.nodes.iter().map(|node| node.label.as_str()).collect();

        assert!(labels.contains(&"src/main.rs"));
        assert!(!labels.contains(&"target"));
        assert!(!labels.contains(&"target/debug.log"));
        assert!(!labels.contains(&".codegraph"));
        assert!(!labels.contains(&".codegraph/graph.json"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scan_project_adds_tree_sitter_symbols() {
        let root = temp_project_root();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("src").join("main.rs"),
            "use std::fs;\nstruct App;\nfn main() {}\n",
        )
        .unwrap();

        let graph = scan_project(&root, &IndexOptions::default()).unwrap();
        let labels: Vec<_> = graph.nodes.iter().map(|node| node.label.as_str()).collect();

        assert!(labels.contains(&"src/main.rs"));
        assert!(labels.contains(&"main"));
        assert!(labels.contains(&"App"));
        assert!(labels.contains(&"use std::fs;"));
        assert!(
            graph
                .edges
                .iter()
                .any(|edge| edge.kind == EdgeKind::Entrypoint)
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scan_project_adds_approximate_call_edges() {
        let root = temp_project_root();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("src").join("main.rs"),
            "fn main() { helper(); }\nfn helper() {}\n",
        )
        .unwrap();

        let graph = scan_project(&root, &IndexOptions::default()).unwrap();
        let main_id = graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Function && node.label == "main")
            .map(|node| node.id)
            .unwrap();
        let helper_id = graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Function && node.label == "helper")
            .map(|node| node.id)
            .unwrap();

        assert!(graph.edges.iter().any(|edge| {
            edge.source == main_id
                && edge.target == helper_id
                && edge.kind == EdgeKind::Calls
                && edge.confidence == Confidence::Heuristic
        }));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scan_project_adds_environment_config_and_error_edges() {
        let root = temp_project_root();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("src").join("main.rs"),
            r#"fn main() {
                let _ = std::env::var("DATABASE_URL");
                let _ = std::fs::read_to_string("config/app.toml");
                panic!("broken");
            }
            "#,
        )
        .unwrap();

        let graph = scan_project(&root, &IndexOptions::default()).unwrap();

        assert!(
            graph
                .nodes
                .iter()
                .any(|node| { node.kind == NodeKind::Environment && node.label == "DATABASE_URL" })
        );
        assert!(
            graph
                .nodes
                .iter()
                .any(|node| node.kind == NodeKind::Config && node.label == "config/app.toml")
        );
        assert!(
            graph
                .edges
                .iter()
                .any(|edge| edge.kind == EdgeKind::ReadsEnvironment)
        );
        assert!(
            graph
                .edges
                .iter()
                .any(|edge| edge.kind == EdgeKind::ReadsConfig)
        );
        assert!(
            graph
                .edges
                .iter()
                .any(|edge| edge.kind == EdgeKind::MayError)
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scan_project_adds_manifest_dependency_edges() {
        let root = temp_project_root();
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            r#"[package]
name = "demo"
version = "0.1.0"

[workspace]
members = ["crates/app"]

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
local-util = { path = "crates/local-util" }

[dependencies]
serde = { version = "1", features = ["derive"] }

[dev-dependencies]
anyhow = "1"
"#,
        )
        .unwrap();
        fs::create_dir_all(root.join("crates").join("app")).unwrap();
        fs::write(
            root.join("crates").join("app").join("Cargo.toml"),
            r#"[package]
name = "app"
version = "0.1.0"

[dependencies]
serde = { workspace = true }
local-util = { workspace = true }
"#,
        )
        .unwrap();
        fs::write(
            root.join("package.json"),
            r#"{
  "dependencies": { "react": "^19.0.0" },
  "devDependencies": { "vite": "^7.0.0" }
}"#,
        )
        .unwrap();
        fs::write(
            root.join("go.mod"),
            "module example.com/demo\n\nrequire github.com/gin-gonic/gin v1.10.0\n",
        )
        .unwrap();
        fs::write(root.join("requirements.txt"), "fastapi==0.115.0\n").unwrap();
        fs::write(
            root.join("pyproject.toml"),
            r#"[project]
dependencies = ["pydantic>=2"]
"#,
        )
        .unwrap();
        fs::write(
            root.join("composer.json"),
            r#"{
  "require": {
    "php": ">=8.2",
    "monolog/monolog": "^3.0"
  }
}"#,
        )
        .unwrap();

        let graph = scan_project(&root, &IndexOptions::default()).unwrap();
        let dependency_labels: BTreeSet<_> = graph
            .nodes
            .iter()
            .filter(|node| {
                node.metadata
                    .get("item_kind")
                    .is_some_and(|kind| kind == "dependency")
            })
            .map(|node| node.label.as_str())
            .collect();

        for expected in [
            "serde",
            "local-util",
            "anyhow",
            "react",
            "vite",
            "github.com/gin-gonic/gin",
            "fastapi",
            "pydantic",
            "monolog/monolog",
        ] {
            assert!(dependency_labels.contains(expected), "missing {expected}");
        }
        assert!(!dependency_labels.contains("php"));
        assert!(graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::DependsOn && edge.confidence == Confidence::Exact
        }));
        let serde_nodes: Vec<_> = graph
            .nodes
            .iter()
            .filter(|node| {
                node.metadata
                    .get("package_id")
                    .is_some_and(|value| value == "cargo:serde")
            })
            .collect();
        assert_eq!(serde_nodes.len(), 1);
        let serde_incoming_edges: Vec<_> = graph
            .edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::DependsOn && edge.target == serde_nodes[0].id)
            .collect();
        assert_eq!(serde_incoming_edges.len(), 2);
        assert!(serde_incoming_edges.iter().all(|edge| {
            edge.metadata
                .get("dependency_kind")
                .is_some_and(|value| value == "runtime")
        }));
        assert!(serde_incoming_edges.iter().all(|edge| {
            edge.metadata
                .get("dependency_version")
                .is_some_and(|value| value == "1")
        }));
        let local_util = graph
            .nodes
            .iter()
            .find(|node| {
                node.metadata
                    .get("package_id")
                    .is_some_and(|value| value == "cargo:local-util")
            })
            .expect("missing local-util dependency");
        let local_util_edge = graph
            .edges
            .iter()
            .find(|edge| edge.kind == EdgeKind::DependsOn && edge.target == local_util.id)
            .expect("missing local-util dependency edge");
        assert!(!local_util_edge.metadata.contains_key("dependency_version"));
        assert!(graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::DependsOn
                && edge
                    .metadata
                    .get("dependency_version")
                    .is_some_and(|value| value == "^19.0.0")
        }));
        assert!(graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::DependsOn
                && edge
                    .metadata
                    .get("dependency_version")
                    .is_some_and(|value| value == "v1.10.0")
        }));
        assert!(graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::DependsOn
                && edge
                    .metadata
                    .get("dependency_version")
                    .is_some_and(|value| value == ">=2")
        }));
        assert!(graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::DependsOn
                && edge
                    .metadata
                    .get("dependency_version")
                    .is_some_and(|value| value == "^3.0")
        }));
        assert!(graph.nodes.iter().any(|node| {
            node.metadata
                .get("ecosystem")
                .is_some_and(|value| value == "cargo")
        }));
        assert!(graph.nodes.iter().any(|node| {
            node.metadata
                .get("ecosystem")
                .is_some_and(|value| value == "npm")
        }));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scan_project_adds_manifest_entrypoint_edges() {
        let root = temp_project_root();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("src").join("bin")).unwrap();
        fs::create_dir_all(root.join("codegraph")).unwrap();
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::create_dir_all(root.join("cmd").join("server")).unwrap();
        fs::write(
            root.join("src").join("main.rs"),
            "fn main() { helper(); }\nfn helper() {}\n",
        )
        .unwrap();
        fs::write(
            root.join("src").join("bin").join("worker.rs"),
            "fn main() {}\n",
        )
        .unwrap();
        fs::write(root.join("src").join("index.js"), "console.log('start');\n").unwrap();
        fs::write(
            root.join("src").join("main.c"),
            "int main(void) { return 0; }\n",
        )
        .unwrap();
        fs::write(root.join("main.go"), "package main\nfunc main() {}\n").unwrap();
        fs::write(
            root.join("cmd").join("server").join("main.go"),
            "package main\nfunc main() {}\n",
        )
        .unwrap();
        fs::write(
            root.join("codegraph").join("cli.py"),
            "def main():\n    pass\n",
        )
        .unwrap();
        fs::write(
            root.join("bin").join("codegraph"),
            "#!/usr/bin/env php\n<?php\n",
        )
        .unwrap();
        fs::write(
            root.join("Cargo.toml"),
            r#"[package]
name = "demo"
version = "0.1.0"

[[bin]]
name = "worker"
path = "src/bin/worker.rs"
"#,
        )
        .unwrap();
        fs::write(
            root.join("package.json"),
            r#"{
  "scripts": {
    "start": "node src/index.js",
    "test": "vitest"
  }
}"#,
        )
        .unwrap();
        fs::write(
            root.join("pyproject.toml"),
            r#"[project.scripts]
cg = "codegraph.cli:main"
"#,
        )
        .unwrap();
        fs::write(
            root.join("composer.json"),
            r#"{
  "bin": ["bin/codegraph"],
  "scripts": {
    "analyse": "phpstan analyse"
  }
}"#,
        )
        .unwrap();
        fs::write(
            root.join("CMakeLists.txt"),
            r#"# comment add_executable(nope src/nope.c)
cmake_minimum_required(VERSION 3.20)
project(demo_c)
add_executable(demo_c
  src/main.c
  src/extra.c
)
add_executable(alias_target ALIAS demo_c)
add_executable(imported_tool IMPORTED)
"#,
        )
        .unwrap();
        fs::write(root.join("go.mod"), "module example.com/demo\n\ngo 1.23\n").unwrap();

        let graph = scan_project(&root, &IndexOptions::default()).unwrap();
        let entrypoints: BTreeSet<_> = graph
            .nodes
            .iter()
            .filter(|node| node.kind == NodeKind::Entrypoint)
            .map(|node| node.label.as_str())
            .collect();

        for expected in [
            "cargo bin:demo",
            "cargo binary:worker",
            "npm script:start",
            "npm script:test",
            "python console_script:cg",
            "composer bin:bin/codegraph",
            "composer script:analyse",
            "cmake executable:demo_c",
            "go module:example.com/demo",
            "go command:server",
        ] {
            assert!(entrypoints.contains(expected), "missing {expected}");
        }
        assert!(!entrypoints.contains("cmake executable:alias_target"));
        assert!(!entrypoints.contains("cmake executable:imported_tool"));
        assert!(!entrypoints.contains("cmake executable:nope"));
        assert!(graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::Entrypoint && edge.confidence == Confidence::Exact
        }));
        assert!(graph.nodes.iter().any(|node| {
            node.kind == NodeKind::Entrypoint
                && node.label == "npm script:start"
                && node
                    .metadata
                    .get("target")
                    .is_some_and(|value| value == "node src/index.js")
        }));
        let cargo_entrypoint = node_id(&graph, NodeKind::Entrypoint, "cargo bin:demo");
        let cargo_file = node_id(&graph, NodeKind::File, "src/main.rs");
        let cargo_main = function_id_in_file(&graph, "main", "src/main.rs");
        let npm_entrypoint = node_id(&graph, NodeKind::Entrypoint, "npm script:start");
        let npm_file = node_id(&graph, NodeKind::File, "src/index.js");
        let python_entrypoint = node_id(&graph, NodeKind::Entrypoint, "python console_script:cg");
        let python_main = function_id_in_file(&graph, "main", "codegraph/cli.py");
        let composer_entrypoint =
            node_id(&graph, NodeKind::Entrypoint, "composer bin:bin/codegraph");
        let composer_file = node_id(&graph, NodeKind::File, "bin/codegraph");
        let cmake_entrypoint = node_id(&graph, NodeKind::Entrypoint, "cmake executable:demo_c");
        let cmake_file = node_id(&graph, NodeKind::File, "src/main.c");
        let cmake_main = function_id_in_file(&graph, "main", "src/main.c");
        let go_module_entrypoint =
            node_id(&graph, NodeKind::Entrypoint, "go module:example.com/demo");
        let go_module_file = node_id(&graph, NodeKind::File, "main.go");
        let go_module_main = function_id_in_file(&graph, "main", "main.go");
        let go_command_entrypoint = node_id(&graph, NodeKind::Entrypoint, "go command:server");
        let go_command_file = node_id(&graph, NodeKind::File, "cmd/server/main.go");
        let go_command_main = function_id_in_file(&graph, "main", "cmd/server/main.go");

        assert!(has_entrypoint_reference(
            &graph,
            cargo_entrypoint,
            cargo_file,
            "entrypoint_file",
            Confidence::Exact,
        ));
        assert!(has_entrypoint_reference(
            &graph,
            cargo_entrypoint,
            cargo_main,
            "entrypoint_function",
            Confidence::Syntactic,
        ));
        assert!(has_entrypoint_reference(
            &graph,
            npm_entrypoint,
            npm_file,
            "entrypoint_file",
            Confidence::Heuristic,
        ));
        assert!(has_entrypoint_reference(
            &graph,
            python_entrypoint,
            python_main,
            "entrypoint_function",
            Confidence::Heuristic,
        ));
        assert!(has_entrypoint_reference(
            &graph,
            composer_entrypoint,
            composer_file,
            "entrypoint_file",
            Confidence::Exact,
        ));
        assert!(has_entrypoint_reference(
            &graph,
            cmake_entrypoint,
            cmake_file,
            "entrypoint_file",
            Confidence::Exact,
        ));
        assert!(has_entrypoint_reference(
            &graph,
            cmake_entrypoint,
            cmake_main,
            "entrypoint_function",
            Confidence::Syntactic,
        ));
        assert!(has_entrypoint_reference(
            &graph,
            go_module_entrypoint,
            go_module_file,
            "entrypoint_file",
            Confidence::Exact,
        ));
        assert!(has_entrypoint_reference(
            &graph,
            go_module_entrypoint,
            go_module_main,
            "entrypoint_function",
            Confidence::Syntactic,
        ));
        assert!(has_entrypoint_reference(
            &graph,
            go_command_entrypoint,
            go_command_file,
            "entrypoint_file",
            Confidence::Exact,
        ));
        assert!(has_entrypoint_reference(
            &graph,
            go_command_entrypoint,
            go_command_main,
            "entrypoint_function",
            Confidence::Syntactic,
        ));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scan_project_adds_shebang_script_entrypoints() {
        let root = temp_project_root();
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::write(
            root.join("bin").join("deploy"),
            "#!/usr/bin/env bash\nmain() { echo deploy; }\nmain \"$@\"\n",
        )
        .unwrap();

        let graph = scan_project(&root, &IndexOptions::default()).unwrap();
        let script_entrypoint = node_id(&graph, NodeKind::Entrypoint, "script:bin/deploy");
        let script_file = node_id(&graph, NodeKind::File, "bin/deploy");
        let script_main = function_id_in_file(&graph, "main", "bin/deploy");
        let entrypoint = graph
            .nodes
            .iter()
            .find(|node| node.id == script_entrypoint)
            .expect("missing script entrypoint");

        assert_eq!(
            entrypoint.metadata.get("item_kind").map(String::as_str),
            Some("script_entrypoint")
        );
        assert_eq!(
            entrypoint.metadata.get("interpreter").map(String::as_str),
            Some("bash")
        );
        assert!(has_entrypoint_reference(
            &graph,
            script_entrypoint,
            script_file,
            "entrypoint_file",
            Confidence::Exact,
        ));
        assert!(has_entrypoint_reference(
            &graph,
            script_entrypoint,
            script_main,
            "entrypoint_function",
            Confidence::Syntactic,
        ));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scan_project_adds_framework_route_entrypoints() {
        let root = temp_project_root();
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("api.py"),
            "from fastapi import FastAPI\napp = FastAPI()\n\n@app.get(\"/health\")\ndef health():\n    return {}\n",
        )
        .unwrap();
        fs::write(
            root.join("server.js"),
            "const express = require('express');\nconst app = express();\nfunction listUsers() {}\napp.post('/users', listUsers);\n",
        )
        .unwrap();
        fs::write(
            root.join("main.go"),
            "package main\nimport \"net/http\"\nfunc health(w http.ResponseWriter, r *http.Request) {}\nfunc main() { http.HandleFunc(\"/ready\", health) }\n",
        )
        .unwrap();
        fs::write(
            root.join("router.rs"),
            "use axum::{routing::get, Router};\nasync fn status() {}\nfn app() -> Router { Router::new().route(\"/status\", get(status)) }\n",
        )
        .unwrap();
        fs::write(
            root.join("Controller.php"),
            "<?php\n#[Route('/admin', methods: ['GET'])]\nfunction admin() {}\n",
        )
        .unwrap();

        let graph = scan_project(&root, &IndexOptions::default()).unwrap();
        let expected = [
            ("route GET /health", "health", "api.py", "fastapi"),
            ("route POST /users", "listUsers", "server.js", "express"),
            ("route ROUTE /ready", "health", "main.go", "net/http"),
            ("route GET /status", "status", "router.rs", "axum"),
            (
                "route GET /admin",
                "admin",
                "Controller.php",
                "php-attribute",
            ),
        ];

        for (route_label, handler, path, framework) in expected {
            let entrypoint = node_id(&graph, NodeKind::Entrypoint, route_label);
            let file = node_id(&graph, NodeKind::File, path);
            let handler_id = function_id_in_file(&graph, handler, path);
            let node = graph
                .nodes
                .iter()
                .find(|node| node.id == entrypoint)
                .expect("missing route entrypoint");

            assert_eq!(
                node.metadata.get("item_kind").map(String::as_str),
                Some("framework_route")
            );
            assert_eq!(
                node.metadata.get("framework").map(String::as_str),
                Some(framework)
            );
            assert!(has_entrypoint_reference(
                &graph,
                entrypoint,
                file,
                "entrypoint_file",
                Confidence::Syntactic,
            ));
            assert!(has_entrypoint_reference(
                &graph,
                entrypoint,
                handler_id,
                "entrypoint_function",
                Confidence::Syntactic,
            ));
        }

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scan_project_adds_framework_config_conventions() {
        let root = temp_project_root();
        fs::create_dir_all(root.join("app")).unwrap();
        fs::create_dir_all(root.join("config")).unwrap();
        fs::write(root.join("app").join("settings.py"), "SECRET_KEY = 'dev'\n").unwrap();
        fs::write(
            root.join("app.py"),
            "from pydantic_settings import BaseSettings, SettingsConfigDict\napp.config.from_pyfile('settings.toml')\nclass Settings(BaseSettings):\n    model_config = SettingsConfigDict(env_file='.env.local')\n",
        )
        .unwrap();
        fs::write(
            root.join("server.js"),
            "const dotenv = require('dotenv');\ndotenv.config({ path: '.env.test' });\napp.set('view engine', 'pug');\n",
        )
        .unwrap();
        fs::write(root.join("next.config.ts"), "export default {};\n").unwrap();
        fs::write(
            root.join("main.go"),
            "package main\nfunc main() { viper.SetConfigName(\"service\"); viper.AddConfigPath(\"/etc/demo\"); godotenv.Load(\".env.go\") }\n",
        )
        .unwrap();
        fs::write(
            root.join("config").join("app.php"),
            "<?php\nreturn ['name' => config('app.name')];\n",
        )
        .unwrap();
        fs::write(root.join("deploy.sh"), "source config/runtime.env\n").unwrap();

        let graph = scan_project(&root, &IndexOptions::default()).unwrap();
        let expected = [
            (
                "django settings:app/settings.py",
                "django",
                "settings_module",
            ),
            ("flask config:settings.toml", "flask", "config_file"),
            ("pydantic settings:Settings", "pydantic", "settings_class"),
            ("pydantic env file:.env.local", "pydantic", "env_file"),
            ("dotenv config:.env.test", "dotenv", "env_file"),
            ("express setting:view engine", "express", "setting"),
            ("nextjs config:next.config.ts", "nextjs", "config_file"),
            ("viper config:service", "viper", "config_name"),
            ("viper config path:/etc/demo", "viper", "config_path"),
            ("godotenv config:.env.go", "godotenv", "env_file"),
            ("laravel config:config/app.php", "laravel", "config_file"),
            ("laravel config key:app.name", "laravel", "config_key"),
            ("shell config:config/runtime.env", "shell", "source_file"),
        ];

        for (label, framework, config_kind) in expected {
            let config_id = node_id(&graph, NodeKind::Config, label);
            let node = graph
                .nodes
                .iter()
                .find(|node| node.id == config_id)
                .expect("missing framework config node");
            assert_eq!(
                node.metadata.get("item_kind").map(String::as_str),
                Some("framework_config")
            );
            assert_eq!(
                node.metadata.get("framework").map(String::as_str),
                Some(framework)
            );
            assert_eq!(
                node.metadata.get("config_kind").map(String::as_str),
                Some(config_kind)
            );
            assert!(graph.edges.iter().any(|edge| {
                edge.target == config_id
                    && edge.kind == EdgeKind::ReadsConfig
                    && edge.confidence == Confidence::Syntactic
                    && edge
                        .metadata
                        .get("source")
                        .is_some_and(|value| value == "framework")
            }));
        }

        fs::remove_dir_all(root).unwrap();
    }

    fn node_id(graph: &CodeGraph, kind: NodeKind, label: &str) -> NodeId {
        graph
            .nodes
            .iter()
            .find(|node| node.kind == kind && node.label == label)
            .map(|node| node.id)
            .unwrap_or_else(|| panic!("missing {kind:?} node `{label}`"))
    }

    fn function_id_in_file(graph: &CodeGraph, label: &str, path: &str) -> NodeId {
        graph
            .nodes
            .iter()
            .find(|node| {
                node.kind == NodeKind::Function
                    && node.label == label
                    && node.span.as_ref().is_some_and(|span| span.path == path)
            })
            .map(|node| node.id)
            .unwrap_or_else(|| panic!("missing function `{label}` in `{path}`"))
    }

    fn has_entrypoint_reference(
        graph: &CodeGraph,
        source: NodeId,
        target: NodeId,
        relation: &str,
        confidence: Confidence,
    ) -> bool {
        graph.edges.iter().any(|edge| {
            edge.source == source
                && edge.target == target
                && edge.kind == EdgeKind::References
                && edge.confidence == confidence
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|value| value == relation)
        })
    }

    fn temp_project_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "codegraph-indexer-test-{}-{nanos}-{id}",
            std::process::id()
        ))
    }
}
