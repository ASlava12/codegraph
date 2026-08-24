//! Jupyter notebooks: a `.ipynb` file is JSON holding the program someone
//! wrote in cells, and the facts in it are the program's.

use std::path::Path;

use codegraph_parser::{Language, ParsedFile, adapter_for_language};

#[allow(unused_imports)]
use crate::*;

/// One line of a notebook's program: the text, and the line of the `.ipynb`
/// file that holds it.
struct NotebookLine {
    text: String,
    notebook_line: u32,
}

/// The program a notebook's code cells hold, and where each of its lines
/// sits in the file. Jupyter writes one source line per line of JSON, which
/// is what makes the second half possible; a notebook written any other way
/// yields the program without the mapping.
fn notebook_program(source: &str) -> Vec<NotebookLine> {
    let mut lines = Vec::new();
    let mut in_code_cell = false;
    let mut in_source = false;
    for (index, raw) in source.lines().enumerate() {
        let trimmed = raw.trim();
        if trimmed.starts_with("\"cell_type\"") {
            in_code_cell = trimmed.contains("\"code\"");
            continue;
        }
        if !in_code_cell {
            continue;
        }
        if trimmed.starts_with("\"source\"") {
            in_source = true;
            // `"source": []` and `"source": ["x"]` are written on one line
            // when the cell is short.
            if let Some(rest) = trimmed.split_once('[').map(|(_, rest)| rest) {
                for value in json_strings_in(rest) {
                    lines.push(NotebookLine {
                        text: value,
                        notebook_line: index as u32 + 1,
                    });
                }
                if rest.contains(']') {
                    in_source = false;
                }
            }
            continue;
        }
        if !in_source {
            continue;
        }
        if trimmed.starts_with(']') {
            in_source = false;
            continue;
        }
        for value in json_strings_in(trimmed) {
            lines.push(NotebookLine {
                text: value,
                notebook_line: index as u32 + 1,
            });
        }
    }
    lines
}

/// The JSON strings a line holds, unescaped.
fn json_strings_in(line: &str) -> Vec<String> {
    let mut values = Vec::new();
    let bytes = line.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'"' {
            index += 1;
            continue;
        }
        let start = index;
        index += 1;
        while index < bytes.len() {
            match bytes[index] {
                b'\\' => index += 2,
                b'"' => break,
                _ => index += 1,
            }
        }
        if index >= bytes.len() {
            break;
        }
        index += 1;
        if let Ok(value) = serde_json::from_str::<String>(&line[start..index]) {
            values.push(value);
        }
    }
    values
}

/// Whether a line is IPython's rather than Python's: a magic, a shell
/// escape, or the `?` that asks for help.
fn is_ipython_magic(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with('%')
        || trimmed.starts_with('!')
        || trimmed.starts_with("?")
        || (trimmed.ends_with('?') && !trimmed.contains(' '))
}

/// A notebook's code read as the program it is, with every fact pointing at
/// the line of the `.ipynb` file that holds it.
pub(crate) fn parse_notebook(label: &str, source: &[u8]) -> Option<ParsedFile> {
    let text = std::str::from_utf8(source).ok()?;
    if !text.contains("\"cells\"") {
        return None;
    }
    let program = notebook_program(text);
    if program.is_empty() {
        return None;
    }
    let joined = program
        .iter()
        .map(|line| {
            let text = line.text.trim_end_matches('\n');
            // `%matplotlib inline` and `!pip install x` are IPython's, not
            // Python's: 69 of pytudes' 113 notebooks fail to parse with
            // them in, and a blank line keeps every other fact on the line
            // that holds it.
            if is_ipython_magic(text) { "" } else { text }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let adapter = adapter_for_language(Language::Python)?;
    let mut parsed = adapter.parse(Path::new(label), joined.as_bytes()).ok()?;
    let notebook_line = |line: u32| {
        program
            .get(line.saturating_sub(1) as usize)
            .map(|entry| entry.notebook_line)
            .unwrap_or(line)
    };
    for item in &mut parsed.items {
        item.span.start_line = notebook_line(item.span.start_line);
        item.span.end_line = notebook_line(item.span.end_line);
    }
    for reference in &mut parsed.type_references {
        reference.span.start_line = notebook_line(reference.span.start_line);
        reference.span.end_line = notebook_line(reference.span.end_line);
    }
    Some(parsed)
}
