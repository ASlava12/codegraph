//! The typed code graph model shared by every CodeGraph crate: nodes,
//! edges, kinds, confidence taxonomy, source spans, and JSON round-trip
//! serialization with a stable schema version.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

pub const CODEGRAPH_SCHEMA_VERSION: u32 = 1;

/// What an environment read is called when its key is computed. The angle
/// brackets are deliberate: no variable is named this, so it cannot be
/// mistaken for one.
pub const COMPUTED_ENVIRONMENT_KEY: &str = "<computed name>";

/// What identifies the build that produced a cached artefact.
///
/// Caches are keyed by a schema version that says when a record's *shape*
/// changed. Extraction rules change far more often than that, and a scan
/// of unchanged files is exactly the case where nothing else in the key
/// moves — so a record written by an earlier build would be served as
/// though the current one had produced it. The crate version covers
/// released binaries and the executable's modification time covers
/// everything between releases.
///
/// When the executable cannot be located the version alone is used: a
/// cache that is too willing to be reused beats no cache at all, and the
/// schema version still guards the record's shape. Computed once, because
/// the per-file parse cache consults it for every file.
/// Whether a name has "test" or "spec" among its words. Splitting is what
/// makes it safe: `blackbox-tests`, `jvmTest`, `Newtonsoft.Json.Tests` and
/// `BufferedSourceTest` all name tests, while `latest` and `manifest` do
/// not, and a rule matching bare substrings cannot tell them apart.
///
/// Public because a symbol can be a test where its file is not: Rust keeps
/// `mod tests` inside the module it exercises.
/// Whether the file is a test runner's own configuration, which its name
/// says outright: `vitest.config.ts`, `vitest.root.mjs`, `jest.config.js`,
/// `playwright.config.ts`. The runner's name alone is not enough -- a file
/// called `jest.ts` is ordinary code -- so a configuration is a name the
/// runner opens and something else follows.
fn names_a_test_runners_configuration(file_name: &str) -> bool {
    let mut parts = file_name.split('.');
    let Some(first) = parts.next() else {
        return false;
    };
    matches!(
        first,
        "vitest" | "jest" | "playwright" | "cypress" | "karma" | "jasmine" | "wdio"
    ) && parts.count() > 1
}

pub fn names_tests(part: &str) -> bool {
    let mut word = String::new();
    let mut found = false;
    let mut previous_lowercase = false;
    for character in part.chars() {
        let boundary =
            !character.is_alphanumeric() || (character.is_ascii_uppercase() && previous_lowercase);
        if boundary {
            found |= is_test_word(&word);
            word.clear();
        }
        if character.is_alphanumeric() {
            word.push(character.to_ascii_lowercase());
        }
        previous_lowercase = character.is_lowercase() || character.is_ascii_digit();
    }
    found | is_test_word(&word)
}

fn is_test_word(word: &str) -> bool {
    matches!(word, "test" | "tests" | "spec" | "specs")
}

/// Whether a path belongs to tests, examples, fixtures or generated
/// code rather than to the program itself. Shared because both the
/// resolver and the ranking need the same answer: a call in `src/`
/// resolving to a helper in `tests/` is not a dependency the program
/// has, and 1143 such links existed across the corpora.
pub fn is_test_like_source_path(path: &str) -> bool {
    let normalized_original = path.replace('\\', "/");
    let normalized = normalized_original.to_ascii_lowercase();
    let file_name = normalized.rsplit('/').next().unwrap_or(normalized.as_str());
    let original_file_name = normalized_original
        .rsplit('/')
        .next()
        .unwrap_or(normalized_original.as_str());
    let stem = file_name
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(file_name);
    let in_test_directory = normalized
        .split('/')
        .rev()
        .skip(1)
        .any(|part| {
            // Go's tool ignores a directory whose name starts with an
            // underscore, which is why gqlgen keeps its example apps in
            // `_examples`: the convention says outright that the code is
            // not part of the build.
            matches!(
                part.trim_matches('_'),
                "testdata"
                    | "testing"
                    | "testthat"
                    | "fixture"
                    | "fixtures"
                    | "example"
                    | "examples"
                    | "sample"
                    | "samples"
                    | "mock"
                    | "mocks"
                    // Code the project ships but did not write. Nearly
                    // half of what redis was calling its program is
                    // jemalloc, hiredis and lua under `deps` -- 6138 of
                    // 13770 definitions -- dune vendors a quarter of its
                    // own and nlohmann/json a fifth.
                    | "vendor"
                    | "vendored"
                    | "deps"
                    | "third_party"
                    | "thirdparty"
            )
        })
        // Case matters for this one: `jvmTest` only gives up its words
        // before it is lowercased.
        || normalized_original.split('/').rev().skip(1).any(names_tests);

    // protobuf names its output the same way in every language and nobody
    // writes it: 25 `*.pb.go` files carry 4694 of terraform's 18277
    // definitions, a quarter of what the graph was calling the program.
    // Dart's generated files were already read this way. This is asked
    // before Go's own rule below, which answers for every `.go` file and
    // would otherwise never let the question be put.
    if matches!(
        file_name.rsplit_once('.').map(|(stem, _)| stem),
        Some(stem) if stem.ends_with(".pb")
    ) || file_name.ends_with("_pb2.py")
        || file_name.ends_with("_pb2_grpc.py")
        || file_name.ends_with("_pb.rb")
        // A directory called `generated` says the same thing and was
        // already read that way -- for every language but Go, whose rule
        // below answers first and returned before this was ever asked.
        || normalized.contains("/generated/")
    {
        return true;
    }

    // Go compiles a test only from a file whose name ends `_test.go`, so
    // `test_file.go` is ordinary code however it reads -- terraform writes
    // its `terraform test` command in five files named that way. A test
    // directory still counts.
    if file_name.ends_with(".go") {
        return in_test_directory || stem.ends_with("_test");
    }

    in_test_directory
        // `BufferedSourceTest.kt` only gives up its words before it is
        // lowercased, as `jvmTest` does above.
        || names_tests(original_file_name)
        || stem == "test"
        || stem == "tests"
        || stem.starts_with("test_")
        || stem.ends_with("_test")
        || stem.ends_with("_tests")
        || stem.ends_with("_spec")
        || stem.ends_with("_specs")
        || file_name.contains(".test.")
        || file_name.contains(".spec.")
        // Storybook names its own files, and a story is development-only
        // code the way a test is. mastodon writes 50 of them, and 17
        // reported `storybook` as a package its production code needs but
        // the manifest declares only for development.
        || file_name.contains(".stories.")
        || file_name.contains(".story.")
        // A test runner's own configuration is not the program either:
        // zod's `vitest.root.mjs` imports `vitest`, which no manifest
        // should declare for production.
        || names_a_test_runners_configuration(file_name)
        || file_name.ends_with(".bats")
        || original_file_name.ends_with("Test.php")
        || original_file_name.ends_with("Spec.php")
        || file_name.ends_with("_test.dart")
        || file_name.ends_with(".g.dart")
        || file_name.ends_with(".freezed.dart")
        || file_name.ends_with(".mocks.dart")
        || file_name.ends_with(".gen.dart")
        || normalized.contains("/.dart_tool/")
}

/// Whether an area is a suite, asked of a directory rather than a file.
///
/// `is_test_like_source_path` looks past the last segment on purpose: there
/// it is a file name, and `example.rs` is not an example directory. An area
/// id is a directory all the way down, so its last segment is a directory
/// name too -- flask's `examples` holds 98 symbols the library does not
/// offer, and it sorted above `.github` among the areas of the project.
pub fn is_test_like_area(area: &str) -> bool {
    let area = area.trim_end_matches('/');
    is_test_like_source_path(area) || is_test_like_source_path(&format!("{area}/x"))
}

/// Whether a path holds code the project vendored rather than wrote: redis
/// carries jemalloc, lua and hiredis under `deps/`, and dune keeps `re` and
/// `opam` under `vendor/`. A FIXME or a cycle in there is upstream's, and
/// saying so as loudly as one in the project's own source buries the second.
pub fn is_vendored_source_path(path: &str) -> bool {
    path.replace('\\', "/").split('/').any(|segment| {
        matches!(
            segment.to_ascii_lowercase().as_str(),
            "vendor"
                | "vendored"
                | "third_party"
                | "thirdparty"
                | "3rdparty"
                | "deps"
                | "node_modules"
                // spdlog carries fmt in `include/spdlog/fmt/bundled`, and a
                // project that copies a library in names the directory
                // either this way or `external`.
                | "bundled"
                | "external"
        )
    })
}

/// Whether a Python module name is the standard library's.
pub fn is_python_stdlib_package(package: &str) -> bool {
    // Python's own `sys.stdlib_module_names`, minus the private
    // underscore modules. A partial list is worse than none here: the
    // insight warns that an import is undeclared, and every module it
    // does not know makes that claim about the standard library.
    matches!(
        package,
        "abc"
            | "annotationlib"
            | "antigravity"
            | "argparse"
            | "array"
            | "ast"
            | "asyncio"
            | "atexit"
            | "base64"
            | "bdb"
            | "binascii"
            | "bisect"
            | "builtins"
            | "bz2"
            | "cProfile"
            | "calendar"
            | "cmath"
            | "cmd"
            | "code"
            | "codecs"
            | "codeop"
            | "collections"
            | "colorsys"
            | "compileall"
            | "compression"
            | "concurrent"
            | "configparser"
            | "contextlib"
            | "contextvars"
            | "copy"
            | "copyreg"
            | "csv"
            | "ctypes"
            | "curses"
            | "dataclasses"
            | "datetime"
            | "dbm"
            | "decimal"
            | "difflib"
            | "dis"
            | "doctest"
            | "email"
            | "encodings"
            | "ensurepip"
            | "enum"
            | "errno"
            | "faulthandler"
            | "fcntl"
            | "filecmp"
            | "fileinput"
            | "fnmatch"
            | "fractions"
            | "ftplib"
            | "functools"
            | "gc"
            | "genericpath"
            | "getopt"
            | "getpass"
            | "gettext"
            | "glob"
            | "graphlib"
            | "grp"
            | "gzip"
            | "hashlib"
            | "heapq"
            | "hmac"
            | "html"
            | "http"
            | "idlelib"
            | "imaplib"
            | "importlib"
            | "inspect"
            | "io"
            | "ipaddress"
            | "itertools"
            | "json"
            | "keyword"
            | "linecache"
            | "locale"
            | "logging"
            | "lzma"
            | "mailbox"
            | "marshal"
            | "math"
            | "mimetypes"
            | "mmap"
            | "modulefinder"
            | "msvcrt"
            | "multiprocessing"
            | "netrc"
            | "nt"
            | "ntpath"
            | "nturl2path"
            | "numbers"
            | "opcode"
            | "operator"
            | "optparse"
            | "os"
            | "pathlib"
            | "pdb"
            | "pickle"
            | "pickletools"
            | "pkgutil"
            | "platform"
            | "plistlib"
            | "poplib"
            | "posix"
            | "posixpath"
            | "pprint"
            | "profile"
            | "pstats"
            | "pty"
            | "pwd"
            | "py_compile"
            | "pyclbr"
            | "pydoc"
            | "pydoc_data"
            | "pyexpat"
            | "queue"
            | "quopri"
            | "random"
            | "re"
            | "readline"
            | "reprlib"
            | "resource"
            | "rlcompleter"
            | "runpy"
            | "sched"
            | "secrets"
            | "select"
            | "selectors"
            | "shelve"
            | "shlex"
            | "shutil"
            | "signal"
            | "site"
            | "smtplib"
            | "socket"
            | "socketserver"
            | "sqlite3"
            | "sre_compile"
            | "sre_constants"
            | "sre_parse"
            | "ssl"
            | "stat"
            | "statistics"
            | "string"
            | "stringprep"
            | "struct"
            | "subprocess"
            | "symtable"
            | "sys"
            | "sysconfig"
            | "syslog"
            | "tabnanny"
            | "tarfile"
            | "tempfile"
            | "termios"
            | "textwrap"
            | "this"
            | "threading"
            | "time"
            | "timeit"
            | "tkinter"
            | "token"
            | "tokenize"
            | "tomllib"
            | "trace"
            | "traceback"
            | "tracemalloc"
            | "tty"
            | "turtle"
            | "turtledemo"
            | "types"
            | "typing"
            | "unicodedata"
            | "unittest"
            | "urllib"
            | "uuid"
            | "venv"
            | "warnings"
            | "wave"
            | "weakref"
            | "webbrowser"
            | "winreg"
            | "winsound"
            | "wsgiref"
            | "xml"
            | "xmlrpc"
            | "zipapp"
            | "zipfile"
            | "zipimport"
            | "zlib"
            | "zoneinfo"
    )
}

pub fn build_identity() -> &'static str {
    static IDENTITY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    IDENTITY.get_or_init(|| {
        let version = env!("CARGO_PKG_VERSION");
        match std::env::current_exe()
            .and_then(|path| path.metadata())
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        {
            Some(since) => format!("{version}+{}", since.as_secs()),
            None => version.to_string(),
        }
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u64);

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "n{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Repository,
    Directory,
    File,
    Module,
    Function,
    Entrypoint,
    Type,
    Config,
    Environment,
    ExternalDependency,
    /// Branch/loop/async/return/error-flow source facts.
    ControlFlow,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Contains,
    Imports,
    Calls,
    Defines,
    References,
    ReadsConfig,
    ReadsEnvironment,
    MayError,
    Entrypoint,
    DependsOn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Exact,
    Semantic,
    Syntactic,
    Heuristic,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpan {
    pub path: String,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    pub label: String,
    pub span: Option<SourceSpan>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    pub source: NodeId,
    pub target: NodeId,
    pub kind: EdgeKind,
    pub confidence: Confidence,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeGraph {
    pub schema_version: u32,
    pub root: NodeId,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

impl CodeGraph {
    pub fn new(root_label: impl Into<String>) -> Self {
        let root = NodeId(1);
        Self {
            schema_version: CODEGRAPH_SCHEMA_VERSION,
            root,
            nodes: vec![Node {
                id: root,
                kind: NodeKind::Repository,
                label: root_label.into(),
                span: None,
                metadata: BTreeMap::new(),
            }],
            edges: Vec::new(),
        }
    }

    pub fn add_node(&mut self, kind: NodeKind, label: impl Into<String>) -> NodeId {
        self.add_node_with_metadata(kind, label, None, BTreeMap::new())
    }

    pub fn add_node_with_span(
        &mut self,
        kind: NodeKind,
        label: impl Into<String>,
        span: SourceSpan,
    ) -> NodeId {
        self.add_node_with_metadata(kind, label, Some(span), BTreeMap::new())
    }

    pub fn add_node_with_metadata(
        &mut self,
        kind: NodeKind,
        label: impl Into<String>,
        span: Option<SourceSpan>,
        metadata: BTreeMap<String, String>,
    ) -> NodeId {
        let id = NodeId(self.nodes.len() as u64 + 1);
        self.nodes.push(Node {
            id,
            kind,
            label: label.into(),
            span,
            metadata,
        });
        id
    }

    pub fn add_edge(
        &mut self,
        source: NodeId,
        target: NodeId,
        kind: EdgeKind,
        confidence: Confidence,
    ) {
        self.add_edge_with_metadata(source, target, kind, confidence, BTreeMap::new());
    }

    pub fn add_edge_with_metadata(
        &mut self,
        source: NodeId,
        target: NodeId,
        kind: EdgeKind,
        confidence: Confidence,
        metadata: BTreeMap<String, String>,
    ) {
        self.edges.push(Edge {
            source,
            target,
            kind,
            confidence,
            metadata,
        });
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn code_a_project_ships_but_did_not_write_is_not_its_program() {
        // Nearly half of what redis was calling its program is jemalloc,
        // hiredis and lua under `deps`: 6138 of 13770 definitions. dune
        // vendors a quarter of its own and nlohmann/json a fifth.
        assert!(is_test_like_source_path("deps/jemalloc/src/jemalloc.c"));
        assert!(is_test_like_source_path("vendor/lwd/lwd/lwd.ml"));
        assert!(is_test_like_source_path(
            "include/nlohmann/thirdparty/hedley/hedley.hpp"
        ));
        assert!(is_test_like_source_path("third_party/zlib/deflate.c"));

        // The word has to be a directory of its own, not part of a name.
        assert!(!is_test_like_source_path("src/vendored_ids.rs"));
        assert!(!is_test_like_source_path("src/depsgraph.c"));
    }

    #[test]
    fn generated_code_is_not_the_program_either() {
        // protobuf names its output the same way in every language and
        // nobody writes it: 25 `*.pb.go` files carry 4694 of terraform's
        // 18277 definitions.
        assert!(is_test_like_source_path(
            "internal/tfplugin5/tfplugin5.pb.go"
        ));
        assert!(is_test_like_source_path("proto/service_pb2.py"));
        assert!(is_test_like_source_path("proto/service_pb2_grpc.py"));
        assert!(is_test_like_source_path("lib/service_pb.rb"));
        assert!(is_test_like_source_path("src/message.pb.cc"));

        // Go answers for every `.go` file with its own rule, and asking it
        // first left this question unable to be put at all.
        assert!(!is_test_like_source_path("internal/tfplugin5/plugin.go"));
        // A name that merely holds the letters is ordinary code.
        assert!(!is_test_like_source_path("src/pbkdf2.go"));
        assert!(!is_test_like_source_path("src/upb.go"));
    }

    #[test]
    fn a_story_is_not_the_program() {
        // Storybook names its files the way a test framework does, and a
        // story is development-only code for the same reason a test is.
        assert!(is_test_like_source_path(
            "app/javascript/mastodon/components/alert/alert.stories.tsx"
        ));
        assert!(is_test_like_source_path("src/Button.story.js"));

        // A test runner's configuration is not the program either.
        assert!(is_test_like_source_path("vitest.root.mjs"));
        assert!(is_test_like_source_path("vitest.config.ts"));
        assert!(is_test_like_source_path("packages/ui/jest.config.js"));

        // The runner's name alone says nothing: a file called `jest.ts` is
        // ordinary code, and so is a component whose name merely holds the
        // word.
        assert!(!is_test_like_source_path("src/jest.ts"));
        assert!(!is_test_like_source_path("src/history.ts"));
        assert!(!is_test_like_source_path("src/stories.ts"));
    }

    #[test]
    fn an_area_is_a_directory_all_the_way_down() {
        // `is_test_like_source_path` looks past the last segment because
        // there it is a file name. An area id has no file name in it, so
        // flask's `examples` sorted above `.github` among the areas of the
        // project, and `examples -> src` above `tests -> src`.
        assert!(!is_test_like_source_path("examples"));
        assert!(is_test_like_area("examples"));
        assert!(is_test_like_area("tests"));
        assert!(is_test_like_area("spec"));
        assert!(is_test_like_area("app/javascript/__tests__"));
        assert!(!is_test_like_area("src"));
        assert!(!is_test_like_area("app/models"));
        // And it stays the file question for a file: `example.rs` is code.
        assert!(!is_test_like_area("src/example.rs"));
    }
    #[test]
    fn vendored_paths_are_the_ones_a_project_carries() {
        for path in [
            "deps/jemalloc/src/arena.c",
            "vendor/re/src/pmark.ml",
            "src/lev/vendor/ev.c",
            "third_party/zlib/zlib.h",
            "include/spdlog/fmt/bundled/format.h",
            "external/googletest/src/gtest.cc",
            "node_modules/puppeteer/install.mjs",
        ] {
            assert!(is_vendored_source_path(path), "{path}");
        }
        for path in [
            "src/server.c",
            "crates/codegraph-core/src/lib.rs",
            "dependencies.md",
        ] {
            assert!(!is_vendored_source_path(path), "{path}");
        }
    }

    use super::*;

    #[test]
    fn confidence_levels_round_trip_as_stable_snake_case_values() {
        let cases = [
            (Confidence::Exact, "exact"),
            (Confidence::Semantic, "semantic"),
            (Confidence::Syntactic, "syntactic"),
            (Confidence::Heuristic, "heuristic"),
            (Confidence::Unknown, "unknown"),
        ];

        for (confidence, expected) in cases {
            let encoded = serde_json::to_value(confidence).expect("serialize confidence");
            assert_eq!(encoded, serde_json::json!(expected));
            let decoded: Confidence =
                serde_json::from_value(encoded).expect("deserialize confidence");
            assert_eq!(decoded, confidence);
        }
    }
}

#[cfg(test)]
mod test_path_words {
    use super::is_test_like_source_path;

    #[test]
    fn recognises_test_conventions_without_swallowing_ordinary_names() {
        for path in [
            "okio/src/jvmTest/kotlin/okio/BufferedSourceTest.kt",
            "test/blackbox-tests/expect-tests/foo.ml",
            "Src/Newtonsoft.Json.Tests/JsonTextReaderTest.cs",
            "packages/core/__tests__/index.spec.ts",
            "tests/test_basic.py",
            "src/foo_test.go",
            "web/browser.test.js",
            // Go's tool ignores a directory that starts with an
            // underscore: gqlgen keeps twenty example apps in `_examples`.
            "_examples/chat/server.go",
            "_testdata/schema.graphql",
            // Jest and Vitest name the directory with underscores on both
            // sides: koel keeps `__mocks__/useContextMenu.ts` there.
            "resources/js/composables/__mocks__/useContextMenu.ts",
        ] {
            assert!(is_test_like_source_path(path), "{path}");
        }
        for path in [
            "src/latest.rs",
            "src/manifest.py",
            "lib/contest/rules.rb",
            "src/protest.go",
            // The underscore is only meaningful as the whole segment's
            // prefix: `_internal` is not an example directory.
            "_internal/registry.go",
            "crates/codegraph-core/src/lib.rs",
            "src/specify_options.ts",
            // Go compiles a test only from a file ending `_test.go`, so
            // these are ordinary code: terraform writes its `terraform
            // test` command in files named exactly this way.
            "internal/configs/test_file.go",
            "internal/command/test_cleanup.go",
        ] {
            assert!(!is_test_like_source_path(path), "{path}");
        }
    }
}
