//! Concatenates the ordered web UI modules from
//! `crates/codegraph-web/static/js/` into one `app.js` in `OUT_DIR` so the
//! browser keeps loading a single script while the sources stay modular.
//! Lexicographic file order (the `NN-` prefixes) preserves the original
//! single-file statement order, which top-level wiring relies on.

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let js_dir = PathBuf::from("../codegraph-web/static/js");
    println!("cargo::rerun-if-changed={}", js_dir.display());

    let mut names: Vec<String> = fs::read_dir(&js_dir)
        .expect("web js module directory should exist")
        .map(|entry| entry.expect("readable js dir entry").file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".js"))
        .collect();
    names.sort();
    assert!(
        !names.is_empty(),
        "no web js modules found in {}",
        js_dir.display()
    );

    let mut bundle = String::new();
    for name in &names {
        let path = js_dir.join(name);
        println!("cargo::rerun-if-changed={}", path.display());
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        bundle.push_str(&source);
        if !bundle.ends_with('\n') {
            bundle.push('\n');
        }
        bundle.push('\n');
    }

    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR")).join("app.js");
    fs::write(&out, bundle).expect("failed to write concatenated app.js");
}
