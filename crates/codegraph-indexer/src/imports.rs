//! Local import/include target resolution across languages, plus shared
//! path normalization utilities.

use std::collections::BTreeSet;

use codegraph_parser::Language;

#[allow(unused_imports)]
use crate::*;

pub(crate) fn local_import_target(
    language: Language,
    source_label: &str,
    import_label: &str,
    cmake_include_dirs: &[String],
    dart_packages: &[DartPackageRoot],
) -> Option<LocalImportTarget> {
    match language {
        Language::JavaScript | Language::TypeScript | Language::Tsx => {
            js_local_import_target(source_label, import_label)
        }
        Language::Python => python_local_import_target(source_label, import_label),
        Language::C | Language::Cpp => {
            c_local_import_target(source_label, import_label, cmake_include_dirs)
        }
        Language::Php => php_local_import_target(source_label, import_label),
        Language::Bash => bash_local_import_target(source_label, import_label),
        Language::Rust => rust_local_import_target(source_label, import_label),
        Language::Go => go_local_import_target(source_label, import_label),
        Language::Dart => dart_local_import_target(source_label, import_label, dart_packages),
    }
}

pub(crate) fn possible_local_import_target(
    language: Language,
    source_label: &str,
    import_label: &str,
    go_modules: &[GoModuleRoot],
    dart_packages: &[DartPackageRoot],
) -> Option<LocalImportTarget> {
    match language {
        Language::Python => python_absolute_local_import_target(source_label, import_label),
        Language::Go => go_module_import_target(import_label, go_modules),
        Language::Dart => dart_package_import_target(import_label, dart_packages),
        _ => None,
    }
}

pub(crate) fn js_local_import_target(
    source_label: &str,
    import_label: &str,
) -> Option<LocalImportTarget> {
    let module = first_quoted_value(import_label)?;
    if !(module.starts_with("./") || module.starts_with("../")) {
        return None;
    }
    Some(LocalImportTarget {
        target: module.clone(),
        candidates: module_file_candidates(
            source_label,
            &module,
            &["js", "ts", "tsx", "mjs", "cjs"],
        ),
    })
}

pub(crate) fn python_absolute_local_import_target(
    source_label: &str,
    import_label: &str,
) -> Option<LocalImportTarget> {
    let value = import_label.trim();
    let (module, imported) = if let Some(rest) = value.strip_prefix("import ") {
        (
            rest.split([',', ' ', '\n', '\t'])
                .find(|part| !part.is_empty())?,
            None,
        )
    } else {
        let rest = value.strip_prefix("from ")?;
        let module = rest.split_whitespace().next()?;
        if module.starts_with('.') {
            return None;
        }
        let imported = rest.split_once(" import ").and_then(|(_, imported)| {
            imported
                .split([',', ' ', '\n', '\t'])
                .find(|part| !part.is_empty())
        });
        (module, imported)
    };
    if module.is_empty() || module.starts_with('.') {
        return None;
    }

    let relative = module.replace('.', "/");
    let mut candidates = Vec::new();
    if let Some(imported) = imported {
        candidates.extend(python_module_candidates(&format!(
            "{relative}/{}",
            imported.replace('.', "/")
        )));
    }
    candidates.extend(python_module_candidates(&relative));
    if let Some(dir) = path_dir(source_label) {
        if let Some(imported) = imported {
            candidates.extend(python_module_candidates(&join_path(
                Some(&dir),
                &format!("{relative}/{}", imported.replace('.', "/")),
            )));
        }
        candidates.extend(python_module_candidates(&join_path(Some(&dir), &relative)));
    }
    dedup_preserving_order(&mut candidates);

    Some(LocalImportTarget {
        target: module.to_string(),
        candidates,
    })
}

pub(crate) fn python_local_import_target(
    source_label: &str,
    import_label: &str,
) -> Option<LocalImportTarget> {
    let value = import_label.trim();
    let rest = value.strip_prefix("from ")?;
    let dot_count = rest
        .chars()
        .take_while(|character| *character == '.')
        .count();
    if dot_count == 0 {
        return None;
    }
    let rest = &rest[dot_count..];
    let (module, imported) = rest.split_once(" import ")?;
    let imported = imported
        .split([',', ' ', '\n', '\t'])
        .find(|part| !part.is_empty())
        .unwrap_or("");
    let target = if module.trim().is_empty() {
        imported.to_string()
    } else {
        module.trim().to_string()
    };
    if target.is_empty() {
        return None;
    }

    let mut base = path_dir(source_label);
    for _ in 1..dot_count {
        if let Some(parent) = base.as_deref().and_then(path_dir) {
            base = Some(parent);
        } else {
            base = None;
        }
    }
    let relative = target.replace('.', "/");
    let module_path = join_path(base.as_deref(), &relative);
    let candidates = python_module_candidates(&module_path);
    Some(LocalImportTarget {
        target: format!("{}{}", ".".repeat(dot_count), target),
        candidates,
    })
}

pub(crate) fn python_module_candidates(module_path: &str) -> Vec<String> {
    let mut candidates = with_file_extensions(module_path, &["py"]);
    candidates.push(normalize_path(&format!("{module_path}/__init__.py")));
    candidates
}

pub(crate) fn dedup_preserving_order(values: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    values.retain(|value| seen.insert(value.clone()));
}

pub(crate) fn c_local_import_target(
    source_label: &str,
    import_label: &str,
    cmake_include_dirs: &[String],
) -> Option<LocalImportTarget> {
    let header = first_quoted_value(import_label)?;
    let mut candidates = vec![join_path(path_dir(source_label).as_deref(), &header)];
    candidates.extend(
        cmake_include_dirs
            .iter()
            .map(|include_dir| join_path(Some(include_dir), &header)),
    );
    dedup_preserving_order(&mut candidates);
    Some(LocalImportTarget {
        target: header.clone(),
        candidates,
    })
}

pub(crate) fn php_local_import_target(
    source_label: &str,
    import_label: &str,
) -> Option<LocalImportTarget> {
    let path = first_quoted_value(import_label)?;
    if path.contains("://") || path.starts_with('/') {
        return None;
    }
    let mut candidates = vec![join_path(path_dir(source_label).as_deref(), &path)];
    if !path_has_extension(&path) {
        candidates.push(join_path(
            path_dir(source_label).as_deref(),
            &format!("{path}.php"),
        ));
    }
    Some(LocalImportTarget {
        target: path,
        candidates,
    })
}

pub(crate) fn bash_local_import_target(
    source_label: &str,
    import_label: &str,
) -> Option<LocalImportTarget> {
    let mut parts = import_label.split_whitespace();
    let command = parts.next()?;
    if !matches!(command, "source" | ".") {
        return None;
    }
    let path = parts.next()?.trim_matches(['"', '\'']);
    if path.starts_with('/') || path.starts_with('$') || path.contains("://") {
        return None;
    }
    Some(LocalImportTarget {
        target: path.to_string(),
        candidates: vec![join_path(path_dir(source_label).as_deref(), path)],
    })
}

pub(crate) fn rust_local_import_target(
    source_label: &str,
    import_label: &str,
) -> Option<LocalImportTarget> {
    let value = import_label.trim().strip_prefix("use ")?.trim();
    let (base, rest) = if let Some(rest) = value.strip_prefix("crate::") {
        (rust_crate_root(source_label), rest)
    } else if let Some(rest) = value.strip_prefix("self::") {
        (path_dir(source_label), rest)
    } else {
        let rest = value.strip_prefix("super::")?;
        (
            path_dir(source_label).and_then(|path| path_dir(&path)),
            rest,
        )
    };
    let module = rest
        .split([':', ';', ',', '{', ' ', '\n', '\t'])
        .find(|part| !part.is_empty())?;
    if module.is_empty() || matches!(module, "self" | "super" | "crate") {
        return None;
    }
    // Glob imports (`use super::*;`) and leading-uppercase segments
    // (`use crate::ImpactRequest`) name items inside an already-loaded
    // module, not module files that could resolve on disk.
    if module == "*"
        || module
            .chars()
            .next()
            .is_some_and(|first| first.is_ascii_uppercase())
    {
        return None;
    }
    let module_path = join_path(base.as_deref(), module);
    Some(LocalImportTarget {
        target: module.to_string(),
        candidates: vec![
            normalize_path(&format!("{module_path}.rs")),
            normalize_path(&format!("{module_path}/mod.rs")),
        ],
    })
}

pub(crate) fn go_local_import_target(
    source_label: &str,
    import_label: &str,
) -> Option<LocalImportTarget> {
    let path = first_quoted_value(import_label)?;
    if !(path.starts_with("./") || path.starts_with("../")) {
        return None;
    }
    let package_dir = join_path(path_dir(source_label).as_deref(), &path);
    Some(LocalImportTarget {
        target: path,
        candidates: vec![directory_candidate(&package_dir)],
    })
}

pub(crate) fn go_module_import_target(
    import_label: &str,
    go_modules: &[GoModuleRoot],
) -> Option<LocalImportTarget> {
    let path = first_quoted_value(import_label)?;
    if path.starts_with("./") || path.starts_with("../") || path.starts_with('/') {
        return None;
    }
    let module = go_modules
        .iter()
        .find(|module| path == module.module || path.starts_with(&format!("{}/", module.module)))?;
    let suffix = path
        .strip_prefix(&module.module)
        .unwrap_or("")
        .trim_start_matches('/');
    let package_dir = join_path(module.dir.as_deref(), suffix);
    Some(LocalImportTarget {
        target: path,
        candidates: vec![directory_candidate(&package_dir)],
    })
}

pub(crate) fn dart_local_import_target(
    source_label: &str,
    import_label: &str,
    dart_packages: &[DartPackageRoot],
) -> Option<LocalImportTarget> {
    let uri = dart_import_uri(import_label)?;
    if uri.starts_with("./") || uri.starts_with("../") {
        return Some(LocalImportTarget {
            target: uri.clone(),
            candidates: vec![join_path(path_dir(source_label).as_deref(), &uri)],
        });
    }
    if uri.ends_with(".dart") && !uri.starts_with("package:") && !uri.contains("://") {
        return Some(LocalImportTarget {
            target: uri.clone(),
            candidates: vec![join_path(path_dir(source_label).as_deref(), &uri)],
        });
    }
    dart_package_uri_target(&uri, dart_packages)
}

pub(crate) fn dart_package_import_target(
    import_label: &str,
    dart_packages: &[DartPackageRoot],
) -> Option<LocalImportTarget> {
    let uri = dart_import_uri(import_label)?;
    dart_package_uri_target(&uri, dart_packages)
}

pub(crate) fn dart_package_uri_target(
    uri: &str,
    dart_packages: &[DartPackageRoot],
) -> Option<LocalImportTarget> {
    let rest = uri.strip_prefix("package:")?;
    let (package, path) = rest.split_once('/')?;
    if package.is_empty() || path.is_empty() {
        return None;
    }
    let package_root = dart_packages.iter().find(|root| root.name == package)?;
    let target = join_path(
        package_root.dir.as_deref(),
        &format!("{}/{path}", package_root.lib_dir),
    );
    Some(LocalImportTarget {
        target: uri.to_string(),
        candidates: vec![target],
    })
}

pub(crate) fn dart_import_uri(import_label: &str) -> Option<String> {
    first_quoted_value(import_label)
}

/// For a generated Dart file (build_runner and protoc conventions), the
/// scan-root-relative path of the source file that generates it.
pub(crate) fn dart_generated_source_name(label: &str) -> Option<String> {
    const GENERATED_SUFFIXES: &[&str] = &[
        ".g.dart",
        ".freezed.dart",
        ".gr.dart",
        ".pb.dart",
        ".pbenum.dart",
        ".pbjson.dart",
        ".pbserver.dart",
        ".mocks.dart",
        ".config.dart",
        ".gen.dart",
    ];
    for suffix in GENERATED_SUFFIXES {
        if let Some(base) = label.strip_suffix(suffix) {
            let file_base = base.rsplit('/').next().unwrap_or(base);
            if !file_base.is_empty() {
                return Some(format!("{base}.dart"));
            }
        }
    }
    None
}

pub(crate) fn directory_candidate(path: &str) -> String {
    let normalized = normalize_path(path);
    if normalized.is_empty() {
        "/".to_string()
    } else {
        format!("{normalized}/")
    }
}

pub(crate) fn module_file_candidates(
    source_label: &str,
    module: &str,
    extensions: &[&str],
) -> Vec<String> {
    let path = join_path(path_dir(source_label).as_deref(), module);
    let mut candidates = with_file_extensions(&path, extensions);
    for extension in extensions {
        candidates.push(normalize_path(&format!("{path}/index.{extension}")));
    }
    candidates
}

pub(crate) fn with_file_extensions(path: &str, extensions: &[&str]) -> Vec<String> {
    if path_has_extension(path) {
        vec![normalize_path(path)]
    } else {
        extensions
            .iter()
            .map(|extension| normalize_path(&format!("{path}.{extension}")))
            .collect()
    }
}

pub(crate) fn path_has_extension(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .is_some_and(|name| name.contains('.'))
}

pub(crate) fn rust_crate_root(source_label: &str) -> Option<String> {
    if source_label == "src/main.rs" || source_label == "src/lib.rs" {
        return Some("src".to_string());
    }
    source_label
        .strip_prefix("src/")
        .map(|_| "src".to_string())
        .or_else(|| path_dir(source_label))
}

pub(crate) fn path_dir(path: &str) -> Option<String> {
    path.rsplit_once('/')
        .map(|(dir, _)| dir.to_string())
        .filter(|dir| !dir.is_empty())
}

pub(crate) fn join_path(base: Option<&str>, relative: &str) -> String {
    let path = match base {
        Some(base) if !base.is_empty() => format!("{base}/{relative}"),
        _ => relative.to_string(),
    };
    normalize_path(&path)
}

pub(crate) fn normalize_path(path: &str) -> String {
    let mut parts = Vec::new();
    let normalized = path.replace('\\', "/");
    for part in normalized.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            value => parts.push(value),
        }
    }
    parts.join("/")
}
