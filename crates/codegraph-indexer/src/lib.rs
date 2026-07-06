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
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ManifestEntrypoint {
    label: String,
    kind: String,
    ecosystem: String,
    target: Option<String>,
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
    let mut context = IndexContext {
        graph: CodeGraph::new(root_label),
        function_symbols: BTreeMap::new(),
        file_nodes: BTreeMap::new(),
        external_dependencies: BTreeMap::new(),
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
    let language = Language::detect(path);
    let mut metadata = BTreeMap::new();

    if let Some(language) = language {
        metadata.insert("language".to_string(), language.to_string());
    }

    let parse_result = language.and_then(|language| match fs::read(path) {
        Ok(source) => Some((language, parse_source(label, &source, language))),
        Err(error) => {
            metadata.insert("read_error".to_string(), error.to_string());
            None
        }
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

    if let Ok(source) = fs::read_to_string(path) {
        index_manifest_facts(context, file_id, path, label, &source);
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

fn index_manifest_dependencies(
    context: &mut IndexContext,
    file_id: NodeId,
    path: &Path,
    source: &str,
) {
    let dependencies = manifest_dependencies(path, source);
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

fn manifest_dependencies(path: &Path, source: &str) -> Vec<ManifestDependency> {
    match path.file_name().and_then(|name| name.to_str()) {
        Some("Cargo.toml") => cargo_dependencies(source),
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
        Some("pyproject.toml") => pyproject_entrypoints(source),
        Some("composer.json") => composer_entrypoints(source),
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
    {
        if path
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

fn cargo_dependencies(source: &str) -> Vec<ManifestDependency> {
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
    );
    collect_toml_table_keys(
        &value,
        "dev-dependencies",
        "dev",
        "cargo",
        &mut dependencies,
    );
    collect_toml_table_keys(
        &value,
        "build-dependencies",
        "build",
        "cargo",
        &mut dependencies,
    );

    if let Some(targets) = value.get("target").and_then(|value| value.as_table()) {
        for target in targets.values() {
            collect_toml_table_keys(
                target,
                "dependencies",
                "runtime",
                "cargo",
                &mut dependencies,
            );
            collect_toml_table_keys(
                target,
                "dev-dependencies",
                "dev",
                "cargo",
                &mut dependencies,
            );
            collect_toml_table_keys(
                target,
                "build-dependencies",
                "build",
                "cargo",
                &mut dependencies,
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
                if let Some(name) = value.as_str().and_then(package_name_from_requirement) {
                    dependencies.push(manifest_dependency(name, "runtime", "python"));
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
                        if let Some(name) = value.as_str().and_then(package_name_from_requirement) {
                            dependencies.push(manifest_dependency(name, "optional", "python"));
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
        );
        collect_toml_table_keys(
            poetry,
            "dev-dependencies",
            "dev",
            "python",
            &mut dependencies,
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
        if let Some(name) = requirement.split_whitespace().next() {
            dependencies.push(manifest_dependency(name.to_string(), "runtime", "go"));
        }
    }
    dependencies
}

fn requirements_dependencies(source: &str) -> Vec<ManifestDependency> {
    source
        .lines()
        .filter_map(|line| {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() || line.starts_with('-') {
                return None;
            }
            package_name_from_requirement(line)
                .map(|name| manifest_dependency(name, "runtime", "python"))
        })
        .collect()
}

fn collect_toml_table_keys(
    value: &toml::Value,
    table_name: &str,
    dependency_kind: &str,
    ecosystem: &str,
    dependencies: &mut Vec<ManifestDependency>,
) {
    let Some(table) = value.get(table_name).and_then(|value| value.as_table()) else {
        return;
    };
    for name in table.keys() {
        dependencies.push(manifest_dependency(
            name.clone(),
            dependency_kind,
            ecosystem,
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
    for name in object.keys() {
        dependencies.push(manifest_dependency(
            name.clone(),
            dependency_kind,
            ecosystem,
        ));
    }
}

fn package_name_from_requirement(requirement: &str) -> Option<String> {
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
        Some(name.to_string())
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
) -> ManifestDependency {
    ManifestDependency {
        name: name.into(),
        kind: dependency_kind.into(),
        ecosystem: ecosystem.into(),
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
    metadata.insert("source".to_string(), "manifest".to_string());
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
        fs::write(root.join("src").join("main.rs"), "fn main() {}\n").unwrap();
        fs::write(root.join("target").join("debug.log"), "noise\n").unwrap();

        let graph = scan_project(&root, &IndexOptions::default()).unwrap();
        let labels: Vec<_> = graph.nodes.iter().map(|node| node.label.as_str()).collect();

        assert!(labels.contains(&"src/main.rs"));
        assert!(!labels.contains(&"target"));
        assert!(!labels.contains(&"target/debug.log"));

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

[dependencies]
serde = "1"

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
serde = "1"
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
        ] {
            assert!(entrypoints.contains(expected), "missing {expected}");
        }
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
