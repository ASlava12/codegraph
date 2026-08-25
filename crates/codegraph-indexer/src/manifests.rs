//! Package manifest extraction across ecosystems: dependencies,
//! entrypoints, lockfiles, package identity, and manifest text parsing
//! helpers.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use codegraph_core::{CodeGraph, Confidence, EdgeKind, NodeId, NodeKind, SourceSpan};
use codegraph_parser::Language;
use globset::GlobSet;
use walkdir::WalkDir;

#[allow(unused_imports)]
use crate::*;

/// The package this manifest declares the project itself to be. A project
/// does not depend on itself: guzzle's own sources `use GuzzleHttp\…`,
/// and without this the graph reported 363 imports of an undeclared
/// `guzzlehttp/guzzle`.
pub(crate) fn manifest_own_package_id(path: &Path, source: &str) -> Option<String> {
    match path.file_name().and_then(|name| name.to_str()) {
        Some("package.json") => package_json_name(source).map(|name| format!("npm:{name}")),
        Some("composer.json") => serde_json::from_str::<serde_json::Value>(source)
            .ok()
            .and_then(|value| value.get("name")?.as_str().map(str::to_string))
            .map(|name| format!("composer:{}", name.to_ascii_lowercase())),
        Some("Cargo.toml") => source
            .lines()
            .skip_while(|line| line.trim() != "[package]")
            .skip(1)
            .take_while(|line| !line.trim_start().starts_with('['))
            .find_map(|line| {
                let (key, value) = line.split_once('=')?;
                (key.trim() == "name").then(|| value.trim().trim_matches('"').to_string())
            })
            .map(|name| format!("cargo:{name}")),
        _ => None,
    }
}

pub(crate) fn index_manifest_dependencies(
    context: &mut IndexContext,
    file_id: NodeId,
    path: &Path,
    source: &str,
) {
    // A manifest that does not parse declares nothing; say so rather than
    // letting every dependency finding describe a file nobody could read.
    if let Some(reason) = manifest_parse_error(path, source) {
        add_file_metadata(&mut context.graph, file_id, "manifest_parse_error", reason);
    }
    if let Some(own) = manifest_own_package_id(path, source) {
        context.own_package_ids.insert(own);
    }
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
        // The node is made the first time the package is named, which is
        // usually the manifest; the lockfile beside it is what states the
        // namespaces, so it has to reach a node that already exists.
        if !dependency.namespaces.is_empty() {
            add_node_metadata(
                &mut context.graph,
                dependency_id,
                "autoloaded_namespaces",
                dependency.namespaces.join(","),
            );
        }

        let mut edge_metadata = BTreeMap::new();
        edge_metadata.insert("dependency_kind".to_string(), dependency.kind);
        edge_metadata.insert("source".to_string(), "manifest".to_string());
        if let Some(version) = dependency.version {
            edge_metadata.insert("dependency_version".to_string(), version);
            if let Some(version_kind) = dependency.version_kind {
                edge_metadata.insert("dependency_version_kind".to_string(), version_kind);
            }
        }
        add_manifest_dependency_edge_once(
            &mut context.graph,
            file_id,
            dependency_id,
            edge_metadata,
        );
    }
}

pub(crate) fn add_manifest_dependency_edge_once(
    graph: &mut CodeGraph,
    file_id: NodeId,
    dependency_id: NodeId,
    metadata: BTreeMap<String, String>,
) {
    if graph.edges.iter().any(|edge| {
        edge.source == file_id
            && edge.target == dependency_id
            && edge.kind == EdgeKind::DependsOn
            && edge.metadata.get("dependency_kind") == metadata.get("dependency_kind")
            && edge.metadata.get("dependency_version") == metadata.get("dependency_version")
    }) {
        return;
    }
    graph.add_edge_with_metadata(
        file_id,
        dependency_id,
        EdgeKind::DependsOn,
        Confidence::Exact,
        metadata,
    );
}

/// The line of a manifest that declares an entry by name: `"start": ..`,
/// `name = "codegraph"`, `start:`. Nothing is claimed when the name is not
/// written plainly, because a span pointing at the wrong line is worse than
/// one the reader knows is missing.
fn manifest_entry_line(source: &str, name: &str) -> Option<u32> {
    if name.is_empty() {
        return None;
    }
    source
        .lines()
        .enumerate()
        .find_map(|(index, line)| line_declares_entry(line, name).then_some(index as u32 + 1))
}

fn line_declares_entry(line: &str, name: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') || trimmed.starts_with("//") {
        return false;
    }
    trimmed.starts_with(&format!("\"{name}\""))
        || trimmed.starts_with(&format!("'{name}'"))
        || trimmed.starts_with(&format!("{name} ="))
        || trimmed.starts_with(&format!("{name}="))
        || trimmed.starts_with(&format!("{name}:"))
        || trimmed.contains(&format!("\"{name}\":"))
        || trimmed.contains(&format!("= \"{name}\""))
        // `module github.com/hashicorp/terraform`: the keyword states
        // what kind of entry it is and the name closes the line.
        || (trimmed.ends_with(name) && trimmed.len() > name.len() + 1)
}

/// The line an entry is written on inside the section that declares it.
/// oscar's package.json writes `"eslint"` twice -- once as a dev
/// dependency on line 11 and once as a script on line 30 -- so a search
/// of the whole file cited the dependency as the script's home. The
/// section that owns the entry has to bound the search.
fn manifest_entry_line_in_sections(source: &str, sections: &[&str], name: &str) -> Option<u32> {
    if name.is_empty() {
        return None;
    }
    let lines: Vec<&str> = source.lines().collect();
    for section in sections {
        for (index, line) in lines.iter().enumerate() {
            let Some(shape) = section_opens_here(line, section) else {
                continue;
            };
            let found = lines
                .iter()
                .enumerate()
                .skip(index + 1)
                .take_while(|(offset, candidate)| {
                    !section_ends_here(shape, line, candidate, &lines[index + 1..*offset])
                })
                .find_map(|(offset, candidate)| {
                    line_declares_entry(candidate, name).then_some(offset as u32 + 1)
                });
            if found.is_some() {
                return found;
            }
        }
    }
    None
}

/// How a manifest writes the head of a section: JSON quotes the key, TOML
/// brackets it, YAML and INI-shaped files end it with a colon or bracket.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SectionShape {
    Braced,
    Bracketed,
    Indented,
}

fn section_opens_here(line: &str, section: &str) -> Option<SectionShape> {
    let trimmed = line.trim();
    if trimmed.starts_with(&format!("\"{section}\"")) {
        return Some(SectionShape::Braced);
    }
    if trimmed.starts_with(&format!("[{section}]"))
        || trimmed.starts_with(&format!("[[{section}]]"))
    {
        return Some(SectionShape::Bracketed);
    }
    if trimmed.starts_with(&format!("{section}:")) || trimmed.starts_with(&format!("{section} =")) {
        return Some(SectionShape::Indented);
    }
    None
}

fn section_ends_here(shape: SectionShape, head: &str, line: &str, between: &[&str]) -> bool {
    match shape {
        // The section's own braces close it: count what the head opened.
        SectionShape::Braced => {
            let mut depth = brace_depth(head);
            for previous in between {
                depth += brace_depth(previous);
            }
            depth <= 0
        }
        SectionShape::Bracketed => line.trim_start().starts_with('['),
        SectionShape::Indented => {
            let indent = head.len() - head.trim_start().len();
            !line.trim().is_empty() && line.len() - line.trim_start().len() <= indent
        }
    }
}

fn brace_depth(line: &str) -> i32 {
    let mut depth = 0;
    let mut quoted = false;
    let mut escaped = false;
    for character in line.chars() {
        match character {
            _ if escaped => escaped = false,
            '\\' if quoted => escaped = true,
            '"' => quoted = !quoted,
            '{' | '[' if !quoted => depth += 1,
            '}' | ']' if !quoted => depth -= 1,
            _ => {}
        }
    }
    depth
}

/// Which sections of a manifest can declare this kind of entry. An
/// ecosystem states its entrypoints in a named place, and naming that
/// place here keeps every extractor free of it.
fn manifest_entry_sections(ecosystem: &str, kind: &str) -> &'static [&'static str] {
    match (ecosystem, kind) {
        ("npm", "script") => &["scripts"],
        ("composer", "script") => &["scripts"],
        ("composer", "bin") => &["bin"],
        ("cargo", "binary") => &["bin", "package"],
        ("cargo", "example") => &["example"],
        ("python", "console_script") => {
            &["project.scripts", "options.entry_points", "console_scripts"]
        }
        ("python", "gui_script") => &["project.gui-scripts"],
        ("python", "poetry_script") => &["tool.poetry.scripts"],
        _ => &[],
    }
}

pub(crate) fn index_manifest_entrypoints(
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

        // A manifest entry is written on a line, and a reader following an
        // entrypoint wants that line: `"start": "node server.js"` is where
        // the program is declared, and the node used to point nowhere.
        let name = entrypoint.label.split_once(':').map(|(_, name)| name);
        let sections = manifest_entry_sections(&entrypoint.ecosystem, &entrypoint.kind);
        let span = entrypoint
            .line
            .or_else(|| {
                name.and_then(|name| {
                    if sections.is_empty() {
                        manifest_entry_line(source, name)
                    } else {
                        // The section knows where its entries are; the rest
                        // of the file only knows where the name appears.
                        manifest_entry_line_in_sections(source, sections, name)
                    }
                })
            })
            .map(|line| SourceSpan {
                path: label.to_string(),
                start_line: line,
                start_column: 0,
                end_line: line,
                end_column: 0,
            });

        let entrypoint_id = context.graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            entrypoint.label,
            span,
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
        if let Some(target) = entrypoint.target {
            context
                .pending_entrypoint_targets
                .push(PendingEntrypointTarget {
                    entrypoint: entrypoint_id,
                    manifest_label: label.to_string(),
                    target,
                    base_dir: None,
                    ecosystem: entrypoint.ecosystem,
                    entrypoint_kind: entrypoint.kind,
                });
        }
    }
}

pub(crate) fn index_pubspec_assets(
    context: &mut IndexContext,
    file_id: NodeId,
    path: &Path,
    label: &str,
    source: &str,
) {
    if path.file_name().and_then(|name| name.to_str()) != Some("pubspec.yaml") {
        return;
    }

    for asset in pubspec_flutter_assets(source) {
        let mut metadata = BTreeMap::new();
        metadata.insert("item_kind".to_string(), "flutter_asset".to_string());
        metadata.insert("source".to_string(), "pubspec".to_string());
        metadata.insert("framework".to_string(), "flutter".to_string());
        metadata.insert("config_kind".to_string(), "flutter_asset".to_string());
        metadata.insert("asset_path".to_string(), asset.path.clone());
        metadata.insert("target".to_string(), label.to_string());
        metadata.insert("line".to_string(), asset.line.to_string());
        let asset_id = context.graph.add_node_with_metadata(
            NodeKind::Config,
            format!("flutter asset:{}", asset.path),
            Some(line_span(label, source, asset.line)),
            metadata,
        );
        let mut edge_metadata = BTreeMap::new();
        edge_metadata.insert("source".to_string(), "pubspec".to_string());
        edge_metadata.insert("framework".to_string(), "flutter".to_string());
        edge_metadata.insert("config_kind".to_string(), "flutter_asset".to_string());
        add_edge_once_with_metadata(
            context,
            file_id,
            asset_id,
            EdgeKind::Contains,
            Confidence::Exact,
            edge_metadata,
        );
    }
}

pub(crate) fn cargo_entrypoints(path: &Path, source: &str) -> Vec<ManifestEntrypoint> {
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

pub(crate) fn collect_cargo_target_entrypoints(
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

pub(crate) fn package_json_entrypoints(source: &str) -> Vec<ManifestEntrypoint> {
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

pub(crate) fn go_mod_entrypoints(path: &Path, source: &str) -> Vec<ManifestEntrypoint> {
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

pub(crate) fn pubspec_entrypoints(path: &Path, source: &str) -> Vec<ManifestEntrypoint> {
    let Some(package_name) = pubspec_package_name(source) else {
        return Vec::new();
    };
    let Some(root) = path.parent() else {
        return Vec::new();
    };

    let mut entrypoints = Vec::new();
    if root.join("lib").join("main.dart").is_file() {
        let ecosystem = if pubspec_uses_flutter(source) {
            "flutter"
        } else {
            "dart"
        };
        let prefix = if ecosystem == "flutter" {
            "flutter app"
        } else {
            "dart package"
        };
        entrypoints.push(manifest_entrypoint(
            format!("{prefix}:{package_name}"),
            "app",
            ecosystem,
            Some("lib/main.dart".to_string()),
        ));
    }

    let bin_dir = root.join("bin");
    if let Ok(commands) = fs::read_dir(&bin_dir) {
        for command in commands.flatten() {
            let command_path = command.path();
            if command_path
                .extension()
                .and_then(|extension| extension.to_str())
                != Some("dart")
            {
                continue;
            }
            let Some(name) = command_path.file_stem().and_then(|name| name.to_str()) else {
                continue;
            };
            entrypoints.push(manifest_entrypoint(
                format!("dart bin:{name}"),
                "binary",
                "dart",
                Some(format!("bin/{name}.dart")),
            ));
        }
    }

    let test_dir = root.join("test");
    if let Ok(tests) = fs::read_dir(&test_dir) {
        for test in tests.flatten() {
            let test_path = test.path();
            let Some(name) = test_path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if !name.ends_with("_test.dart") {
                continue;
            }
            entrypoints.push(manifest_entrypoint(
                format!("dart test:{name}"),
                "test",
                "dart",
                Some(format!("test/{name}")),
            ));
        }
    }

    entrypoints
}

pub(crate) fn pyproject_entrypoints(source: &str) -> Vec<ManifestEntrypoint> {
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

pub(crate) fn setup_py_entrypoints(source: &str) -> Vec<ManifestEntrypoint> {
    setup_py_console_scripts(source)
        .into_iter()
        .filter_map(|entrypoint| {
            let (name, target) = python_console_script_name_and_target(&entrypoint)?;
            Some(manifest_entrypoint(
                format!("python console_script:{name}"),
                "console_script",
                "python",
                Some(target),
            ))
        })
        .collect()
}

pub(crate) fn setup_cfg_entrypoints(source: &str) -> Vec<ManifestEntrypoint> {
    let sections = setup_cfg_sections(source);
    setup_cfg_values(&sections, "options.entry_points", "console_scripts")
        .into_iter()
        .filter_map(|entrypoint| {
            let (name, target) = python_console_script_name_and_target(&entrypoint)?;
            Some(manifest_entrypoint(
                format!("python console_script:{name}"),
                "console_script",
                "python",
                Some(target),
            ))
        })
        .collect()
}

pub(crate) fn composer_entrypoints(source: &str) -> Vec<ManifestEntrypoint> {
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

pub(crate) fn cmake_entrypoints(source: &str) -> Vec<ManifestEntrypoint> {
    cmake_command_sites(source, "add_executable")
        .into_iter()
        .filter_map(|(body, line)| {
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
            Some(manifest_entrypoint_at(
                format!("cmake executable:{name}"),
                "executable",
                "cmake",
                target,
                line,
            ))
        })
        .collect()
}

/// The programs a `.cabal` file declares. Haskell states them in stanzas:
/// `executable shellcheck` with `main-is: shellcheck.hs`, and the module
/// sits under the stanza's `hs-source-dirs` when it names one. A
/// `test-suite` states a program too, and says it is a test.
pub(crate) fn cabal_entrypoints(source: &str) -> Vec<ManifestEntrypoint> {
    let mut entrypoints = Vec::new();
    let mut stanza: Option<(&'static str, String, u32)> = None;
    let mut main_is: Option<String> = None;
    let mut source_dir: Option<String> = None;
    let finish = |stanza: &Option<(&'static str, String, u32)>,
                  main_is: &Option<String>,
                  source_dir: &Option<String>,
                  entrypoints: &mut Vec<ManifestEntrypoint>| {
        let (Some((kind, name, line)), Some(main)) = (stanza.as_ref(), main_is.as_ref()) else {
            return;
        };
        let target = match source_dir.as_deref() {
            Some(directory) if directory != "." => format!("{directory}/{main}"),
            _ => main.clone(),
        };
        entrypoints.push(manifest_entrypoint_at(
            format!("cabal {kind}:{name}"),
            *kind,
            "cabal",
            Some(target),
            *line,
        ));
    };
    for (index, raw) in source.lines().enumerate() {
        let line = index as u32 + 1;
        // A stanza opens at the left margin; its fields are indented.
        if !raw.starts_with(char::is_whitespace) && !raw.trim().is_empty() {
            finish(&stanza, &main_is, &source_dir, &mut entrypoints);
            stanza = None;
            main_is = None;
            source_dir = None;
            let head = raw.trim();
            for (keyword, kind) in [
                ("executable ", "executable"),
                ("test-suite ", "test"),
                ("benchmark ", "benchmark"),
            ] {
                if let Some(name) = head.strip_prefix(keyword) {
                    let name = name.trim();
                    if !name.is_empty() {
                        stanza = Some((kind, name.to_string(), line));
                    }
                }
            }
            continue;
        }
        let Some((field, value)) = raw.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match field.trim().to_ascii_lowercase().as_str() {
            "main-is" if !value.is_empty() => main_is = Some(value.to_string()),
            // `hs-source-dirs: src, other` states more than one root; the
            // first is where a program's own module is looked for.
            "hs-source-dirs" if !value.is_empty() => {
                source_dir = value
                    .split(',')
                    .next()
                    .map(str::trim)
                    .filter(|directory| !directory.is_empty())
                    .map(ToString::to_string);
            }
            _ => {}
        }
    }
    finish(&stanza, &main_is, &source_dir, &mut entrypoints);
    entrypoints
}

/// The programs a `dune` file declares. Dune is how OCaml projects state
/// what they build: `(executable (name main))` in `bin/dune` is
/// `bin/main.ml`, and `(executables (names a b))` states two. Without
/// reading them, dune's own repository showed 1% of its functions as
/// reachable from an entrypoint -- the compiler knows where its programs
/// start and the graph did not.
pub(crate) fn dune_entrypoints(source: &str) -> Vec<ManifestEntrypoint> {
    let mut entrypoints = Vec::new();
    for (head, body, line) in dune_stanzas(source) {
        let kind = match head.as_str() {
            "executable" | "executables" => "executable",
            "test" | "tests" => "test",
            _ => continue,
        };
        for name in dune_stanza_names(&body) {
            entrypoints.push(manifest_entrypoint_at(
                format!("dune {kind}:{name}"),
                kind,
                "dune",
                Some(format!("{name}.ml")),
                line,
            ));
        }
    }
    entrypoints
}

/// Every top-level stanza of a dune file: the symbol that opens it, the
/// text inside its parentheses, and the line it starts on. A `;` comment
/// runs to the end of its line, and a string can hold a parenthesis.
fn dune_stanzas(source: &str) -> Vec<(String, String, u32)> {
    let mut stanzas = Vec::new();
    let mut depth = 0usize;
    let mut line = 1u32;
    let mut start_line = 1u32;
    let mut body = String::new();
    let mut in_comment = false;
    let mut in_string = false;
    let mut previous = ' ';
    for character in source.chars() {
        if character == '\n' {
            line += 1;
            in_comment = false;
        }
        if in_comment {
            continue;
        }
        if in_string {
            body.push(character);
            if character == '"' && previous != '\\' {
                in_string = false;
            }
            previous = character;
            continue;
        }
        match character {
            ';' if depth == 0 || !in_string => {
                in_comment = true;
                continue;
            }
            '"' => {
                in_string = true;
                body.push(character);
            }
            '(' => {
                if depth == 0 {
                    body.clear();
                    start_line = line;
                } else {
                    body.push(character);
                }
                depth += 1;
            }
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let head = body
                        .split(|c: char| c.is_whitespace() || c == '(')
                        .next()
                        .unwrap_or("")
                        .to_string();
                    stanzas.push((head, std::mem::take(&mut body), start_line));
                } else {
                    body.push(character);
                }
            }
            _ if depth > 0 => body.push(character),
            _ => {}
        }
        previous = character;
    }
    stanzas
}

/// The names a stanza states: `(name main)` gives one, `(names a b)` gives
/// each. A name is a plain symbol -- a variable (`%{...}`) is not one.
fn dune_stanza_names(body: &str) -> Vec<String> {
    let mut names = Vec::new();
    for field in ["name ", "names "] {
        let mut rest = body;
        while let Some(offset) = rest.find(field) {
            let after = &rest[offset + field.len()..];
            // The field has to open a form of its own: `(name main)`, not
            // `public_name` or `root_module`.
            let opens_the_field = rest[..offset]
                .chars()
                .next_back()
                .is_some_and(|character| character == '(');
            let value = after.split(')').next().unwrap_or("");
            if opens_the_field {
                names.extend(
                    value
                        .split_whitespace()
                        .filter(|name| {
                            !name.is_empty()
                                && name.chars().all(|character| {
                                    character.is_ascii_alphanumeric()
                                        || matches!(character, '_' | '-' | '.')
                                })
                        })
                        .map(ToString::to_string),
                );
            }
            rest = after;
        }
    }
    names.sort();
    names.dedup();
    names
}

pub(crate) fn cargo_dependencies(
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

pub(crate) fn pyproject_dependencies(source: &str) -> Vec<ManifestDependency> {
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

    // PEP 735 development groups, which uv fills in and pip installs with
    // `--group`. flask keeps `cryptography`, `python-dotenv` and its test
    // and typing tools here, and nothing else in the file declares them.
    if let Some(groups) = value
        .get("dependency-groups")
        .and_then(|value| value.as_table())
    {
        for values in groups.values() {
            let Some(values) = values.as_array() else {
                continue;
            };
            for value in values {
                // `{include-group = "tests"}` pulls in another group rather
                // than naming a package.
                if let Some((name, version)) = value
                    .as_str()
                    .and_then(package_name_and_version_from_requirement)
                {
                    dependencies.push(manifest_dependency(name, "dev", "python", version));
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
        collect_poetry_group_dependencies(poetry, &mut dependencies);
    }

    dependencies
}

pub(crate) fn collect_poetry_group_dependencies(
    poetry: &toml::Value,
    dependencies: &mut Vec<ManifestDependency>,
) {
    let Some(groups) = poetry.get("group").and_then(|value| value.as_table()) else {
        return;
    };
    for (group_name, group) in groups {
        let dependency_kind = poetry_group_dependency_kind(group_name);
        collect_toml_table_keys(
            group,
            "dependencies",
            dependency_kind,
            "python",
            dependencies,
            None,
        );
    }
}

pub(crate) fn poetry_group_dependency_kind(group_name: &str) -> &'static str {
    match group_name.to_ascii_lowercase().as_str() {
        "dev" | "develop" | "development" => "dev",
        "test" | "tests" | "testing" => "test",
        _ => "optional",
    }
}

pub(crate) fn setup_py_dependencies(source: &str) -> Vec<ManifestDependency> {
    let mut dependencies = Vec::new();
    collect_setup_py_requirement_key(source, "install_requires", "runtime", &mut dependencies);
    collect_setup_py_requirement_key(source, "setup_requires", "build", &mut dependencies);
    collect_setup_py_requirement_key(source, "tests_require", "test", &mut dependencies);

    for requirement in setup_py_dict_list_string_values(source, "extras_require") {
        if let Some((name, version)) = package_name_and_version_from_requirement(&requirement) {
            dependencies.push(manifest_dependency(name, "optional", "python", version));
        }
    }

    dependencies
}

pub(crate) fn collect_setup_py_requirement_key(
    source: &str,
    key: &str,
    dependency_kind: &str,
    dependencies: &mut Vec<ManifestDependency>,
) {
    for requirement in setup_py_sequence_string_values(source, key) {
        if let Some((name, version)) = package_name_and_version_from_requirement(&requirement) {
            dependencies.push(manifest_dependency(
                name,
                dependency_kind,
                "python",
                version,
            ));
        }
    }
}

pub(crate) fn setup_cfg_dependencies(source: &str) -> Vec<ManifestDependency> {
    let sections = setup_cfg_sections(source);
    let mut dependencies = Vec::new();
    collect_setup_cfg_requirement_key(
        &sections,
        "options",
        "install_requires",
        "runtime",
        &mut dependencies,
    );
    collect_setup_cfg_requirement_key(
        &sections,
        "options",
        "setup_requires",
        "build",
        &mut dependencies,
    );
    collect_setup_cfg_requirement_key(
        &sections,
        "options",
        "tests_require",
        "test",
        &mut dependencies,
    );

    if let Some(extras) = sections.get("options.extras_require") {
        for requirements in extras.values() {
            for requirement in requirements {
                if let Some((name, version)) =
                    package_name_and_version_from_requirement(requirement)
                {
                    dependencies.push(manifest_dependency(name, "optional", "python", version));
                }
            }
        }
    }

    dependencies
}

pub(crate) fn pipfile_dependencies(source: &str) -> Vec<ManifestDependency> {
    let Ok(value) = toml::from_str::<toml::Value>(source) else {
        return Vec::new();
    };
    let mut dependencies = Vec::new();
    collect_pipfile_table(&value, "packages", "runtime", &mut dependencies);
    collect_pipfile_table(&value, "dev-packages", "dev", &mut dependencies);
    dependencies
}

pub(crate) fn collect_pipfile_table(
    value: &toml::Value,
    table_name: &str,
    dependency_kind: &str,
    dependencies: &mut Vec<ManifestDependency>,
) {
    let Some(table) = value.get(table_name).and_then(|value| value.as_table()) else {
        return;
    };
    for (name, value) in table {
        dependencies.push(manifest_dependency(
            name.clone(),
            dependency_kind,
            "python",
            pipfile_dependency_version(value),
        ));
    }
}

pub(crate) fn collect_setup_cfg_requirement_key(
    sections: &BTreeMap<String, BTreeMap<String, Vec<String>>>,
    section: &str,
    key: &str,
    dependency_kind: &str,
    dependencies: &mut Vec<ManifestDependency>,
) {
    for requirement in setup_cfg_values(sections, section, key) {
        if let Some((name, version)) = package_name_and_version_from_requirement(&requirement) {
            dependencies.push(manifest_dependency(
                name,
                dependency_kind,
                "python",
                version,
            ));
        }
    }
}

pub(crate) fn package_json_dependencies(source: &str) -> Vec<ManifestDependency> {
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

pub(crate) fn package_lock_dependencies(source: &str) -> Vec<ManifestDependency> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(source) else {
        return Vec::new();
    };
    let Some(root_package) = value
        .get("packages")
        .and_then(|packages| packages.get(""))
        .and_then(|package| package.as_object())
    else {
        return Vec::new();
    };

    let mut dependencies = Vec::new();
    collect_package_lock_root_dependencies(
        &value,
        root_package,
        "dependencies",
        "runtime",
        &mut dependencies,
    );
    collect_package_lock_root_dependencies(
        &value,
        root_package,
        "devDependencies",
        "dev",
        &mut dependencies,
    );
    collect_package_lock_root_dependencies(
        &value,
        root_package,
        "peerDependencies",
        "peer",
        &mut dependencies,
    );
    collect_package_lock_root_dependencies(
        &value,
        root_package,
        "optionalDependencies",
        "optional",
        &mut dependencies,
    );
    dependencies
}

pub(crate) fn collect_package_lock_root_dependencies(
    value: &serde_json::Value,
    root_package: &serde_json::Map<String, serde_json::Value>,
    object_name: &str,
    dependency_kind: &str,
    dependencies: &mut Vec<ManifestDependency>,
) {
    let Some(object) = root_package
        .get(object_name)
        .and_then(|value| value.as_object())
    else {
        return;
    };
    for (name, declared) in object {
        let locked_version = package_lock_package_version(value, name);
        let declared_version = declared
            .as_str()
            .map(str::trim)
            .filter(|version| !version.is_empty())
            .map(str::to_string);
        let (version, version_kind) = if let Some(version) = locked_version {
            (Some(version), Some("locked"))
        } else {
            (declared_version, Some("constraint"))
        };
        dependencies.push(manifest_dependency_with_version_kind(
            name.clone(),
            dependency_kind,
            "npm",
            version,
            version_kind,
        ));
    }
}

pub(crate) fn package_lock_package_version(
    value: &serde_json::Value,
    name: &str,
) -> Option<String> {
    value
        .get("packages")
        .and_then(|packages| packages.get(format!("node_modules/{name}")))
        .and_then(|package| package.get("version"))
        .and_then(|version| version.as_str())
        .map(str::trim)
        .filter(|version| !version.is_empty())
        .map(str::to_string)
}

#[derive(Debug)]
pub(crate) struct PendingPnpmDependency {
    name: String,
    kind: String,
    indent: usize,
    specifier: Option<String>,
    version: Option<String>,
}

pub(crate) fn pnpm_lock_dependencies(source: &str) -> Vec<ManifestDependency> {
    let mut dependencies = Vec::new();
    let mut in_importers = false;
    let mut in_importer = false;
    let mut active_section: Option<(&str, usize)> = None;
    let mut pending: Option<PendingPnpmDependency> = None;

    for raw_line in source.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = yaml_indent(raw_line);

        if indent == 0 {
            flush_pnpm_dependency(&mut pending, &mut dependencies);
            in_importers = trimmed == "importers:";
            in_importer = false;
            active_section = None;
            continue;
        }

        if !in_importers {
            continue;
        }

        if indent == 2 {
            flush_pnpm_dependency(&mut pending, &mut dependencies);
            in_importer = yaml_key(trimmed).is_some();
            active_section = None;
            continue;
        }

        if !in_importer {
            continue;
        }

        if indent == 4 {
            flush_pnpm_dependency(&mut pending, &mut dependencies);
            active_section = pnpm_dependency_section(trimmed).map(|kind| (kind, indent));
            continue;
        }

        let Some((dependency_kind, section_indent)) = active_section else {
            continue;
        };
        if indent <= section_indent {
            flush_pnpm_dependency(&mut pending, &mut dependencies);
            active_section = None;
            continue;
        }

        if indent == section_indent + 2 {
            flush_pnpm_dependency(&mut pending, &mut dependencies);
            if let Some(name) = yaml_key(trimmed) {
                pending = Some(PendingPnpmDependency {
                    name,
                    kind: dependency_kind.to_string(),
                    indent,
                    specifier: None,
                    version: None,
                });
            }
            continue;
        }

        let Some(dependency) = pending.as_mut() else {
            continue;
        };
        if indent <= dependency.indent {
            flush_pnpm_dependency(&mut pending, &mut dependencies);
            continue;
        }
        if let Some(value) = yaml_key_value(trimmed, "specifier") {
            dependency.specifier = Some(value);
        } else if let Some(value) = yaml_key_value(trimmed, "version") {
            dependency.version = Some(pnpm_clean_version(&value));
        }
    }

    flush_pnpm_dependency(&mut pending, &mut dependencies);
    dependencies
}

pub(crate) fn flush_pnpm_dependency(
    pending: &mut Option<PendingPnpmDependency>,
    dependencies: &mut Vec<ManifestDependency>,
) {
    let Some(dependency) = pending.take() else {
        return;
    };
    let (version, version_kind) = if let Some(version) = dependency.version {
        (Some(version), Some("locked"))
    } else {
        (
            dependency.specifier.filter(|value| !value.is_empty()),
            Some("constraint"),
        )
    };
    dependencies.push(manifest_dependency_with_version_kind(
        dependency.name,
        dependency.kind,
        "npm",
        version,
        version_kind,
    ));
}

pub(crate) fn pnpm_dependency_section(trimmed: &str) -> Option<&'static str> {
    match yaml_key(trimmed)?.as_str() {
        "dependencies" => Some("runtime"),
        "devDependencies" => Some("dev"),
        "peerDependencies" => Some("peer"),
        "optionalDependencies" => Some("optional"),
        _ => None,
    }
}

pub(crate) fn yaml_indent(raw_line: &str) -> usize {
    raw_line
        .chars()
        .take_while(|character| *character == ' ')
        .count()
}

pub(crate) fn yaml_key(trimmed: &str) -> Option<String> {
    let (key, _) = trimmed.split_once(':')?;
    let key = yaml_clean_scalar(key);
    (!key.is_empty()).then_some(key)
}

pub(crate) fn yaml_key_value(trimmed: &str, expected_key: &str) -> Option<String> {
    let (key, value) = trimmed.split_once(':')?;
    if yaml_clean_scalar(key) != expected_key {
        return None;
    }
    let value = yaml_clean_scalar(value);
    (!value.is_empty()).then_some(value)
}

pub(crate) fn yaml_key_pair(trimmed: &str) -> Option<(String, String)> {
    let (key, value) = trimmed.split_once(':')?;
    if value
        .chars()
        .next()
        .is_none_or(|character| !character.is_whitespace())
    {
        return None;
    }
    let key = yaml_clean_scalar(key);
    let value = yaml_clean_scalar(value);
    (!key.is_empty() && !value.is_empty()).then_some((key, value))
}

pub(crate) fn yaml_list_scalar(trimmed: &str) -> Option<String> {
    let value = trimmed.strip_prefix('-')?.trim();
    if value.is_empty() || yaml_key_pair(value).is_some() {
        None
    } else {
        Some(yaml_clean_scalar(value))
    }
}

pub(crate) fn yaml_clean_scalar(value: &str) -> String {
    value
        .split(" #")
        .next()
        .unwrap_or("")
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
}

pub(crate) fn pnpm_clean_version(value: &str) -> String {
    value.split('(').next().unwrap_or(value).trim().to_string()
}

pub(crate) fn composer_dependencies(source: &str) -> Vec<ManifestDependency> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(source) else {
        return Vec::new();
    };
    let mut dependencies = Vec::new();
    collect_json_object_keys(&value, "require", "runtime", "composer", &mut dependencies);
    collect_json_object_keys(&value, "require-dev", "dev", "composer", &mut dependencies);
    // `suggest` names packages a feature needs and the project deliberately
    // does not require: monolog suggests a dozen, one per optional handler,
    // and its own `RedisHandler` imports one of them on purpose.
    collect_suggested_packages(&value, &mut dependencies);
    dependencies.retain(|dependency| dependency.name != "php");
    dependencies
}

pub(crate) fn composer_lock_dependencies(source: &str) -> Vec<ManifestDependency> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(source) else {
        return Vec::new();
    };
    let mut dependencies = Vec::new();
    collect_composer_lock_packages(&value, "packages", "runtime", &mut dependencies);
    collect_composer_lock_packages(&value, "packages-dev", "dev", &mut dependencies);
    dependencies
}

pub(crate) fn collect_composer_lock_packages(
    value: &serde_json::Value,
    array_name: &str,
    dependency_kind: &str,
    dependencies: &mut Vec<ManifestDependency>,
) {
    let Some(packages) = value.get(array_name).and_then(|value| value.as_array()) else {
        return;
    };
    for package in packages {
        let Some(name) = package
            .get("name")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let version = package
            .get("version")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        // The lockfile states which namespaces the package autoloads, and
        // that is the only place the mapping is written down: koel imports
        // `Illuminate\\Broadcasting\\..` and declares `laravel/framework`.
        let namespaces = package
            .get("autoload")
            .and_then(|autoload| autoload.get("psr-4"))
            .and_then(|psr4| psr4.as_object())
            .map(|psr4| psr4.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        let mut dependency = manifest_dependency_with_version_kind(
            name.to_string(),
            dependency_kind,
            "composer",
            version,
            Some("locked"),
        );
        dependency.namespaces = namespaces;
        dependencies.push(dependency);
    }
}

pub(crate) fn vcpkg_dependencies(source: &str) -> Vec<ManifestDependency> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(source) else {
        return Vec::new();
    };
    let override_versions = vcpkg_override_versions(&value);
    let mut dependencies = Vec::new();
    let Some(values) = value.get("dependencies").and_then(|value| value.as_array()) else {
        return dependencies;
    };

    for value in values {
        let name = value
            .as_str()
            .map(str::to_string)
            .or_else(|| {
                value
                    .as_object()
                    .and_then(|object| object.get("name"))
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
            })
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty());
        let Some(name) = name else {
            continue;
        };
        let version = value
            .as_object()
            .and_then(vcpkg_dependency_version)
            .or_else(|| override_versions.get(&name.to_ascii_lowercase()).cloned());
        dependencies.push(manifest_dependency(name, "runtime", "vcpkg", version));
    }

    dependencies
}

pub(crate) fn vcpkg_override_versions(value: &serde_json::Value) -> BTreeMap<String, String> {
    let mut versions = BTreeMap::new();
    let Some(overrides) = value.get("overrides").and_then(|value| value.as_array()) else {
        return versions;
    };
    for override_value in overrides {
        let Some(object) = override_value.as_object() else {
            continue;
        };
        let Some(name) = object
            .get("name")
            .and_then(|value| value.as_str())
            .map(|name| name.trim().to_ascii_lowercase())
            .filter(|name| !name.is_empty())
        else {
            continue;
        };
        if let Some(version) = vcpkg_dependency_version(object) {
            versions.insert(name, version);
        }
    }
    versions
}

pub(crate) fn vcpkg_dependency_version(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Option<String> {
    [
        "version>=",
        "version",
        "version-string",
        "version-date",
        "version-semver",
    ]
    .into_iter()
    .find_map(|key| {
        object
            .get(key)
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| {
                if key == "version>=" {
                    format!(">={value}")
                } else {
                    value.to_string()
                }
            })
    })
}

pub(crate) fn conanfile_txt_dependencies(source: &str) -> Vec<ManifestDependency> {
    let mut dependencies = Vec::new();
    let mut section: Option<String> = None;
    for raw_line in source.lines() {
        let line = raw_line
            .split('#')
            .next()
            .unwrap_or("")
            .split(';')
            .next()
            .unwrap_or("")
            .trim();
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
        {
            section = Some(name.trim().to_ascii_lowercase());
            continue;
        }
        let Some(section_name) = section.as_deref() else {
            continue;
        };
        let dependency_kind = match section_name {
            "requires" => "runtime",
            "tool_requires" | "build_requires" => "build",
            "test_requires" => "test",
            _ => continue,
        };
        let Some((name, version)) = conan_reference_name_and_version(line) else {
            continue;
        };
        dependencies.push(manifest_dependency(name, dependency_kind, "conan", version));
    }
    dependencies
}

pub(crate) fn conan_reference_name_and_version(line: &str) -> Option<(String, Option<String>)> {
    let reference = line
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_matches('"')
        .trim_matches('\'')
        .trim();
    let (name, rest) = reference.split_once('/')?;
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let version = rest
        .split('@')
        .next()
        .map(str::trim)
        .filter(|version| !version.is_empty())
        .map(str::to_string);
    Some((name.to_string(), version))
}

pub(crate) fn cmake_dependencies(source: &str) -> Vec<ManifestDependency> {
    cmake_command_bodies(source, "find_package")
        .into_iter()
        .filter_map(|body| {
            let args = cmake_command_args(&body);
            cmake_find_package_dependency(&args)
        })
        .collect()
}

pub(crate) fn cmake_find_package_dependency(args: &[String]) -> Option<ManifestDependency> {
    let name = args
        .first()
        .map(String::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .filter(|name| !name.starts_with('$'))?;
    if is_cmake_find_package_option(name) {
        return None;
    }
    let version = args
        .iter()
        .skip(1)
        .find(|arg| is_cmake_version_argument(arg))
        .cloned();
    Some(manifest_dependency(
        name.to_string(),
        "runtime",
        "cmake",
        version,
    ))
}

pub(crate) fn is_cmake_version_argument(value: &str) -> bool {
    value
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_digit())
}

pub(crate) fn is_cmake_find_package_option(value: &str) -> bool {
    matches!(
        value.to_ascii_uppercase().as_str(),
        "REQUIRED"
            | "QUIET"
            | "MODULE"
            | "CONFIG"
            | "NO_MODULE"
            | "COMPONENTS"
            | "OPTIONAL_COMPONENTS"
            | "EXACT"
    )
}

pub(crate) fn go_mod_dependencies(source: &str) -> Vec<ManifestDependency> {
    let mut dependencies = Vec::new();
    let mut in_require_block = false;
    for raw_line in source.lines() {
        let dependency_kind = if raw_line.contains("// indirect") {
            "indirect"
        } else {
            "runtime"
        };
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
                dependency_kind,
                "go",
                version,
            ));
        }
    }
    dependencies
}

pub(crate) fn pubspec_dependencies(source: &str) -> Vec<ManifestDependency> {
    let mut dependencies = Vec::new();
    let mut active_section: Option<(String, usize)> = None;

    for raw_line in source.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = yaml_indent(raw_line);

        if indent == 0 {
            active_section = None;
            if let Some(section) = yaml_key(trimmed).filter(|section| {
                matches!(
                    section.as_str(),
                    "dependencies" | "dev_dependencies" | "dependency_overrides"
                )
            }) {
                active_section = Some((section, indent));
            }
            continue;
        }

        let Some((section, section_indent)) = active_section.as_ref() else {
            continue;
        };
        if indent <= *section_indent {
            active_section = None;
            continue;
        }
        if indent != section_indent + 2 {
            continue;
        }
        // `flutter_test:` followed by an indented `sdk: flutter` declares a
        // dependency whose source, not whose version, is written below it.
        // Requiring a value on the same line dropped every dependency that
        // comes from an SDK, a path, or a git remote.
        let (name, value) = match yaml_key_pair(trimmed) {
            Some((name, value)) => (name, Some(value)),
            None => match yaml_key(trimmed) {
                Some(name) => (name, None),
                None => continue,
            },
        };
        if name.is_empty() {
            continue;
        }
        let value = value.unwrap_or_default();
        let kind = match section.as_str() {
            "dependencies" => "runtime",
            "dev_dependencies" => "dev",
            "dependency_overrides" => "override",
            _ => "runtime",
        };
        let version = (!value.is_empty()
            && !matches!(
                value.as_str(),
                "{}" | "[]" | "null" | "~" | "sdk" | "path" | "git"
            ))
        .then_some(value);
        dependencies.push(manifest_dependency(name, kind, "dart", version));
    }

    dependencies
}

pub(crate) fn pubspec_flutter_assets(source: &str) -> Vec<FlutterAsset> {
    let mut assets = Vec::new();
    let mut in_flutter: Option<usize> = None;
    let mut in_assets: Option<usize> = None;

    for (index, raw_line) in source.lines().enumerate() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = yaml_indent(raw_line);

        if let Some(flutter_indent) = in_flutter
            && indent <= flutter_indent
            && yaml_key(trimmed).is_some()
        {
            in_flutter = None;
            in_assets = None;
        }
        if let Some(assets_indent) = in_assets
            && indent <= assets_indent
            && yaml_key(trimmed).is_some()
        {
            in_assets = None;
        }

        if indent == 0 && yaml_key(trimmed).is_some_and(|key| key == "flutter") {
            in_flutter = Some(indent);
            in_assets = None;
            continue;
        }
        if in_flutter.is_some() && yaml_key(trimmed).is_some_and(|key| key == "assets") {
            in_assets = Some(indent);
            continue;
        }
        if in_assets.is_some()
            && let Some(asset) = yaml_list_scalar(trimmed)
            && is_flutter_asset_path(&asset)
        {
            assets.push(FlutterAsset {
                path: asset,
                line: index as u32 + 1,
            });
        }
    }

    assets
}

pub(crate) fn is_flutter_asset_path(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('$')
        && !value.contains("://")
        && !Path::new(value).is_absolute()
}

pub(crate) fn go_module_name(source: &str) -> Option<String> {
    source.lines().find_map(|line| {
        let line = line.split("//").next().unwrap_or("").trim();
        line.strip_prefix("module ")
            .map(str::trim)
            .filter(|module| !module.is_empty())
            .map(str::to_string)
    })
}

pub(crate) fn pubspec_package_name(source: &str) -> Option<String> {
    source.lines().find_map(|line| {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            return None;
        }
        yaml_key_value(trimmed, "name").filter(|name| pubspec_package_name_valid(name))
    })
}

pub(crate) fn pubspec_package_name_valid(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|character| character == '_' || character.is_ascii_alphanumeric())
}

pub(crate) fn pubspec_uses_flutter(source: &str) -> bool {
    source.lines().any(|line| {
        let trimmed = line.trim();
        trimmed == "flutter:" || trimmed.starts_with("sdk: flutter")
    })
}

/// The interpreter's own name, kept static so the entrypoint metadata
/// names what runs the file rather than the language it is written in.
fn interpreter_name(interpreter: &str) -> &'static str {
    match interpreter {
        "luajit" => "luajit",
        "resty" => "resty",
        _ => "lua",
    }
}

pub(crate) fn shebang_interpreter(source: &str) -> Option<(&'static str, &'static str)> {
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
        "dash" => Some(("dash", "bash")),
        "python" | "python2" | "python3" => Some(("python", "python")),
        "node" | "nodejs" => Some(("node", "javascript")),
        "php" => Some(("php", "php")),
        // A file with no extension states its language in its first line,
        // and these were not read: mastodon keeps thirteen ruby programs in
        // `bin/`, and kong's `bin/kong` -- the gateway's whole CLI -- runs
        // under OpenResty's lua.
        "ruby" => Some(("ruby", "ruby")),
        "lua" | "luajit" | "resty" => Some((interpreter_name(interpreter), "lua")),
        "ocaml" => Some(("ocaml", "ocaml")),
        "elixir" => Some(("elixir", "elixir")),
        "julia" => Some(("julia", "julia")),
        "Rscript" => Some(("Rscript", "r")),
        _ => None,
    }
}

pub(crate) fn shebang_language(source: &str) -> Option<Language> {
    match shebang_interpreter(source)?.1 {
        "bash" => Some(Language::Bash),
        "python" => Some(Language::Python),
        "javascript" => Some(Language::JavaScript),
        "php" => Some(Language::Php),
        "ruby" => Some(Language::Ruby),
        "lua" => Some(Language::Lua),
        "ocaml" => Some(Language::OCaml),
        "elixir" => Some(Language::Elixir),
        "julia" => Some(Language::Julia),
        "r" => Some(Language::R),
        _ => None,
    }
}

pub(crate) fn requirements_dependencies(source: &str) -> Vec<ManifestDependency> {
    // A compiled requirements file is a lockfile: it states the versions
    // one installation resolved to, not what the project asks for. flask's
    // `examples/celery/requirements.txt` pins `flask==2.3.2` along with
    // the `blinker==1.6.2` that release wanted, and reading those as the
    // project's own constraints put them against `pyproject.toml`.
    let compiled = is_a_compiled_requirements_file(source);
    source
        .lines()
        .filter_map(|line| {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() || line.starts_with('-') {
                return None;
            }
            package_name_and_version_from_requirement(line).map(|(name, version)| {
                manifest_dependency_with_version_kind(
                    name,
                    "runtime",
                    "python",
                    version,
                    Some(if compiled { "locked" } else { "constraint" }),
                )
            })
        })
        .collect()
}

/// Whether a requirements file was written by a tool that resolved it.
/// Both `pip-compile` and `uv` say so in the header they write.
fn is_a_compiled_requirements_file(source: &str) -> bool {
    source
        .lines()
        .take(8)
        .filter(|line| line.trim_start().starts_with('#'))
        .any(|line| {
            let lowered = line.to_ascii_lowercase();
            lowered.contains("autogenerated by pip-compile")
                || lowered.contains("generated by pip-compile")
                || lowered.contains("generated by uv")
        })
}

pub(crate) fn setup_py_sequence_string_values(source: &str, key: &str) -> Vec<String> {
    let Some(value) = setup_py_keyword_value(source, key) else {
        return Vec::new();
    };
    extract_python_quoted_strings(&value)
}

pub(crate) fn setup_py_dict_list_string_values(source: &str, key: &str) -> Vec<String> {
    let Some(value) = setup_py_keyword_value(source, key) else {
        return Vec::new();
    };
    python_dict_list_values(&value)
        .into_iter()
        .flat_map(|value| extract_python_quoted_strings(&value))
        .collect()
}

pub(crate) fn setup_py_console_scripts(source: &str) -> Vec<String> {
    let Some(value) = setup_py_keyword_value(source, "entry_points") else {
        return Vec::new();
    };
    python_dict_list_values_for_key(&value, "console_scripts")
        .into_iter()
        .flat_map(|value| extract_python_quoted_strings(&value))
        .collect()
}

pub(crate) fn setup_cfg_sections(source: &str) -> BTreeMap<String, BTreeMap<String, Vec<String>>> {
    let mut sections: BTreeMap<String, BTreeMap<String, Vec<String>>> = BTreeMap::new();
    let mut current_section: Option<String> = None;
    let mut current_key: Option<String> = None;

    for raw_line in source.lines() {
        let is_continuation = raw_line.starts_with([' ', '\t']);
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        if let Some(section) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
        {
            let section = section.trim().to_ascii_lowercase();
            if section.is_empty() {
                current_section = None;
            } else {
                sections.entry(section.clone()).or_default();
                current_section = Some(section);
            }
            current_key = None;
            continue;
        }

        let Some(section) = current_section.as_deref() else {
            continue;
        };

        if is_continuation {
            let Some(key) = current_key.as_deref() else {
                continue;
            };
            let value = setup_cfg_clean_value(line);
            if !value.is_empty() {
                sections
                    .entry(section.to_string())
                    .or_default()
                    .entry(key.to_string())
                    .or_default()
                    .push(value);
            }
            continue;
        }

        let Some((key, value)) = line.split_once('=').or_else(|| line.split_once(':')) else {
            current_key = None;
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        if key.is_empty() {
            current_key = None;
            continue;
        }
        sections
            .entry(section.to_string())
            .or_default()
            .entry(key.clone())
            .or_default();
        current_key = Some(key.clone());
        let value = setup_cfg_clean_value(value);
        if !value.is_empty() {
            sections
                .entry(section.to_string())
                .or_default()
                .entry(key)
                .or_default()
                .push(value);
        }
    }

    sections
}

pub(crate) fn setup_cfg_values(
    sections: &BTreeMap<String, BTreeMap<String, Vec<String>>>,
    section: &str,
    key: &str,
) -> Vec<String> {
    sections
        .get(&section.to_ascii_lowercase())
        .and_then(|values| values.get(&key.to_ascii_lowercase()))
        .cloned()
        .unwrap_or_default()
}

pub(crate) fn setup_cfg_clean_value(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
}

pub(crate) fn setup_py_keyword_value(source: &str, key: &str) -> Option<String> {
    let lower = source.to_ascii_lowercase();
    let key_lower = key.to_ascii_lowercase();
    let mut search_start = 0;
    while let Some(offset) = lower[search_start..].find(&key_lower) {
        let key_start = search_start + offset;
        let key_end = key_start + key.len();
        if !is_python_identifier_boundary(source, key_start, key_end) {
            search_start = key_end;
            continue;
        }
        let mut cursor = skip_ascii_whitespace(source, key_end);
        if !source[cursor..].starts_with('=') {
            search_start = key_end;
            continue;
        }
        cursor = skip_ascii_whitespace(source, cursor + 1);
        return python_value_literal_at(source, cursor);
    }
    None
}

pub(crate) fn is_python_identifier_boundary(source: &str, start: usize, end: usize) -> bool {
    let before = source[..start].chars().next_back();
    let after = source[end..].chars().next();
    before.is_none_or(|character| !is_python_identifier_character(character))
        && after.is_none_or(|character| !is_python_identifier_character(character))
}

pub(crate) fn is_python_identifier_character(character: char) -> bool {
    character == '_' || character.is_ascii_alphanumeric()
}

pub(crate) fn python_value_literal_at(source: &str, start: usize) -> Option<String> {
    let first = source[start..].chars().next()?;
    if matches!(first, '[' | '(' | '{') {
        return balanced_python_delimited_value(source, start);
    }
    if matches!(first, '"' | '\'') {
        let quoted = extract_python_quoted_string_at(source, start)?;
        return Some(quoted.raw);
    }
    let end = source[start..]
        .find([',', '\n'])
        .map(|offset| start + offset)
        .unwrap_or(source.len());
    Some(source[start..end].trim().to_string()).filter(|value| !value.is_empty())
}

pub(crate) fn balanced_python_delimited_value(source: &str, start: usize) -> Option<String> {
    let open = source[start..].chars().next()?;
    let close = match open {
        '[' => ']',
        '(' => ')',
        '{' => '}',
        _ => return None,
    };
    let mut depth = 0usize;
    let mut quote = None;
    let mut escaped = false;
    for (relative, character) in source[start..].char_indices() {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
                continue;
            }
            if character == '\\' {
                escaped = true;
                continue;
            }
            if character == active_quote {
                quote = None;
            }
            continue;
        }
        if matches!(character, '"' | '\'') {
            quote = Some(character);
            continue;
        }
        if character == open {
            depth += 1;
        } else if character == close {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                let end = start + relative + character.len_utf8();
                return Some(source[start..end].to_string());
            }
        }
    }
    None
}

#[derive(Debug)]
pub(crate) struct PythonQuotedString {
    raw: String,
    value: String,
    end: usize,
}

pub(crate) fn extract_python_quoted_strings(source: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut cursor = 0;
    while cursor < source.len() {
        if let Some(relative) = source[cursor..].find(['"', '\'']) {
            let start = cursor + relative;
            if let Some(quoted) = extract_python_quoted_string_at(source, start) {
                values.push(quoted.value);
                cursor = quoted.end;
                continue;
            }
            cursor = start + 1;
        } else {
            break;
        }
    }
    values
}

pub(crate) fn extract_python_quoted_string_at(
    source: &str,
    start: usize,
) -> Option<PythonQuotedString> {
    let quote = source[start..].chars().next()?;
    if !matches!(quote, '"' | '\'') {
        return None;
    }
    let mut escaped = false;
    let mut value = String::new();
    for (relative, character) in source[start + quote.len_utf8()..].char_indices() {
        if escaped {
            value.push(character);
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if character == quote {
            let end = start + quote.len_utf8() + relative + character.len_utf8();
            return Some(PythonQuotedString {
                raw: source[start..end].to_string(),
                value,
                end,
            });
        }
        value.push(character);
    }
    None
}

pub(crate) fn python_dict_list_values(source: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut cursor = 0;
    while cursor < source.len() {
        let Some(relative) = source[cursor..].find(['[', '(']) else {
            break;
        };
        let start = cursor + relative;
        if let Some(value) = balanced_python_delimited_value(source, start) {
            cursor = start + value.len();
            values.push(value);
        } else {
            cursor = start + 1;
        }
    }
    values
}

pub(crate) fn python_dict_list_values_for_key(source: &str, wanted_key: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut cursor = 0;
    while cursor < source.len() {
        let Some(relative) = source[cursor..].find(['"', '\'']) else {
            break;
        };
        let key_start = cursor + relative;
        let Some(key) = extract_python_quoted_string_at(source, key_start) else {
            cursor = key_start + 1;
            continue;
        };
        let mut after_key = skip_ascii_whitespace(source, key.end);
        if !source[after_key..].starts_with(':') {
            cursor = key.end;
            continue;
        }
        after_key = skip_ascii_whitespace(source, after_key + 1);
        if key.value == wanted_key
            && let Some(value) = python_value_literal_at(source, after_key)
        {
            values.push(value);
        }
        cursor = after_key.saturating_add(1);
    }
    values
}

pub(crate) fn python_console_script_name_and_target(value: &str) -> Option<(String, String)> {
    let (name, target) = value.split_once('=')?;
    let name = name.trim();
    let target = target.trim();
    if name.is_empty() || target.is_empty() {
        None
    } else {
        Some((name.to_string(), target.to_string()))
    }
}

pub(crate) fn skip_ascii_whitespace(value: &str, mut cursor: usize) -> usize {
    while cursor < value.len()
        && value[cursor..]
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_whitespace())
    {
        cursor += value[cursor..].chars().next().unwrap().len_utf8();
    }
    cursor
}

pub(crate) fn collect_toml_table_keys(
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
        let version = dependency_version_from_toml_value(
            name,
            value,
            ecosystem,
            cargo_workspace_dependencies,
        );
        // The key is the name the code writes: ripgrep declares `memmap =
        // { package = "memmap2" }` and its searcher writes `use
        // memmap::Mmap`, which read as a dependency the project never
        // declared. The registry name travels beside it.
        let renamed = value
            .as_table()
            .and_then(|table| table.get("package"))
            .and_then(|value| value.as_str())
            .filter(|package| *package != name);
        dependencies.push(manifest_dependency(
            name.to_string(),
            dependency_kind,
            ecosystem,
            version.clone(),
        ));
        if let Some(package) = renamed {
            dependencies.push(manifest_dependency(
                package.to_string(),
                dependency_kind,
                ecosystem,
                version,
            ));
        }
    }
}

pub(crate) fn collect_toml_entrypoint_keys(
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

/// Composer's `suggest` maps a package to a sentence telling the reader why
/// they might install it, not to a version. The name is the fact; reading
/// the sentence as a constraint made monolog disagree with itself about
/// eight packages.
fn collect_suggested_packages(
    value: &serde_json::Value,
    dependencies: &mut Vec<ManifestDependency>,
) {
    let Some(object) = value.get("suggest").and_then(|value| value.as_object()) else {
        return;
    };
    for name in object.keys() {
        dependencies.push(manifest_dependency(
            name.clone(),
            "optional",
            "composer",
            None,
        ));
    }
}

pub(crate) fn collect_json_object_keys(
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

pub(crate) fn cargo_workspace_dependencies(root: &Path) -> BTreeMap<String, Option<String>> {
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

pub(crate) fn go_module_roots(
    root: &Path,
    options: &IndexOptions,
    ignored_globs: &Option<GlobSet>,
) -> Vec<GoModuleRoot> {
    let mut modules = Vec::new();
    for entry in WalkDir::new(root)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| should_enter(entry, root, options, ignored_globs))
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path == root || !entry.file_type().is_file() {
            continue;
        }
        if path.file_name().and_then(|name| name.to_str()) != Some("go.mod")
            || !is_probably_source_file(path, options.max_file_size)
        {
            continue;
        }

        let Ok(source) = fs::read_to_string(path) else {
            continue;
        };
        let Some(module) = go_module_name(&source) else {
            continue;
        };
        let dir = path
            .parent()
            .and_then(|parent| parent.strip_prefix(root).ok())
            .map(|relative| relative.to_string_lossy().replace('\\', "/"))
            .map(|relative| normalize_path(&relative))
            .filter(|relative| !relative.is_empty());
        modules.push(GoModuleRoot { module, dir });
    }
    modules.sort_by(|left, right| {
        right
            .module
            .len()
            .cmp(&left.module.len())
            .then_with(|| left.dir.cmp(&right.dir))
    });
    modules
}

/// Every package.json in the repository that names a package, so a
/// workspace import can be told from a dependency. Longest name first, so
/// `@scope/a-b` wins over `@scope/a` when both could prefix-match.
/// What each `tsconfig.json` says an import prefix stands for. Bundlers
/// read the same file, and without it koel's `@/utils` looks like a
/// package nobody declared rather than the directory beside it.
pub(crate) fn typescript_path_aliases(
    root: &Path,
    options: &IndexOptions,
    ignored_globs: &Option<GlobSet>,
) -> Vec<PathAlias> {
    let mut aliases: Vec<PathAlias> = Vec::new();
    for entry in WalkDir::new(root)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| should_enter(entry, root, options, ignored_globs))
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if !entry.file_type().is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !(name.starts_with("tsconfig") || name.starts_with("jsconfig"))
            || !name.ends_with(".json")
        {
            continue;
        }
        let Ok(source) = fs::read_to_string(path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&strip_json_comments(&source))
        else {
            continue;
        };
        let options_value = value.get("compilerOptions");
        let paths = options_value
            .and_then(|options| options.get("paths"))
            .and_then(|paths| paths.as_object());
        let directory = path
            .parent()
            .and_then(|parent| parent.strip_prefix(root).ok())
            .map(|relative| normalize_path(&relative.to_string_lossy().replace('\\', "/")))
            .unwrap_or_default();
        let base = options_value
            .and_then(|options| options.get("baseUrl"))
            .and_then(|base| base.as_str())
            .map(|base| join_path(Some(&directory), base))
            .unwrap_or(directory);
        // `baseUrl` alone makes every directory under it importable by
        // name: taxonomy writes `import { User } from "types"` in eleven
        // files, and that is the `types/` directory beside its tsconfig.
        // Only the directories that are really there become aliases, so a
        // package name still reads as a package.
        if options_value
            .and_then(|options| options.get("baseUrl"))
            .and_then(|base| base.as_str())
            .is_some()
        {
            let base_directory = path.parent().map(|parent| parent.join(&base));
            if let Some(entries) = base_directory.and_then(|base| fs::read_dir(base).ok()) {
                for entry in entries.filter_map(Result::ok) {
                    if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                        continue;
                    }
                    let Some(name) = entry.file_name().to_str().map(ToString::to_string) else {
                        continue;
                    };
                    if name.starts_with('.') || name == "node_modules" {
                        continue;
                    }
                    let target = join_path(Some(&base), &name);
                    if let Some(existing) = aliases.iter_mut().find(|alias| alias.prefix == name) {
                        if !existing.targets.contains(&target) {
                            existing.targets.push(target);
                        }
                    } else {
                        aliases.push(PathAlias {
                            prefix: name,
                            targets: vec![target],
                        });
                    }
                }
            }
        }
        for (pattern, targets) in paths.into_iter().flatten() {
            // Only the `prefix/*` form names a directory; an exact mapping
            // names one module, which the same substitution covers.
            let Some(prefix) = pattern.strip_suffix('*') else {
                continue;
            };
            let targets: Vec<String> = targets
                .as_array()
                .map(|targets| {
                    targets
                        .iter()
                        .filter_map(|target| target.as_str())
                        .filter_map(|target| target.strip_suffix('*'))
                        .map(|target| join_path(Some(&base), target))
                        .collect()
                })
                .unwrap_or_default();
            if prefix.is_empty() || targets.is_empty() {
                continue;
            }
            if let Some(existing) = aliases.iter_mut().find(|alias| alias.prefix == *prefix) {
                existing.targets.extend(targets);
                existing.targets.dedup();
            } else {
                aliases.push(PathAlias {
                    prefix: prefix.to_string(),
                    targets,
                });
            }
        }
    }
    // The longest prefix wins, as it does in TypeScript.
    aliases.sort_by(|left, right| {
        right
            .prefix
            .len()
            .cmp(&left.prefix.len())
            .then_with(|| left.prefix.cmp(&right.prefix))
    });
    aliases
}

/// A `tsconfig.json` is JSON with comments, which serde will not read.
fn strip_json_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut in_string = false;
    let mut escaped = false;
    let mut chars = source.chars().peekable();
    while let Some(character) = chars.next() {
        if in_string {
            out.push(character);
            match character {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match character {
            '"' => {
                in_string = true;
                out.push(character);
            }
            '/' if chars.peek() == Some(&'/') => {
                for next in chars.by_ref() {
                    if next == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut previous = ' ';
                for next in chars.by_ref() {
                    if previous == '*' && next == '/' {
                        break;
                    }
                    if next == '\n' {
                        out.push('\n');
                    }
                    previous = next;
                }
            }
            _ => out.push(character),
        }
    }
    // A trailing comma is legal in a tsconfig and not in JSON, and what
    // follows it is usually a newline and an indent.
    let mut cleaned = String::with_capacity(out.len());
    let mut held = String::new();
    for character in out.chars() {
        if !held.is_empty() {
            if character.is_whitespace() {
                held.push(character);
                continue;
            }
            if !matches!(character, '}' | ']') {
                cleaned.push_str(&held);
            } else {
                // Keep the layout, drop the comma.
                cleaned.push_str(&held[1..]);
            }
            held.clear();
        }
        if character == ',' {
            held.push(character);
            continue;
        }
        cleaned.push(character);
    }
    cleaned.push_str(&held);
    cleaned
}

pub(crate) fn npm_package_roots(
    root: &Path,
    options: &IndexOptions,
    ignored_globs: &Option<GlobSet>,
) -> Vec<NpmPackageRoot> {
    let mut packages = Vec::new();
    for entry in WalkDir::new(root)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| should_enter(entry, root, options, ignored_globs))
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path == root || !entry.file_type().is_file() {
            continue;
        }
        if path.file_name().and_then(|name| name.to_str()) != Some("package.json")
            || !is_probably_source_file(path, options.max_file_size)
        {
            continue;
        }

        let Ok(source) = fs::read_to_string(path) else {
            continue;
        };
        let Some(name) = package_json_name(&source) else {
            continue;
        };
        let dir = path
            .parent()
            .and_then(|parent| parent.strip_prefix(root).ok())
            .map(|relative| relative.to_string_lossy().replace('\\', "/"))
            .map(|relative| normalize_path(&relative))
            .filter(|relative| !relative.is_empty());
        packages.push(NpmPackageRoot { name, dir });
    }
    packages.sort_by(|left, right| {
        right
            .name
            .len()
            .cmp(&left.name.len())
            .then_with(|| left.dir.cmp(&right.dir))
    });
    packages
}

fn package_json_name(source: &str) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(source).ok()?;
    let name = value.get("name")?.as_str()?.trim();
    (!name.is_empty()).then(|| name.to_string())
}

pub(crate) fn dart_package_roots(
    root: &Path,
    options: &IndexOptions,
    ignored_globs: &Option<GlobSet>,
) -> Vec<DartPackageRoot> {
    let mut packages = Vec::new();
    for entry in WalkDir::new(root)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| should_enter(entry, root, options, ignored_globs))
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path == root || !entry.file_type().is_file() {
            continue;
        }
        if path.file_name().and_then(|name| name.to_str()) != Some("pubspec.yaml")
            || !is_probably_source_file(path, options.max_file_size)
        {
            continue;
        }

        let Ok(source) = fs::read_to_string(path) else {
            continue;
        };
        let Some(name) = pubspec_package_name(&source) else {
            continue;
        };
        let dir = path
            .parent()
            .and_then(|parent| parent.strip_prefix(root).ok())
            .map(|relative| relative.to_string_lossy().replace('\\', "/"))
            .map(|relative| normalize_path(&relative))
            .filter(|relative| !relative.is_empty());
        packages.push(DartPackageRoot {
            name,
            dir,
            lib_dir: "lib".to_string(),
        });
    }
    let mut probe_dirs: Vec<Option<String>> =
        packages.iter().map(|package| package.dir.clone()).collect();
    probe_dirs.push(None);
    probe_dirs.sort();
    probe_dirs.dedup();
    for extra in dart_package_config_roots(root, &probe_dirs) {
        if !packages.iter().any(|package| package.name == extra.name) {
            packages.push(extra);
        }
    }
    packages.sort_by_key(|package| std::cmp::Reverse(package.name.len()));
    packages
}

/// Packages from `.dart_tool/package_config.json` next to each pubspec (and
/// the scan root). This resolves `package:` imports for path dependencies
/// and workspace monorepos that a single `pubspec.yaml` cannot describe.
/// Only workspace-relative `rootUri` values are used; absolute URIs point at
/// the pub cache outside the scanned tree.
pub(crate) fn dart_package_config_roots(
    root: &Path,
    base_dirs: &[Option<String>],
) -> Vec<DartPackageRoot> {
    let mut packages = Vec::new();
    for base in base_dirs {
        let config_dir = match base {
            Some(dir) => format!("{dir}/.dart_tool"),
            None => ".dart_tool".to_string(),
        };
        let config_path = root.join(&config_dir).join("package_config.json");
        let Ok(raw) = fs::read_to_string(&config_path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
            continue;
        };
        let Some(entries) = value.get("packages").and_then(|value| value.as_array()) else {
            continue;
        };
        for entry in entries {
            let Some(name) = entry.get("name").and_then(|value| value.as_str()) else {
                continue;
            };
            let Some(root_uri) = entry.get("rootUri").and_then(|value| value.as_str()) else {
                continue;
            };
            let package_uri = entry
                .get("packageUri")
                .and_then(|value| value.as_str())
                .unwrap_or("lib/");
            let Some(dir) = resolve_dart_root_uri(&config_dir, root_uri) else {
                continue;
            };
            let lib_dir = package_uri.trim_matches('/');
            packages.push(DartPackageRoot {
                name: name.to_string(),
                dir,
                lib_dir: if lib_dir.is_empty() {
                    "lib".to_string()
                } else {
                    lib_dir.to_string()
                },
            });
        }
    }
    packages
}

/// Resolve a package_config `rootUri` against the `.dart_tool` directory it
/// lives in. Returns the scan-root-relative directory, or `None` when the
/// URI is absolute or escapes the scanned workspace.
pub(crate) fn resolve_dart_root_uri(config_dir: &str, root_uri: &str) -> Option<Option<String>> {
    let uri = root_uri.strip_prefix("file://").unwrap_or(root_uri);
    if uri.starts_with('/') || uri.contains("://") {
        return None;
    }
    let combined = format!("{config_dir}/{uri}").replace('\\', "/");
    let mut parts: Vec<&str> = Vec::new();
    for part in combined.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            value => parts.push(value),
        }
    }
    let dir = parts.join("/");
    Some(if dir.is_empty() { None } else { Some(dir) })
}

pub(crate) fn c_include_dirs(
    root: &Path,
    options: &IndexOptions,
    ignored_globs: &Option<GlobSet>,
) -> Vec<String> {
    let mut dirs = cmake_include_dirs(root, options, ignored_globs);
    dirs.extend(compile_commands_include_dirs(root, options, ignored_globs));
    dedup_preserving_order(&mut dirs);
    dirs
}

pub(crate) fn cmake_include_dirs(
    root: &Path,
    options: &IndexOptions,
    ignored_globs: &Option<GlobSet>,
) -> Vec<String> {
    let mut dirs = Vec::new();
    for entry in WalkDir::new(root)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| should_enter(entry, root, options, ignored_globs))
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path == root || !entry.file_type().is_file() {
            continue;
        }
        if path.file_name().and_then(|name| name.to_str()) != Some("CMakeLists.txt")
            || !is_probably_source_file(path, options.max_file_size)
        {
            continue;
        }

        let Ok(source) = fs::read_to_string(path) else {
            continue;
        };
        let base = path
            .parent()
            .and_then(|parent| parent.strip_prefix(root).ok())
            .map(|relative| relative.to_string_lossy().replace('\\', "/"))
            .map(|relative| normalize_path(&relative))
            .filter(|relative| !relative.is_empty());
        dirs.extend(cmake_include_dirs_from_source(base.as_deref(), &source));
    }
    dedup_preserving_order(&mut dirs);
    dirs
}

pub(crate) fn compile_commands_include_dirs(
    root: &Path,
    options: &IndexOptions,
    ignored_globs: &Option<GlobSet>,
) -> Vec<String> {
    let mut dirs = Vec::new();
    for entry in WalkDir::new(root)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| should_enter(entry, root, options, ignored_globs))
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path == root || !entry.file_type().is_file() {
            continue;
        }
        if path.file_name().and_then(|name| name.to_str()) != Some("compile_commands.json")
            || !is_probably_source_file(path, options.max_file_size)
        {
            continue;
        }

        let Ok(source) = fs::read_to_string(path) else {
            continue;
        };
        let base = path
            .parent()
            .and_then(|parent| parent.strip_prefix(root).ok())
            .map(|relative| relative.to_string_lossy().replace('\\', "/"))
            .map(|relative| normalize_path(&relative))
            .filter(|relative| !relative.is_empty());
        dirs.extend(compile_commands_include_dirs_from_source(
            root,
            base.as_deref(),
            &source,
        ));
    }
    dedup_preserving_order(&mut dirs);
    dirs
}

pub(crate) fn compile_commands_include_dirs_from_source(
    root: &Path,
    base: Option<&str>,
    source: &str,
) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(source) else {
        return Vec::new();
    };
    let Some(commands) = value.as_array() else {
        return Vec::new();
    };

    let mut dirs = Vec::new();
    for command in commands {
        let command_base = compile_command_base(root, base, command);
        if let Some(arguments) = command.get("arguments").and_then(|value| value.as_array()) {
            let args = arguments
                .iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .collect::<Vec<_>>();
            dirs.extend(include_dirs_from_compiler_args(
                command_base.as_deref(),
                &args,
            ));
        } else if let Some(command_line) = command.get("command").and_then(|value| value.as_str()) {
            let args = split_command_tokens(command_line);
            dirs.extend(include_dirs_from_compiler_args(
                command_base.as_deref(),
                &args,
            ));
        }
    }
    dedup_preserving_order(&mut dirs);
    dirs
}

pub(crate) fn compile_command_base(
    root: &Path,
    base: Option<&str>,
    command: &serde_json::Value,
) -> Option<String> {
    command
        .get("directory")
        .and_then(|value| value.as_str())
        .and_then(|directory| normalize_compile_command_directory(root, base, directory))
        .or_else(|| base.map(str::to_string))
}

pub(crate) fn normalize_compile_command_directory(
    root: &Path,
    base: Option<&str>,
    directory: &str,
) -> Option<String> {
    let value = directory.trim();
    if value.is_empty() {
        return base.map(str::to_string);
    }
    let path = Path::new(value);
    if path.is_absolute() {
        return path
            .strip_prefix(root)
            .ok()
            .map(|relative| relative.to_string_lossy().replace('\\', "/"))
            .map(|relative| normalize_path(&relative))
            .map(|relative| {
                if relative.is_empty() {
                    ".".to_string()
                } else {
                    relative
                }
            });
    }
    Some(join_path(base, value))
}

pub(crate) fn include_dirs_from_compiler_args(base: Option<&str>, args: &[String]) -> Vec<String> {
    let mut dirs = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].trim();
        let mut consumed_next = false;
        let candidate = if let Some(rest) = arg.strip_prefix("-I") {
            if rest.is_empty() {
                consumed_next = true;
                args.get(index + 1).map(String::as_str)
            } else {
                Some(rest)
            }
        } else if let Some(rest) = arg.strip_prefix("-isystem") {
            if rest.is_empty() {
                consumed_next = true;
                args.get(index + 1).map(String::as_str)
            } else {
                Some(rest)
            }
        } else if let Some(rest) = arg.strip_prefix("-iquote") {
            if rest.is_empty() {
                consumed_next = true;
                args.get(index + 1).map(String::as_str)
            } else {
                Some(rest)
            }
        } else if matches!(arg, "/I" | "-idirafter") {
            consumed_next = true;
            args.get(index + 1).map(String::as_str)
        } else if arg.starts_with("/I") {
            arg.strip_prefix("/I").filter(|rest| !rest.is_empty())
        } else {
            None
        };

        if let Some(candidate) = candidate.and_then(|value| compiler_include_dir_arg(base, value)) {
            dirs.push(candidate);
        }
        index += if consumed_next { 2 } else { 1 };
    }
    dedup_preserving_order(&mut dirs);
    dirs
}

pub(crate) fn compiler_include_dir_arg(base: Option<&str>, arg: &str) -> Option<String> {
    let value = arg.trim().trim_matches(['"', '\'']);
    if value.is_empty() || value.starts_with('$') || value.starts_with('<') {
        return None;
    }
    if Path::new(value).is_absolute() {
        return None;
    }
    let path = join_path(base, value);
    if path.is_empty() { None } else { Some(path) }
}

pub(crate) fn cmake_include_dirs_from_source(base: Option<&str>, source: &str) -> Vec<String> {
    let mut dirs = Vec::new();
    for body in cmake_command_bodies(source, "include_directories") {
        for arg in cmake_command_args(&body) {
            if let Some(dir) = cmake_include_dir_arg(base, &arg) {
                dirs.push(dir);
            }
        }
    }
    for body in cmake_command_bodies(source, "target_include_directories") {
        for arg in cmake_command_args(&body).into_iter().skip(1) {
            if is_cmake_include_scope_or_option(&arg) {
                continue;
            }
            if let Some(dir) = cmake_include_dir_arg(base, &arg) {
                dirs.push(dir);
            }
        }
    }
    dedup_preserving_order(&mut dirs);
    dirs
}

pub(crate) fn cmake_include_dir_arg(base: Option<&str>, arg: &str) -> Option<String> {
    let mut value = arg.trim().trim_matches(['"', '\'']).to_string();
    if value.is_empty()
        || value.starts_with('/')
        || value.starts_with('$') && !value.starts_with("${")
        || value.starts_with("$<")
        || is_cmake_include_scope_or_option(&value)
    {
        return None;
    }

    let current_dir = base.unwrap_or(".");
    let root_relative =
        value.contains("${PROJECT_SOURCE_DIR}") || value.contains("${CMAKE_SOURCE_DIR}");
    value = value
        .replace("${CMAKE_CURRENT_SOURCE_DIR}", current_dir)
        .replace("${CMAKE_CURRENT_LIST_DIR}", current_dir)
        .replace("${PROJECT_SOURCE_DIR}", ".")
        .replace("${CMAKE_SOURCE_DIR}", ".");
    if value.contains('$') || value.starts_with('/') {
        return None;
    }

    let path = if value == "." {
        if root_relative {
            ".".to_string()
        } else {
            current_dir.to_string()
        }
    } else if root_relative {
        normalize_path(&value)
    } else {
        join_path(base, &value)
    };
    if path.is_empty() { None } else { Some(path) }
}

pub(crate) fn is_cmake_include_scope_or_option(value: &str) -> bool {
    matches!(
        value.to_ascii_uppercase().as_str(),
        "PUBLIC" | "PRIVATE" | "INTERFACE" | "SYSTEM" | "BEFORE" | "AFTER"
    )
}

pub(crate) fn dependency_version_from_toml_value(
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

pub(crate) fn direct_toml_dependency_version(value: &toml::Value) -> Option<String> {
    if let Some(version) = value.as_str() {
        return Some(version.to_string());
    }
    let table = value.as_table()?;
    table
        .get("version")
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

pub(crate) fn pipfile_dependency_version(value: &toml::Value) -> Option<String> {
    let version = direct_toml_dependency_version(value)?;
    let version = version.trim();
    if version.is_empty() || version == "*" {
        None
    } else {
        Some(version.to_string())
    }
}

pub(crate) fn package_name_and_version_from_requirement(
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
        return None;
    }
    // `celery[redis]==5.2.7` asks for celery with its redis extra, at that
    // version. The extras name optional parts to install, so reading them
    // as the version leaves `celery` pinned to `[redis]`.
    let mut rest = trimmed[end..].trim_start();
    if let Some(extras) = rest.strip_prefix('[')
        && let Some((_, tail)) = extras.split_once(']')
    {
        rest = tail.trim_start();
    }
    let version = rest.trim();
    Some((
        name.to_string(),
        (!version.is_empty()).then(|| version.to_string()),
    ))
}

/// Candidate package hub ids for a source import label, used to link code
/// imports to manifest package hubs where package identity is stable.
/// Go imports are matched by hub-prefix in the resolution pass instead.
pub(crate) fn import_package_id_candidates(language: &str, label: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    match language {
        "rust" => {
            let Some(rest) = label.trim().strip_prefix("use ") else {
                return candidates;
            };
            let Some(segment) = rest
                .split([':', ';', '{', ' ', '\n', '\t'])
                .find(|part| !part.is_empty())
            else {
                return candidates;
            };
            if matches!(
                segment,
                "std" | "core" | "alloc" | "crate" | "self" | "super"
            ) {
                return candidates;
            }
            let lower = segment.to_ascii_lowercase();
            candidates.push(package_id("cargo", &lower));
            let dashed = lower.replace('_', "-");
            if dashed != lower {
                candidates.push(package_id("cargo", &dashed));
            }
        }
        "javascript" | "typescript" | "tsx" => {
            let Some(module) = first_quoted_value(label) else {
                return candidates;
            };
            if module.starts_with('.') || module.starts_with('/') || module.starts_with("node:") {
                return candidates;
            }
            let mut segments = module.split('/');
            let package = if module.starts_with('@') {
                match (segments.next(), segments.next()) {
                    (Some(scope), Some(name)) => format!("{scope}/{name}"),
                    _ => return candidates,
                }
            } else {
                match segments.next() {
                    Some(name) if !name.is_empty() => name.to_string(),
                    _ => return candidates,
                }
            };
            candidates.push(package_id("npm", &package.to_ascii_lowercase()));
        }
        "python" => {
            let value = label.trim();
            let module = if let Some(rest) = value.strip_prefix("from ") {
                rest.split_whitespace().next()
            } else if let Some(rest) = value.strip_prefix("import ") {
                rest.split([',', ' ', '.']).find(|part| !part.is_empty())
            } else {
                None
            };
            let Some(module) = module else {
                return candidates;
            };
            if module.starts_with('.') {
                return candidates;
            }
            let root = module.split('.').next().unwrap_or(module);
            candidates.push(package_id(
                "python",
                &canonical_package_name("python", root),
            ));
        }
        "php" => {
            let Some(rest) = label.trim().strip_prefix("use ") else {
                return candidates;
            };
            let parts: Vec<&str> = rest
                .trim_end_matches(';')
                .split('\\')
                .filter(|part| !part.is_empty())
                .collect();
            if parts.len() >= 2 {
                candidates.push(package_id(
                    "composer",
                    &format!(
                        "{}/{}",
                        parts[0].to_ascii_lowercase(),
                        parts[1].to_ascii_lowercase()
                    ),
                ));
            }
        }
        "dart" => {
            let Some(uri) = first_quoted_value(label) else {
                return candidates;
            };
            if let Some(rest) = uri.strip_prefix("package:")
                && let Some(name) = rest.split('/').next()
                && !name.is_empty()
            {
                candidates.push(package_id("dart", &name.to_ascii_lowercase()));
            }
        }
        _ => {}
    }
    candidates
}

pub(crate) fn package_id(ecosystem: &str, package_name: &str) -> String {
    format!("{ecosystem}:{package_name}")
}

pub(crate) fn canonical_package_name(ecosystem: &str, name: &str) -> String {
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
        "cargo" | "npm" | "composer" | "vcpkg" | "conan" | "cmake" | "dart" => {
            trimmed.to_ascii_lowercase()
        }
        "go" => trimmed.to_string(),
        _ => trimmed.to_string(),
    }
}

pub(crate) fn manifest_dependency(
    name: impl Into<String>,
    dependency_kind: impl Into<String>,
    ecosystem: impl Into<String>,
    version: Option<String>,
) -> ManifestDependency {
    manifest_dependency_with_version_kind(
        name,
        dependency_kind,
        ecosystem,
        version,
        Some("constraint"),
    )
}

pub(crate) fn manifest_dependency_with_version_kind(
    name: impl Into<String>,
    dependency_kind: impl Into<String>,
    ecosystem: impl Into<String>,
    version: Option<String>,
    version_kind: Option<&str>,
) -> ManifestDependency {
    let version_kind = if version.is_some() {
        version_kind.map(str::to_string)
    } else {
        None
    };
    ManifestDependency {
        name: name.into(),
        kind: dependency_kind.into(),
        ecosystem: ecosystem.into(),
        version,
        version_kind,
        namespaces: Vec::new(),
    }
}

pub(crate) fn manifest_entrypoint(
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
        line: None,
    }
}

/// The same entry, declared on a line the extractor already knows.
pub(crate) fn manifest_entrypoint_at(
    label: impl Into<String>,
    entrypoint_kind: impl Into<String>,
    ecosystem: impl Into<String>,
    target: Option<String>,
    line: u32,
) -> ManifestEntrypoint {
    ManifestEntrypoint {
        line: Some(line),
        ..manifest_entrypoint(label, entrypoint_kind, ecosystem, target)
    }
}
