//! Single-file components: a `.vue` or `.svelte` file holds a template, a
//! script and a style together, and the script is the program. koel writes
//! 337 of them -- its whole interface -- and the graph held nothing from
//! any of them.

use std::path::Path;

use codegraph_parser::{Language, ParsedFile, adapter_for_language};

/// Whether the file is a component whose script the scan can read.
pub(crate) fn is_single_file_component(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("vue") || extension.eq_ignore_ascii_case("svelte")
        })
}

/// The program a component states, with every other line blanked so a
/// fact keeps the line of the file that holds it.
pub(crate) fn parse_single_file_component(
    label: &str,
    source: &[u8],
) -> Option<(Language, Result<ParsedFile, codegraph_parser::ParseError>)> {
    let text = std::str::from_utf8(source).ok()?;
    let (program, language) = component_script(text)?;
    if program.trim().is_empty() {
        return None;
    }
    let adapter = adapter_for_language(language)?;
    Some((
        language,
        adapter.parse(Path::new(label), program.as_bytes()),
    ))
}

/// The script blocks of a component, blank-padded to the file's own
/// lines. A component may open more than one -- Vue writes `<script
/// setup>` beside a plain `<script>` -- and both are the program.
fn component_script(text: &str) -> Option<(String, Language)> {
    let mut program = String::with_capacity(text.len());
    let mut inside = false;
    let mut language = Language::JavaScript;
    let mut found = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if !inside && trimmed.starts_with("<script") {
            inside = true;
            found = true;
            if names_typescript(line) {
                language = Language::TypeScript;
            }
            // What follows the opening tag on the same line is program.
            match line.split_once('>') {
                Some((_, rest)) if !rest.trim().is_empty() => {
                    // A one-line component: `<script>const a = 1</script>`.
                    match rest.split_once("</script>") {
                        Some((body, _)) => {
                            program.push_str(body);
                            inside = false;
                        }
                        None => program.push_str(rest),
                    }
                }
                _ => {}
            }
            program.push('\n');
            continue;
        }
        if inside && trimmed.starts_with("</script>") {
            inside = false;
            program.push('\n');
            continue;
        }
        if inside {
            program.push_str(line);
        }
        program.push('\n');
    }
    found.then_some((program, language))
}

/// Whether the opening tag says the script is TypeScript.
fn names_typescript(line: &str) -> bool {
    let lowered = line.to_ascii_lowercase();
    lowered.contains("lang=\"ts\"")
        || lowered.contains("lang='ts'")
        || lowered.contains("lang=\"typescript\"")
        || lowered.contains("lang='typescript'")
}
