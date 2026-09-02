//! Post-scan resolution passes over pending queues: calls, imports,
//! entrypoint targets, compose/Kubernetes/CI references, documents, SQL,
//! and the function symbol registry.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

use codegraph_core::{
    COMPUTED_ENVIRONMENT_KEY, CodeGraph, Confidence, EdgeKind, Node, NodeId, NodeKind, SourceSpan,
    is_shipped_but_not_written, is_test_like_source_path,
};

#[allow(unused_imports)]
use crate::*;

/// Language builtins and std macros that read as call sites but are not
/// external dependencies: constructors of core enums, print/assert/format
/// macro families, and std namespaces.
/// Whether a call written through a receiver names a member the language
/// gives every value. A project may declare `fn map` or `ToString` of its
/// own, but `option.map(..)` and `value.ToString()` are not calls to it, and
/// the syntax alone cannot say what type the receiver has.
/// Methods every ruby object or collection answers to. A project declares
/// these on its own types as readily as the core library does, which is why
/// they are not builtins -- but a call written through a value the syntax
/// does not name is the core library's far more often than it is the
/// project's, and matching by name alone cannot tell them apart.
fn ruby_method_of_every_value(method: &str) -> bool {
    matches!(
        method,
        "each"
            | "each_with_index"
            | "each_with_object"
            | "map"
            | "flat_map"
            | "select"
            | "filter"
            | "reject"
            | "find"
            | "detect"
            | "reduce"
            | "inject"
            | "size"
            | "length"
            | "count"
            | "first"
            | "last"
            | "empty?"
            | "any?"
            | "all?"
            | "none?"
            | "include?"
            | "to_s"
            | "to_a"
            | "to_h"
            | "to_i"
            | "to_json"
            | "keys"
            | "values"
            | "sort"
            | "sort_by"
            | "group_by"
            | "min"
            | "max"
            | "sum"
            | "uniq"
            | "join"
            | "split"
            | "freeze"
            | "dup"
            | "clone"
            | "hash"
            | "tap"
            | "then"
            | "present?"
            | "blank?"
            | "nil?"
            | "is_a?"
            | "kind_of?"
            | "respond_to?"
            | "instance_of?"
    )
}

/// Whether the name is a method every value in the language already has,
/// whatever the receiver is. Asked when the receiver is known to be a
/// value but never reached the label.
fn method_of_every_value(language: &str, method: &str) -> bool {
    match language {
        "rust" => rust_method_is_std(method),
        "javascript" | "typescript" | "tsx" => js_member_of_every_value(method),
        "python" => python_method_of_every_value(method),
        "csharp" => csharp_member_of_every_value(method),
        _ => false,
    }
}

pub(crate) fn receiver_call_is_universal(language: &str, label: &str) -> bool {
    let (receiver, method) = match label.rsplit_once('.') {
        Some((receiver, method)) => (receiver.trim(), method.trim()),
        None => return false,
    };
    match language {
        "rust" => rust_method_is_std(method),
        // A JS or TS call keeps its receiver in the label, and `this` is
        // the one receiver whose methods are the class's own. Everything
        // else is a value whose type the syntax does not name: `str.trim`
        // is a string's, `Buffer.concat` node's, `map.get` a Map's, and
        // `this._def.checks.find` an array's -- yet axios declares a
        // `trim`, vue a `get` and zod a `find`, and matching on the tail
        // gave each of them callers it never had.
        "javascript" | "typescript" | "tsx" => {
            !matches!(receiver, "this" | "self") && js_member_of_every_value(method)
        }
        // Python names the receiver too, and `self` (or `cls`) is the one
        // whose methods are the class's own: `value.split` is a string's,
        // `kwargs.setdefault` a dict's, `stack.extend` a list's -- while
        // django-oscar declares a `split`, flask a `setdefault` and pytudes
        // an `extend`.
        "python" => !matches!(receiver, "self" | "cls") && python_method_of_every_value(method),
        // C# writes a static call the same way as an instance call, and only
        // the receiver's spelling tells them apart: `JsonConvert.ToString` is
        // Newtonsoft's own API, called 720 times, while `value.ToString` is
        // the one every object inherits.
        "csharp" => !receiver_names_a_csharp_type(receiver) && csharp_member_of_every_value(method),
        _ => false,
    }
}

/// Methods Python gives every list, dict, set or string. Names a project
/// defines as readily as the language does -- `get`, `update`, `read`,
/// `write`, `close`, `add`, `count`, `index`, `format` -- are left out.
fn python_method_of_every_value(method: &str) -> bool {
    matches!(
        method,
        "append"
            | "extend"
            | "insert"
            | "sort"
            | "reverse"
            // `keys`, `values` and `items` are the mapping protocol, and a
            // project that mimics a dict declares all three: requests'
            // `RequestsCookieJar` does, and refusing them would take its
            // callers away.
            | "setdefault"
            | "popitem"
            | "join"
            | "split"
            | "rsplit"
            | "splitlines"
            | "strip"
            | "lstrip"
            | "rstrip"
            | "startswith"
            | "endswith"
            | "lower"
            | "upper"
            | "title"
            | "casefold"
            | "encode"
            | "decode"
            | "rjust"
            | "ljust"
            | "zfill"
            | "isdigit"
            | "isalpha"
            | "isspace"
            | "partition"
            | "format_map"
    )
}

/// Methods JavaScript gives every array, string, promise or function.
/// Names a project defines as readily as the platform does -- `get`, `set`,
/// `has`, `add`, `on`, `emit`, `close`, `find` -- are left out, because
/// there the project's own method is the likelier answer.
fn js_member_of_every_value(method: &str) -> bool {
    matches!(
        method,
        "map"
            | "filter"
            | "forEach"
            | "reduce"
            | "reduceRight"
            | "some"
            | "every"
            | "findIndex"
            | "indexOf"
            | "lastIndexOf"
            | "join"
            | "slice"
            | "splice"
            | "concat"
            | "push"
            | "pop"
            | "shift"
            | "unshift"
            | "sort"
            | "reverse"
            | "flat"
            | "flatMap"
            | "entries"
            | "then"
            | "catch"
            | "finally"
            | "toString"
            | "valueOf"
            | "trim"
            | "trimStart"
            | "trimEnd"
            | "split"
            | "replace"
            | "replaceAll"
            | "match"
            | "matchAll"
            | "startsWith"
            | "endsWith"
            | "padStart"
            | "padEnd"
            | "toLowerCase"
            | "toUpperCase"
            | "charAt"
            | "charCodeAt"
            | "substring"
            | "repeat"
            | "call"
            | "apply"
            | "bind"
            | "toFixed"
    )
}

/// Whether a C# receiver is spelled the way the language spells a type: a
/// static call names one, an instance call names a local, field or property.
fn receiver_names_a_csharp_type(receiver: &str) -> bool {
    let last = receiver.rsplit('.').next().unwrap_or(receiver);
    last.chars().next().is_some_and(char::is_uppercase)
}

/// Members of `object`, `IDisposable`, `IEnumerable`, `Task` and LINQ: every
/// value in the language has them, whatever its type. Names a project defines
/// as readily as the framework does — `Add`, `Contains`, `Format` — are left
/// out, because there the project's own method is the likelier answer.
fn csharp_member_of_every_value(method: &str) -> bool {
    let method = method.split('<').next().unwrap_or(method);
    matches!(
        method,
        "ToString"
            | "Equals"
            | "GetHashCode"
            | "GetType"
            | "Dispose"
            | "DisposeAsync"
            | "MemberwiseClone"
            | "ReferenceEquals"
            | "GetEnumerator"
            | "MoveNext"
            | "ConfigureAwait"
            | "GetAwaiter"
            | "GetResult"
            | "ToArray"
            | "ToList"
            | "AsEnumerable"
            | "Cast"
            | "OfType"
            | "Select"
            | "SelectMany"
            | "Where"
            | "OrderBy"
            | "OrderByDescending"
            | "ThenBy"
            | "FirstOrDefault"
            | "LastOrDefault"
            | "SingleOrDefault"
            | "Distinct"
            | "Aggregate"
    )
}

/// Types the .NET platform gives every C# program. A call qualified by one
/// of them — `TimeSpan.FromSeconds(..)`, `Guid.NewGuid()` — is the
/// framework's, and so is `new ArgumentNullException(..)`.
fn csharp_platform_type(name: &str) -> bool {
    matches!(
        name.split('<').next().unwrap_or(name),
        "string"
            | "String"
            | "object"
            | "Object"
            | "Convert"
            | "Math"
            | "Console"
            | "Debug"
            | "Trace"
            | "Task"
            | "ValueTask"
            | "Thread"
            | "Interlocked"
            | "Monitor"
            | "Volatile"
            | "TimeSpan"
            | "DateTime"
            | "DateTimeOffset"
            | "Stopwatch"
            | "Guid"
            | "Uri"
            | "Random"
            | "Enumerable"
            | "Array"
            | "Activator"
            | "Encoding"
            | "StringBuilder"
            | "List"
            | "Dictionary"
            | "HashSet"
            | "Queue"
            | "Stack"
            | "Lazy"
            | "Nullable"
            | "CancellationToken"
            | "CancellationTokenSource"
            | "MemoryStream"
            | "StreamReader"
            | "StreamWriter"
            | "StringReader"
            | "StringWriter"
            | "TextReader"
            | "TextWriter"
            | "BitConverter"
            | "Buffer"
            | "Environment"
            | "CultureInfo"
            | "Regex"
            | "File"
            | "Path"
            | "Directory"
            | "Exception"
            | "ArgumentException"
            | "ArgumentNullException"
            | "ArgumentOutOfRangeException"
            | "InvalidOperationException"
            | "NotSupportedException"
            | "NotImplementedException"
            | "ObjectDisposedException"
            | "OperationCanceledException"
            | "TaskCanceledException"
            | "TimeoutException"
            | "FormatException"
            | "OverflowException"
    )
}

/// Methods the Rust standard library gives every type.
fn rust_method_is_std(method: &str) -> bool {
    matches!(
        method,
        "unwrap"
            | "expect"
            | "parse"
            | "map"
            | "map_err"
            | "and_then"
            | "or_else"
            | "ok_or"
            | "ok_or_else"
            | "unwrap_or"
            | "unwrap_or_default"
            | "unwrap_or_else"
            | "clone"
            | "cloned"
            | "copied"
            | "into"
            | "to_string"
            | "to_owned"
            | "as_str"
            | "as_ref"
            | "as_deref"
            | "as_slice"
            | "iter"
            | "into_iter"
            | "iter_mut"
            | "collect"
            | "next"
            | "is_empty"
            | "is_some"
            | "is_none"
            | "is_ok"
            | "is_err"
            | "is_some_and"
            | "is_none_or"
    )
}

/// A C function one of Apple's frameworks provides. Objective-C calls
/// them by bare name -- `dispatch_async`, `NSStringFromSelector`,
/// `SecCertificateCopyData`, `objc_setAssociatedObject` -- and every
/// framework prefixes its own. A project that defines the name itself
/// never reaches here: this answers only for calls nothing resolved.
fn objc_platform_function(name: &str) -> bool {
    if name.contains(':') {
        return false;
    }
    let lowercase_prefixes = [
        "dispatch_",
        "objc_",
        "os_",
        "sel_",
        "class_",
        "method_",
        "imp_",
    ];
    if lowercase_prefixes
        .iter()
        .any(|prefix| name.starts_with(prefix))
    {
        return true;
    }
    // `NSStringFromClass` is Foundation's and `AFQueryStringFromParameters`
    // is AFNetworking's: the capital that follows the prefix is what tells
    // a framework function from a name that merely begins with those
    // letters.
    [
        "NS",
        "CF",
        "CG",
        "CA",
        "CM",
        "CV",
        "CT",
        "AV",
        "UI",
        "WK",
        "Sec",
        "SC",
        "AudioObject",
    ]
    .iter()
    .any(|prefix| {
        name.starts_with(prefix)
            && name[prefix.len()..]
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_uppercase())
    })
}

/// Whether an Objective-C message goes to a class one of Apple's
/// frameworks provides. `[NSURL URLWithString:]` and `[NSSet
/// setWithObject:]` are Foundation's, and the selector alone reads like
/// any other method. A project's own method never reaches here: this
/// answers only for calls that resolved to nothing.
fn objc_platform_receiver(language: &str, receiver: Option<&str>) -> bool {
    if language != "objc" {
        return false;
    }
    let Some(receiver) = receiver else {
        return false;
    };
    [
        "NS", "UI", "CF", "CG", "CA", "CM", "AV", "WK", "SC", "SK", "MK", "CL", "PH", "Sec", "XCT",
    ]
    .iter()
    .any(|prefix| {
        receiver.starts_with(prefix)
            && receiver[prefix.len()..]
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_uppercase())
    })
}

/// Modules OTP ships with every Erlang release. A call through one is the
/// platform's, whatever the project declares.
fn otp_standard_module(module: &str) -> bool {
    matches!(
        module,
        "erlang"
            | "lists"
            | "maps"
            | "dict"
            | "sets"
            | "ordsets"
            | "orddict"
            | "gb_trees"
            | "gb_sets"
            | "queue"
            | "array"
            | "binary"
            | "string"
            | "io"
            | "io_lib"
            | "file"
            | "filename"
            | "filelib"
            | "os"
            | "timer"
            | "proplists"
            | "ets"
            | "dets"
            | "mnesia"
            | "gen_server"
            | "gen_statem"
            | "gen_event"
            | "gen_fsm"
            | "gen_tcp"
            | "gen_udp"
            | "gen_sctp"
            | "inet"
            | "ssl"
            | "supervisor"
            | "application"
            | "code"
            | "crypto"
            | "public_key"
            | "logger"
            | "error_logger"
            | "rand"
            | "math"
            | "re"
            | "unicode"
            | "calendar"
            | "base64"
            | "zlib"
            | "erl_eval"
            | "erl_scan"
            | "erl_parse"
            | "erl_anno"
            | "beam_lib"
            | "sys"
            | "persistent_term"
            | "counters"
            | "atomics"
            | "process"
            | "global"
            | "net_kernel"
            | "httpc"
            | "ssh"
            | "xmerl"
            | "ct"
            | "eunit"
    )
}

/// Modules Elixir ships with the language.
fn elixir_standard_module(module: &str) -> bool {
    matches!(
        module,
        "Kernel"
            | "Enum"
            | "Stream"
            | "Map"
            | "MapSet"
            | "List"
            | "Keyword"
            | "Tuple"
            | "Atom"
            | "String"
            | "Integer"
            | "Float"
            | "Range"
            | "Regex"
            | "IO"
            | "File"
            | "Path"
            | "URI"
            | "Base"
            | "Bitwise"
            | "Process"
            | "Task"
            | "Agent"
            | "GenServer"
            | "Supervisor"
            | "DynamicSupervisor"
            | "Registry"
            | "Application"
            | "System"
            | "Code"
            | "Module"
            | "Macro"
            | "Access"
            | "Date"
            | "Time"
            | "DateTime"
            | "NaiveDateTime"
            | "Calendar"
            | "Logger"
            | "Exception"
            | "Version"
            | "Port"
            | "Node"
            | "Function"
            | "Record"
            | "Protocol"
            | "Inspect"
            | "Enumerable"
            | "Collectable"
            | "String.Chars"
    )
}

/// Whether two languages share one set of symbols. A TypeScript project
/// with React components is written in two of them -- `.ts` and `.tsx` --
/// and every import from a module into a component crosses the line:
/// taxonomy resolved 32 of its 494 calls because of it, and every Next.js,
/// Remix or React project is shaped the same way.
pub(crate) fn languages_share_symbols(one: &str, other: &str) -> bool {
    one == other
        || matches!(
            (one, other),
            ("typescript", "tsx")
                | ("tsx", "typescript")
                | ("javascript", "jsx")
                | ("jsx", "javascript")
        )
}

/// A macro the compiler expands rather than a function anybody calls: a
/// `<script setup>` block is compiled, and `defineProps` is expanded
/// there. koel writes 361 of them. This is asked before resolution
/// because the macro wins even where the repository exports a function by
/// that name, which vue does.
pub(crate) fn environment_provides_call(language: &str, path: &str, label: &str) -> bool {
    matches!(language, "javascript" | "typescript" | "tsx")
        && path.ends_with(".vue")
        && matches!(
            label,
            "defineProps"
                | "defineEmits"
                | "defineExpose"
                | "defineOptions"
                | "defineSlots"
                | "defineModel"
                | "withDefaults"
        )
}

/// Names a test file gets from the runner it is written for, when nothing
/// in the project answered for them. `describe` and `expect` are a JS
/// runner's, `assertSame` and `shouldReceive` come to a PHP test case
/// through the class it extends and the doubles it builds, and `assertEq`
/// and `vm.` reach a Solidity test the same way. This is asked last, so a
/// project that writes an assertion helper of its own keeps its callers:
/// guzzle declares 27 and koel 32.
fn test_runner_provides_call(language: &str, path: &str, label: &str) -> bool {
    if !is_test_like_source_path(path) {
        return false;
    }
    match language {
        "php" => {
            // A label can name the class it goes through -- koel writes
            // `Song::factory` 829 times -- so the method is read off the
            // end. PHPUnit's mock builder and Laravel's factories and JSON
            // helpers account for 1747 of koel's 7342 unresolved php calls
            // and 257 of monolog's 1685.
            let method = label.rsplit("::").next().unwrap_or(label);
            label.starts_with("assert")
                || label.starts_with("expect")
                || matches!(
                    method,
                    "factory"
                        | "createOne"
                        | "createMany"
                        | "createQuietly"
                        | "makeOne"
                        | "makeMany"
                        | "getMock"
                        | "getMockForAbstractClass"
                        | "onlyMethods"
                        | "setMethods"
                        | "setConstructorArgs"
                        | "disableOriginalConstructor"
                        | "method"
                        | "once"
                        | "never"
                        | "atLeastOnce"
                        | "atMost"
                        | "exactly"
                        | "willReturnSelf"
                        | "willReturnMap"
                        | "willReturnOnConsecutiveCalls"
                        | "getJson"
                        | "postJson"
                        | "putJson"
                        | "patchJson"
                        | "deleteJson"
                        | "actingAs"
                        | "withoutExceptionHandling"
                        | "artisan"
                        | "freezeTime"
                )
                || matches!(
                    label,
                    "fail"
                        | "markTestSkipped"
                        | "markTestIncomplete"
                        | "createMock"
                        | "createStub"
                        | "createPartialMock"
                        | "getMockBuilder"
                        | "willReturn"
                        | "willReturnCallback"
                        | "willThrowException"
                        | "shouldReceive"
                        | "shouldNotReceive"
                        | "andReturn"
                        | "andThrow"
                        | "andReturnUsing"
                )
        }
        "solidity" => {
            matches!(
                label,
                "assertEq"
                    | "assertNotEq"
                    | "assertTrue"
                    | "assertFalse"
                    | "assertGt"
                    | "assertGe"
                    | "assertLt"
                    | "assertLe"
                    | "assertApproxEqAbs"
                    | "assertApproxEqRel"
                    | "bound"
                    | "deal"
                    | "hoax"
                    | "fail"
                    | "emit"
            ) || label.starts_with("vm.")
        }
        // busted hands a Lua spec its cases and its assertions, and kong
        // writes 1011 spec files: `describe`, `it`, `lazy_setup`,
        // `assert.same`.
        "lua" => {
            matches!(
                label,
                "describe"
                    | "it"
                    | "pending"
                    | "setup"
                    | "teardown"
                    | "lazy_setup"
                    | "lazy_teardown"
                    | "before_each"
                    | "after_each"
                    | "spy"
                    | "stub"
                    | "mock"
                    | "finally"
            ) || label == "assert"
                || label.starts_with("assert.")
                || label.starts_with("spy.")
                || label.starts_with("stub.")
                || label.starts_with("mock.")
        }
        // munit and ScalaCheck hand a Scala suite its cases and its
        // properties: cats writes `test`, `checkAll` and `forAll` 433
        // times in its own tests.
        "scala" => {
            matches!(
                label,
                "test"
                    | "property"
                    | "checkAll"
                    | "forAll"
                    | "assert"
                    | "assertEquals"
                    | "assertNotEquals"
                    | "assume"
                    | "intercept"
                    | "beforeAll"
                    | "afterAll"
            )
        }
        // RSpec hands a Ruby spec its cases, its hooks, its doubles and
        // its matchers, and not one of them is a method the project wrote:
        // 7366 of mastodon's 29668 unresolved ruby calls are these, led by
        // `expect`, `it`, `let` and `before`. minitest's assertions arrive
        // the same way, and `Fabricate` is the fabrication gem building a
        // record for a spec to use.
        // package:test hands a Dart suite its cases and its matchers, and
        // the `http` package writes 470 of them: `test`, `group`,
        // `expect`, `setUp`, `throwsA`.
        // testthat is R's harness and its vocabulary is closed: every
        // assertion is an `expect_*`, and the blocks around them are named
        // outright. dplyr writes 418 of these and every one sits under
        // `tests/`.
        "r" => {
            label.starts_with("expect_")
                || matches!(
                    label,
                    "test_that"
                        | "describe"
                        | "it"
                        | "expect"
                        | "skip"
                        | "skip_if"
                        | "skip_if_not"
                        | "skip_if_not_installed"
                        | "skip_on_cran"
                        | "skip_on_ci"
                        | "skip_on_os"
                        | "succeed"
                        | "fail"
                        | "verify_output"
                        | "local_edition"
                        | "local_reproducible_output"
                        | "with_mocked_bindings"
                        | "local_mocked_bindings"
                )
        }
        "dart" => {
            matches!(
                label,
                "test"
                    | "group"
                    | "setUp"
                    | "tearDown"
                    | "setUpAll"
                    | "tearDownAll"
                    | "expect"
                    | "expectLater"
                    | "fail"
                    | "skip"
                    | "throwsA"
                    | "isA"
                    | "equals"
                    | "predicate"
                    | "anyOf"
                    | "allOf"
                    | "completion"
                    | "emits"
                    | "emitsInOrder"
                    | "emitsDone"
                    | "returnsNormally"
                    | "isNot"
                    | "startsWith"
                    | "endsWith"
                    | "hasLength"
                    | "addTearDown"
                    | "registerException"
                    | "markTestSkipped"
                    | "verify"
                    | "captureAny"
            )
        }
        // XCTest hands a Swift suite its assertions and its waiting, and
        // Alamofire writes 2559 of them -- 59% of everything unresolved in
        // its swift -- led by XCTAssertEqual, expectation, fulfill and
        // waitForExpectations.
        // kotlin.test hands a Kotlin suite its assertions and AssertJ or
        // Truth the chain that reads them, and okio writes 1791 of those --
        // 45% of everything unresolved in its kotlin. The one `assertEquals`
        // okio declares is a private helper in a sample, and a call that
        // reaches a definition never gets here.
        // A python test case gets its assertions from unittest.TestCase,
        // the way a PHPUnit one does: django-oscar writes `self.assertEqual`
        // 841 times and `self.assertTrue` 314, 1717 calls in all, none of
        // them a method the project declares. pytest's own module is named
        // outright. A project that writes its assertions with the `assert`
        // statement, as flask and requests do, has nothing here to find.
        // Shouldly, xUnit and Moq hand a C# suite its assertions, its
        // expectations and its doubles, and Polly writes 5762 of them --
        // 67% of everything unresolved in its csharp -- led by
        // `Should.Throw` and the `ShouldBe` that reads it. The name a
        // project gives its own shim still wins: Newtonsoft declares
        // XUnitAssert and its 2775 `Assert.AreEqual` calls reach it,
        // because a call that resolves never asks this.
        "csharp" => {
            let tail = label.rsplit('.').next().unwrap_or(label);
            tail.starts_with("Should")
                || label.starts_with("Should.")
                || tail.starts_with("Assert")
                || label.starts_with("Assert.")
                || label.starts_with("Mock")
                || matches!(
                    tail,
                    "Returns" | "ReturnsAsync" | "Verify" | "Setup" | "Throws" | "ThrowsAsync"
                )
        }
        "python" => {
            label.starts_with("self.assert")
                || label.starts_with("pytest.")
                || matches!(
                    label,
                    "self.fail" | "self.skipTest" | "self.subTest" | "self.addCleanup"
                )
        }
        "kotlin" => {
            label.starts_with("assert")
                || matches!(
                    label,
                    "fail"
                        | "assertThat"
                        | "isEqualTo"
                        | "isNotEqualTo"
                        | "isTrue"
                        | "isFalse"
                        | "isNull"
                        | "isNotNull"
                        | "isEmpty"
                        | "isNotEmpty"
                        | "hasSize"
                        | "containsExactly"
                        | "isInstanceOf"
                        | "hasMessage"
                        | "hasMessageThat"
                        | "isGreaterThan"
                        | "isLessThan"
                        | "isSameInstanceAs"
                        | "runTest"
                        | "runBlockingTest"
                )
        }
        "swift" => {
            label.starts_with("XCTAssert")
                || matches!(
                    label,
                    "XCTFail"
                        | "XCTUnwrap"
                        | "XCTSkip"
                        | "XCTSkipIf"
                        | "XCTSkipUnless"
                        | "expectation"
                        | "fulfill"
                        | "waitForExpectations"
                        | "wait"
                        | "measure"
                        | "addTeardownBlock"
                        | "setUp"
                        | "tearDown"
                        | "setUpWithError"
                        | "tearDownWithError"
                )
        }
        "ruby" => {
            matches!(
                label,
                "describe"
                    | "context"
                    | "xdescribe"
                    | "xcontext"
                    | "fdescribe"
                    | "feature"
                    | "scenario"
                    | "it"
                    | "specify"
                    | "example"
                    | "xit"
                    | "fit"
                    | "pending"
                    | "skip"
                    | "before"
                    | "after"
                    | "around"
                    | "let"
                    | "let!"
                    | "subject"
                    | "subject!"
                    | "expect"
                    | "is_expected"
                    | "should"
                    | "should_not"
                    | "to"
                    | "to_not"
                    | "not_to"
                    | "eq"
                    | "eql"
                    | "equal"
                    | "be_a"
                    | "be_an"
                    | "be_within"
                    | "contain_exactly"
                    | "match_array"
                    | "have_attributes"
                    | "have_received"
                    | "have_http_status"
                    | "raise_error"
                    | "change"
                    | "satisfy"
                    | "start_with"
                    | "end_with"
                    | "double"
                    | "instance_double"
                    | "class_double"
                    | "spy"
                    | "allow"
                    | "receive"
                    | "and_return"
                    | "and_raise"
                    | "and_call_original"
                    | "stub_request"
                    | "shared_examples"
                    | "shared_context"
                    | "include_examples"
                    | "it_behaves_like"
                    | "it_should_behave_like"
                    | "travel_to"
                    | "freeze_time"
                    | "Fabricate"
                    | "assert"
                    | "refute"
            ) || label.starts_with("assert_")
                || label.starts_with("refute_")
        }
        "javascript" | "typescript" | "tsx" => {
            matches!(
                label,
                "describe"
                    | "suite"
                    | "it"
                    | "test"
                    | "expect"
                    | "beforeEach"
                    | "afterEach"
                    | "beforeAll"
                    | "afterAll"
                    | "before"
                    | "after"
            ) || ["vi.", "jest.", "expect.", "describe.", "it.", "test."]
                .iter()
                .any(|prefix| label.starts_with(prefix))
                // `expect` was here and the matchers that read it were not,
                // which is most of what a suite actually writes: `toBe`,
                // `toEqual`, `toHaveBeenCalledWith`, `toThrow`. 3271 across
                // core, koel, zod and openzeppelin. chai reads the same way
                // through `to`, which is how openzeppelin writes
                // `to.be.revertedWithCustomError` and `withArgs`.
                || [
                    "toBe",
                    "toHave",
                    "toEqual",
                    "toMatch",
                    "toThrow",
                    "toContain",
                    "toStrict",
                    "toReturn",
                    "toSatisfy",
                    "toNot",
                ]
                .iter()
                .any(|matcher| {
                    label
                        .rsplit('.')
                        .next()
                        .unwrap_or(label)
                        .starts_with(matcher)
                })
                || label.starts_with("to.")
                || label.ends_with(".withArgs")
                || label == "withArgs"
        }
        _ => false,
    }
}

/// Whether a call names a module the language ships, when nothing in the
/// project answered for it. This is asked only after resolution has
/// failed, because a project may declare a module of the same name and
/// mean its own: dune writes `String.` and `List.` for the modules its
/// own `stdune` library defines, and 19,897 of its qualified calls
/// resolve into the project that way.
fn standard_library_module_call(language: &str, label: &str) -> bool {
    let Some((module, _)) = label.split_once('.') else {
        return false;
    };
    match language {
        "ocaml" => matches!(
            module,
            "Stdlib"
                | "Printf"
                | "Format"
                | "Scanf"
                | "Printexc"
                | "Sys"
                | "Unix"
                | "Filename"
                | "Arg"
                | "Buffer"
                | "Bytes"
                | "Char"
                | "Digest"
                | "Either"
                | "Fun"
                | "Gc"
                | "Hashtbl"
                | "In_channel"
                | "Int32"
                | "Int64"
                | "Lazy"
                | "Marshal"
                | "Mutex"
                | "Nativeint"
                | "Obj"
                | "Out_channel"
                | "Random"
                | "Seq"
                | "Stack"
                | "Str"
                | "Uchar"
                | "Weak"
        ),
        // Julia's `Base` and `Core` are open in every module, and nothing
        // outside the language declares them.
        "julia" => matches!(module, "Base" | "Core"),
        _ => false,
    }
}

/// Ruby's own methods on Object, String, Array, Hash and Enumerable. 4434
/// of mastodon's 22033 unresolved ruby calls are these -- `new` 672,
/// `to_s` 350, `map` 347, `each` 298 -- and none is a method the project
/// wrote. This is asked only where the choice is between `builtin` and
/// `unresolved`, never earlier: a ruby call written through a constant the
/// project never declares is a gem's and is filed as one, and letting
/// these names past that rule handed 107 of them to a same-named
/// definition of the project's own, which is the mistake that rule exists
/// to prevent. ActiveSupport's `present?` and `blank?` are left out, being
/// a gem's rather than the language's.
fn ruby_core_method(label: &str) -> bool {
    matches!(
        label,
        "new"
            | "allocate"
            | "to_s"
            | "to_i"
            | "to_f"
            | "to_a"
            | "to_h"
            | "to_sym"
            | "to_proc"
            | "map"
            | "flat_map"
            | "filter_map"
            | "each"
            | "each_with_index"
            | "each_with_object"
            | "select"
            | "reject"
            | "find"
            | "detect"
            | "reduce"
            | "inject"
            | "partition"
            | "group_by"
            | "sort"
            | "sort_by"
            | "min"
            | "max"
            | "min_by"
            | "max_by"
            | "sum"
            | "tally"
            | "zip"
            | "flatten"
            | "compact"
            | "uniq"
            | "reverse"
            | "size"
            | "length"
            | "count"
            | "first"
            | "last"
            | "keys"
            | "values"
            | "fetch"
            | "dig"
            | "merge"
            | "store"
            | "delete"
            | "clear"
            | "slice"
            | "push"
            | "pop"
            | "shift"
            | "unshift"
            | "concat"
            | "join"
            | "split"
            | "strip"
            | "chomp"
            | "upcase"
            | "downcase"
            | "capitalize"
            | "gsub"
            | "sub"
            | "match"
            | "match?"
            | "start_with?"
            | "end_with?"
            | "nil?"
            | "empty?"
            | "any?"
            | "all?"
            | "none?"
            | "one?"
            | "include?"
            | "key?"
            | "has_key?"
            | "member?"
            | "dup"
            | "clone"
            | "hash"
            | "inspect"
            | "itself"
            | "tap"
            | "then"
            | "send"
            | "public_send"
            | "respond_to?"
            | "is_a?"
            | "kind_of?"
            | "instance_of?"
            | "times"
            | "upto"
            | "downto"
            | "step"
            | "abs"
            | "round"
            | "floor"
            | "ceil"
            | "zero?"
            | "positive?"
            | "negative?"
            | "even?"
            | "odd?"
    )
}

pub(crate) fn builtin_call_target(language: &str, label: &str) -> bool {
    // PHP writes `\count(..)` to mean the global function rather than one
    // the current namespace might define, and monolog writes 273 of its
    // standard-library calls that way.
    let base = label.trim_end_matches('!').trim_start_matches('\\');
    match language {
        "rust" => {
            matches!(
                base,
                "Some"
                    | "None"
                    | "Ok"
                    | "Err"
                    | "Box::new"
                    | "String::from"
                    | "String::new"
                    | "Vec::new"
                    | "Default::default"
                    | "drop"
                    | "format"
                    | "format_args"
                    | "print"
                    | "println"
                    | "eprint"
                    | "eprintln"
                    | "write"
                    | "writeln"
                    | "panic"
                    | "assert"
                    | "assert_eq"
                    | "assert_ne"
                    | "debug_assert"
                    | "debug_assert_eq"
                    | "debug_assert_ne"
                    | "vec"
                    | "matches"
                    | "todo"
                    | "unimplemented"
                    | "unreachable"
                    | "dbg"
                    | "include_str"
                    | "include_bytes"
                    | "concat"
                    | "stringify"
                    | "env"
                    | "option_env"
            ) || base.starts_with("std::")
        }
        "python" => matches!(
            base,
            "print"
                | "len"
                | "range"
                | "str"
                | "int"
                | "float"
                | "bool"
                | "list"
                | "dict"
                | "set"
                | "tuple"
                | "enumerate"
                | "zip"
                | "map"
                | "filter"
                | "sorted"
                | "reversed"
                | "isinstance"
                | "issubclass"
                | "super"
                | "open"
                | "type"
                | "getattr"
                | "setattr"
                | "hasattr"
                | "repr"
                | "min"
                | "max"
                | "sum"
                | "abs"
                | "round"
                | "any"
                | "all"
                | "next"
                | "iter"
                | "format"
                | "id"
                | "vars"
                | "hash"
                | "callable"
                | "bytes"
                | "bytearray"
                | "frozenset"
                | "object"
                | "property"
                | "staticmethod"
                | "classmethod"
                | "dir"
                | "input"
                | "globals"
                | "locals"
                | "eval"
                | "exec"
                | "compile"
                | "chr"
                | "ord"
                | "hex"
                | "oct"
                | "bin"
                | "divmod"
                | "pow"
                | "slice"
                | "complex"
                // `raise ValueError(...)` reads as a call to a name the
                // language itself provides, and reporting the whole
                // exception hierarchy as unresolved suggested a resolver
                // that had failed.
                | "Exception"
                | "BaseException"
                | "ValueError"
                | "TypeError"
                | "RuntimeError"
                | "KeyError"
                | "IndexError"
                | "AttributeError"
                | "NotImplementedError"
                | "AssertionError"
                | "StopIteration"
                | "StopAsyncIteration"
                | "OSError"
                | "IOError"
                | "ImportError"
                | "ModuleNotFoundError"
                | "ZeroDivisionError"
                | "ArithmeticError"
                | "OverflowError"
                | "NameError"
                | "UnboundLocalError"
                | "LookupError"
                | "MemoryError"
                | "RecursionError"
                | "SystemExit"
                | "SystemError"
                | "KeyboardInterrupt"
                | "GeneratorExit"
                | "FileNotFoundError"
                | "FileExistsError"
                | "PermissionError"
                | "IsADirectoryError"
                | "NotADirectoryError"
                | "InterruptedError"
                | "TimeoutError"
                | "ConnectionError"
                | "ConnectionResetError"
                | "ConnectionRefusedError"
                | "ConnectionAbortedError"
                | "BrokenPipeError"
                | "EOFError"
                | "SyntaxError"
                | "IndentationError"
                | "UnicodeError"
                | "UnicodeDecodeError"
                | "UnicodeEncodeError"
                | "ReferenceError"
                | "BufferError"
                | "Warning"
                | "UserWarning"
                | "DeprecationWarning"
                | "PendingDeprecationWarning"
                | "RuntimeWarning"
                | "FutureWarning"
                | "ResourceWarning"
        ),
        "javascript" | "typescript" | "tsx" => {
            matches!(
                base,
                "parseInt"
                    | "parseFloat"
                    | "isNaN"
                    | "isFinite"
                    | "String"
                    | "Number"
                    | "Boolean"
                    | "Array"
                    | "Symbol"
                    | "BigInt"
                    | "Error"
                    | "TypeError"
                    | "RangeError"
                    | "Promise"
                    | "Date"
                    | "Map"
                    | "Set"
                    | "WeakMap"
                    | "WeakSet"
                    | "Proxy"
                    | "Reflect"
                    | "setTimeout"
                    | "setInterval"
                    | "clearTimeout"
                    | "clearInterval"
                    | "queueMicrotask"
                    | "structuredClone"
                    | "encodeURIComponent"
                    | "decodeURIComponent"
                    | "encodeURI"
                    | "decodeURI"
                    | "fetch"
                    | "alert"
                    | "confirm"
                    | "RegExp"
                    | "URL"
                    | "URLSearchParams"
                    | "AbortController"
                    | "TextEncoder"
                    | "TextDecoder"
                    | "Function"
                    | "Object"
                    | "SyntaxError"
                    | "ReferenceError"
                    | "EvalError"
                    | "URIError"
                    | "AggregateError"
                    // CommonJS: the module loader is part of the runtime,
                    // not something the repository declares.
                    | "require"
            ) || [
                "console.", "Math.", "JSON.", "Object.", "Array.", "Number.", "String.",
                "Promise.", "Reflect.", "Date.", "Symbol.",
            ]
            .iter()
            .any(|prefix| base.starts_with(prefix))
        }
        // Go's predeclared types are written as calls when a value is
        // converted -- `string(b)`, `int64(n)`, `[]byte` aside. terraform
        // writes 400 of those, and reporting them as unresolved calls claims
        // the scan looked for a function that was never meant to exist.
        "go" => matches!(
            base,
            "make"
                | "len"
                | "cap"
                | "append"
                | "new"
                | "copy"
                | "delete"
                | "panic"
                | "recover"
                | "print"
                | "println"
                | "close"
                | "complex"
                | "real"
                | "imag"
                | "min"
                | "max"
                | "clear"
                | "string"
                | "bool"
                | "byte"
                | "rune"
                | "error"
                | "any"
                | "int"
                | "int8"
                | "int16"
                | "int32"
                | "int64"
                | "uint"
                | "uint8"
                | "uint16"
                | "uint32"
                | "uint64"
                | "uintptr"
                | "float32"
                | "float64"
                | "complex64"
                | "complex128"
        ),
        "c" | "cpp" => {
            matches!(
                base,
                "printf"
                    | "fprintf"
                    | "sprintf"
                    | "snprintf"
                    | "scanf"
                    | "sscanf"
                    | "puts"
                    | "putchar"
                    | "getchar"
                    | "malloc"
                    | "calloc"
                    | "realloc"
                    | "free"
                    | "memcpy"
                    | "memmove"
                    | "memset"
                    | "memcmp"
                    | "strlen"
                    | "strcmp"
                    | "strncmp"
                    | "strcpy"
                    | "strncpy"
                    | "strcat"
                    | "strstr"
                    | "strchr"
                    | "sizeof"
                    | "assert"
                    | "exit"
                    | "abort"
                    | "atoi"
                    | "atof"
                    | "fopen"
                    | "fclose"
                    | "fread"
                    | "fwrite"
                    | "fgets"
                    | "fputs"
                    // A cast is written like a call and is the language's
                    // own: nlohmann/json writes 437 `static_cast` and 70
                    // `reinterpret_cast`, and reporting them as unresolved
                    // claims the scan looked for a function nobody wrote.
                    | "static_cast"
                    | "dynamic_cast"
                    | "const_cast"
                    | "reinterpret_cast"
                    | "alignof"
                    | "typeid"
                    | "decltype"
                    | "offsetof"
                    | "va_start"
                    | "va_end"
                    | "va_arg"
                    | "strcasecmp"
                    | "strncasecmp"
                    | "snprintf_s"
                    | "qsort"
                    | "bsearch"
                    | "abs"
                    | "labs"
                    | "strtol"
                    | "strtoul"
                    | "strtod"
                    | "isdigit"
                    | "isalpha"
                    | "isspace"
                    | "toupper"
                    | "tolower"
            ) || base.starts_with("std::")
        }
        "php" => matches!(
            base,
            "print"
                | "printf"
                | "sprintf"
                | "echo"
                | "strlen"
                | "count"
                | "isset"
                | "empty"
                | "unset"
                | "array"
                | "implode"
                | "explode"
                | "in_array"
                | "array_map"
                | "array_filter"
                | "array_merge"
                | "array_keys"
                | "array_values"
                | "json_encode"
                | "json_decode"
                | "intval"
                | "floatval"
                | "strval"
                | "is_array"
                | "is_string"
                | "is_null"
                | "is_numeric"
        ),
        // Kernel and Object: what every Ruby object answers, and what the
        // language itself writes as a call. sinatra's 1234 unresolved calls
        // were led by `super`, `raise`, `nil?` and `block_given?`, none of
        // which any project declares. Collection methods (`first`, `map`,
        // `include?`) are left out: a project defines those on its own
        // types as readily as the core library does.
        // The framework every C# program compiles against. Polly's 11759
        // unresolved calls were led by `TimeSpan.FromSeconds`, `nameof` and
        // `ArgumentNullException` — the platform, not the project. Test
        // libraries (Shouldly, xunit) are dependencies and stay out.
        "csharp" => match base.rsplit_once('.') {
            // The receiver has to be the type itself: `TimeSpan.FromSeconds`
            // names one, while `args.Outcome.Exception.GetType` only ends in
            // a property that shares a type's name.
            Some((receiver, _)) => {
                csharp_platform_type(receiver)
                    || receiver
                        .strip_prefix("System.")
                        .is_some_and(csharp_platform_type)
            }
            None => {
                csharp_platform_type(base)
                    || matches!(base, "nameof" | "typeof" | "sizeof" | "default")
            }
        },
        // Swift's own functions and the Foundation types it ships with.
        "swift" => matches!(
            base,
            "print"
                | "debugPrint"
                | "dump"
                | "NSLog"
                | "assert"
                | "assertionFailure"
                | "precondition"
                | "preconditionFailure"
                | "fatalError"
                | "abs"
                | "min"
                | "max"
                | "swap"
                | "zip"
                | "stride"
                | "String"
                | "Int"
                | "Double"
                | "Float"
                | "Bool"
                | "Data"
                | "Date"
                | "URL"
                | "URLRequest"
                | "UUID"
                | "NSError"
                | "IndexPath"
        ),
        // Objective-C sends every message through the runtime, so the
        // names NSObject and the frameworks answer to look exactly like a
        // project's own methods. AFNetworking's 2136 unresolved calls are
        // led by `alloc`, `class`, XCTest's assertions and Foundation's
        // free functions.
        "objc" => {
            matches!(
                base,
                // What every object answers, whatever it is.
                "alloc"
                    | "new"
                    | "init"
                    | "self"
                    | "class"
                    | "superclass"
                    | "copy"
                    | "mutableCopy"
                    | "retain"
                    | "release"
                    | "autorelease"
                    | "dealloc"
                    | "hash"
                    | "description"
                    | "debugDescription"
                    | "load"
                    | "initialize"
                    | "isEqual:"
                    | "isKindOfClass:"
                    | "isMemberOfClass:"
                    | "respondsToSelector:"
                    | "conformsToProtocol:"
                    | "performSelector:"
                    | "performSelector:withObject:"
                    | "performSelector:withObject:afterDelay:"
                    | "methodSignatureForSelector:"
                    | "forwardInvocation:"
                    | "valueForKey:"
                    | "setValue:forKey:"
                    | "valueForKeyPath:"
                    | "setValue:forKeyPath:"
                    // The test framework, which a test file calls as
                    // often as it calls the code under test.
                    | "XCTAssert"
                    | "XCTAssertTrue"
                    | "XCTAssertFalse"
                    | "XCTAssertNil"
                    | "XCTAssertNotNil"
                    | "XCTAssertEqual"
                    | "XCTAssertNotEqual"
                    | "XCTAssertEqualObjects"
                    | "XCTAssertNotEqualObjects"
                    | "XCTAssertEqualWithAccuracy"
                    | "XCTAssertGreaterThan"
                    | "XCTAssertGreaterThanOrEqual"
                    | "XCTAssertLessThan"
                    | "XCTAssertLessThanOrEqual"
                    | "XCTAssertThrows"
                    | "XCTAssertThrowsSpecific"
                    | "XCTAssertNoThrow"
                    | "XCTFail"
                    | "XCTSkip"
                    | "XCTSkipIf"
                    | "XCTSkipUnless"
                    | "expectationWithDescription:"
                    | "expectationForNotification:object:handler:"
                    | "expectationForPredicate:evaluatedWithObject:handler:"
                    | "waitForExpectationsWithTimeout:handler:"
                    | "keyValueObservingExpectationForObject:keyPath:handler:"
                    | "measureBlock:"
                    | "fulfill"
                    // Assertion macros Foundation defines.
                    | "NSAssert"
                    | "NSCAssert"
                    | "NSParameterAssert"
                    | "NSCParameterAssert"
                    | "NSLog"
            ) || objc_platform_function(base)
        }
        "ruby" => matches!(
            base,
            "super"
                | "raise"
                | "fail"
                | "catch"
                | "throw"
                | "loop"
                | "lambda"
                | "proc"
                | "block_given?"
                | "binding"
                | "caller"
                | "sleep"
                | "rand"
                | "srand"
                | "format"
                | "sprintf"
                | "printf"
                | "puts"
                | "print"
                | "p"
                | "pp"
                | "gets"
                | "exit"
                | "abort"
                | "at_exit"
                | "load"
                | "freeze"
                | "frozen?"
                | "dup"
                | "clone"
                | "tap"
                | "then"
                | "itself"
                | "nil?"
                | "is_a?"
                | "kind_of?"
                | "instance_of?"
                | "respond_to?"
                | "equal?"
                | "eql?"
                | "object_id"
                | "inspect"
                | "class"
                | "send"
                | "__send__"
                | "public_send"
                | "instance_variable_get"
                | "instance_variable_set"
                | "instance_variables"
                | "define_method"
                | "attr_accessor"
                | "attr_reader"
                | "attr_writer"
                | "extend"
                | "include"
                | "prepend"
                | "module_function"
                | "private"
                | "public"
                | "protected"
                | "alias_method"
        ),
        "bash" => matches!(
            base,
            "echo"
                | "printf"
                | "cd"
                | "exit"
                | "export"
                | "source"
                | "test"
                | "read"
                | "local"
                | "set"
                | "unset"
                | "shift"
                | "eval"
                | "exec"
                | "trap"
                | "return"
                | "wait"
                | "kill"
                | "true"
                | "false"
                // The rest of the shell's own vocabulary. `pwd`, `:`,
                // `break`, `continue` and `command` alone account for 54 of
                // redis's 424 unresolved shell calls, and reporting them
                // says the resolver failed to find a function no project
                // ever wrote.
                | ":"
                | "pwd"
                | "break"
                | "continue"
                | "command"
                | "builtin"
                | "declare"
                | "typeset"
                | "readonly"
                | "shopt"
                | "type"
                | "hash"
                | "let"
                | "getopts"
                | "umask"
                | "ulimit"
                | "times"
                | "jobs"
                | "fg"
                | "bg"
                | "disown"
                | "suspend"
                | "logout"
                | "caller"
                | "alias"
                | "unalias"
                | "pushd"
                | "popd"
                | "dirs"
                | "mapfile"
                | "readarray"
                | "bind"
                | "enable"
                | "complete"
                | "compgen"
                | "compopt"
                | "history"
        ),
        "dart" => matches!(base, "print" | "identical" | "assert"),
        // Nix names its own vocabulary outright: everything under
        // `builtins.` is the evaluator's, and a handful more sit in the
        // global scope. home-manager writes 477 of the first and 321 of
        // the second, and reporting them as unresolved says the resolver
        // failed to find a function no project ever wrote.
        "nix" => {
            base.starts_with("builtins.")
                || matches!(
                    base,
                    "abort"
                        | "baseNameOf"
                        | "builtins"
                        | "derivation"
                        | "derivationStrict"
                        | "dirOf"
                        | "fetchGit"
                        | "fetchMercurial"
                        | "fetchTarball"
                        | "fetchTree"
                        | "fetchurl"
                        | "fromTOML"
                        | "import"
                        | "isNull"
                        | "map"
                        | "placeholder"
                        | "removeAttrs"
                        | "scopedImport"
                        | "throw"
                        | "toString"
                )
        }
        // R's base package is attached in every session, so `c`, `length`, and
        // `UseMethod` are the language. Names from rlang (`abort`, `enquo`,
        // `caller_env`) look just as ubiquitous in modern R code but come from
        // a dependency, and calling those builtin would be a lie.
        "r" => matches!(
            base,
            "c" | "length"
                | "names"
                | "return"
                | "invisible"
                | "missing"
                | "identical"
                | "inherits"
                | "attr"
                | "attributes"
                | "class"
                | "UseMethod"
                | "NextMethod"
                | "structure"
                | "unlist"
                | "lapply"
                | "sapply"
                | "vapply"
                | "mapply"
                | "do.call"
                | "Recall"
                | "paste"
                | "paste0"
                | "sprintf"
                | "format"
                | "nchar"
                | "substr"
                | "sub"
                | "gsub"
                | "grepl"
                | "seq"
                | "seq_len"
                | "seq_along"
                | "rep"
                | "which"
                | "any"
                | "all"
                | "setdiff"
                | "union"
                | "intersect"
                | "unique"
                | "sort"
                | "order"
                | "rev"
                | "match"
                | "list"
                | "vector"
                | "matrix"
                | "data.frame"
                | "nrow"
                | "ncol"
                | "dim"
                | "stop"
                | "warning"
                | "message"
                | "stopifnot"
                | "tryCatch"
                | "on.exit"
                | "print"
                | "cat"
                | "is.null"
                | "is.na"
                | "is.function"
                | "is.character"
                | "is.numeric"
                | "is.logical"
                | "is.list"
                | "as.character"
                | "as.numeric"
                | "as.integer"
                | "as.logical"
                | "as.list"
                | "nargs"
                | "sys.call"
                | "sys.function"
                | "match.arg"
                | "match.call"
        ),
        // Elixir's Kernel is imported into every module automatically, and its
        // guards are the most-called names in any Elixir file. Erlang's BIFs
        // are the same story without the import: `self()` and `element/2` are
        // the language, not something a project declares.
        // Lua's standard library is a set of globals; a project that
        // defines its own `type` still wins, because a name that resolves
        // locally never reaches this point.
        "lua" => matches!(
            base,
            "assert"
                | "collectgarbage"
                | "dofile"
                | "error"
                | "getmetatable"
                | "ipairs"
                | "load"
                | "loadfile"
                | "loadstring"
                | "next"
                | "pairs"
                | "pcall"
                | "print"
                | "rawequal"
                | "rawget"
                | "rawlen"
                | "rawset"
                | "require"
                | "select"
                | "setmetatable"
                | "tonumber"
                | "tostring"
                | "type"
                | "unpack"
                | "xpcall"
        ),
        // OCaml's Stdlib is opened by default, so its constructors and
        // functions read as unqualified calls — the same class as Rust's
        // `Some`/`Ok`, which this list already covers.
        "ocaml" => matches!(
            base,
            "Some"
                | "None"
                | "Ok"
                | "Error"
                | "ref"
                | "raise"
                | "failwith"
                | "invalid_arg"
                | "ignore"
                | "fst"
                | "snd"
                | "not"
                | "incr"
                | "decr"
                | "succ"
                | "pred"
                | "abs"
                | "min"
                | "max"
                | "compare"
                | "print_string"
                | "print_endline"
                | "print_newline"
                | "print_int"
                | "prerr_endline"
                | "sprintf"
                | "printf"
                | "eprintf"
                | "fprintf"
                | "string_of_int"
                | "int_of_string"
                | "string_of_float"
                | "float_of_string"
                | "char_of_int"
                | "int_of_char"
        ),
        // Haskell's Prelude is in scope without an import.
        "haskell" => matches!(
            base,
            "return"
                | "pure"
                | "fmap"
                | "mapM"
                | "mapM_"
                | "forM"
                | "forM_"
                | "sequence"
                | "sequence_"
                | "when"
                | "unless"
                | "maybe"
                | "either"
                | "fromMaybe"
                | "fromJust"
                | "isJust"
                | "isNothing"
                | "Just"
                | "Nothing"
                | "Left"
                | "Right"
                | "error"
                | "show"
                | "read"
                | "print"
                | "putStr"
                | "putStrLn"
                | "getLine"
                | "id"
                | "const"
                | "flip"
                | "not"
                | "and"
                | "or"
                | "any"
                | "all"
                | "elem"
                | "notElem"
                | "null"
                | "length"
                | "head"
                | "last"
                | "tail"
                | "init"
                | "reverse"
                | "map"
                | "filter"
                | "concat"
                | "concatMap"
                | "foldr"
                | "foldl"
                | "zip"
                | "zipWith"
                | "unzip"
                | "lookup"
                | "fst"
                | "snd"
                | "take"
                | "drop"
                | "span"
                | "words"
                | "unwords"
                | "lines"
                | "unlines"
                | "replicate"
                | "fromIntegral"
                | "realToFrac"
                | "otherwise"
        ),
        // scala.Predef and the collection companions are in scope
        // everywhere. Test-framework names (ScalaCheck's `forAll`) are not
        // the language's, so they stay unresolved rather than pretending
        // to be builtin.
        "scala" => matches!(
            base,
            "Some"
                | "None"
                | "Option"
                | "Left"
                | "Right"
                | "Seq"
                | "List"
                | "Set"
                | "Map"
                | "Vector"
                | "Array"
                | "Nil"
                | "println"
                | "print"
                | "require"
                | "implicitly"
                | "identity"
                | "sys"
        ),
        // Kotlin's stdlib builders and the JDK exceptions it wraps.
        "kotlin" => matches!(
            base,
            "listOf"
                | "mutableListOf"
                | "arrayOf"
                | "arrayOfNulls"
                | "byteArrayOf"
                | "intArrayOf"
                | "mapOf"
                | "mutableMapOf"
                | "setOf"
                | "mutableSetOf"
                | "emptyList"
                | "emptyMap"
                | "emptySet"
                | "ByteArray"
                | "IntArray"
                | "CharArray"
                | "StringBuilder"
                | "require"
                | "requireNotNull"
                | "check"
                | "checkNotNull"
                | "error"
                | "TODO"
                | "println"
                | "print"
                | "lazy"
                | "run"
                | "with"
                | "IOException"
                | "FileNotFoundException"
                | "IllegalArgumentException"
                | "IllegalStateException"
                | "RuntimeException"
                | "Exception"
                | "AssertionError"
        ),
        // java.lang is imported implicitly and java.util comes with the
        // platform. Assertion libraries (Truth, AssertJ) are dependencies,
        // not the language, and are left out on purpose.
        "java" => matches!(
            base,
            "ArrayList"
                | "LinkedList"
                | "HashMap"
                | "LinkedHashMap"
                | "TreeMap"
                | "HashSet"
                | "LinkedHashSet"
                | "TreeSet"
                | "ArrayDeque"
                | "StringBuilder"
                | "StringBuffer"
                | "StringWriter"
                | "StringReader"
                | "Thread"
                | "Object"
                | "Integer"
                | "Long"
                | "Double"
                | "Float"
                | "Short"
                | "Byte"
                | "Character"
                | "Boolean"
                | "String"
                | "Math"
                | "System"
                | "Objects"
                | "Arrays"
                | "Collections"
                | "Optional"
                | "requireNonNull"
                | "getClass"
                | "hashCode"
                | "toString"
                | "equals"
                | "Exception"
                | "RuntimeException"
                | "IllegalArgumentException"
                | "IllegalStateException"
                | "UnsupportedOperationException"
                | "NullPointerException"
                | "IndexOutOfBoundsException"
                | "NumberFormatException"
                | "AssertionError"
                | "IOException"
                | "InterruptedException"
        ),
        // A call into OTP or Elixir's standard library is the platform's:
        // cowboy calls `gen_tcp:recv` 283 times and `lists:keyfind` 238,
        // ecto `Enum.reverse` 79 and `Enum.reduce` 72, and reporting those
        // as unresolved reads as a resolver that failed rather than a
        // dependency that was never in this repository.
        "erlang" if base.contains(':') => base
            .split_once(':')
            .is_some_and(|(module, _)| otp_standard_module(module)),
        "elixir" if base.contains('.') => base
            .split_once('.')
            .is_some_and(|(module, _)| elixir_standard_module(module)),
        // Zig reaches its standard library through the constant a file
        // binds with `@import("std")`, and `builtin` is the compiler's own
        // description of the build. 775 of zls's 2955 unresolved calls are
        // `std.` -- `std.debug.assert`, `std.ArrayList` -- and not one of
        // them is a function zls failed to declare.
        "zig" => base.starts_with("std.") || base.starts_with("builtin."),
        // Solidity's own: `require`/`revert` state a condition the call
        // has to meet, `keccak256` and `ecrecover` are the chain's
        // primitives, and `abi.` is how a contract encodes what it sends.
        "solidity" => {
            matches!(
                base,
                "require"
                    | "revert"
                    | "assert"
                    | "keccak256"
                    | "sha256"
                    | "ripemd160"
                    | "ecrecover"
                    | "addmod"
                    | "mulmod"
                    | "selfdestruct"
                    | "blockhash"
                    | "blobhash"
                    | "gasleft"
                    | "type"
                    | "payable"
            ) || [
                "abi.",
                "bytes.concat",
                "string.concat",
                "msg.",
                "block.",
                "tx.",
            ]
            .iter()
            .any(|prefix| base.starts_with(prefix))
        }
        "elixir" | "erlang" => matches!(
            base.trim_end_matches('?'),
            "is_atom"
                | "is_binary"
                | "is_bitstring"
                | "is_boolean"
                | "is_exception"
                | "is_float"
                | "is_function"
                | "is_integer"
                | "is_list"
                | "is_map"
                | "is_nil"
                | "is_number"
                | "is_pid"
                | "is_port"
                | "is_reference"
                | "is_struct"
                | "is_tuple"
                | "abs"
                | "apply"
                | "binary_part"
                | "bit_size"
                | "byte_size"
                | "div"
                | "elem"
                | "erase"
                | "error"
                | "exit"
                | "hd"
                | "inspect"
                | "iolist_size"
                | "iolist_to_binary"
                | "length"
                | "link"
                | "make_ref"
                | "map_size"
                | "max"
                | "min"
                | "monitor"
                | "node"
                | "nodes"
                | "process_flag"
                | "raise"
                | "reraise"
                | "rem"
                | "round"
                | "self"
                | "send"
                | "setelement"
                | "size"
                | "spawn"
                | "spawn_link"
                | "struct"
                | "throw"
                | "tl"
                | "to_charlist"
                | "to_string"
                | "trunc"
                | "tuple_size"
                | "unlink"
                | "element"
                | "list_to_binary"
                | "binary_to_list"
                | "atom_to_list"
                | "list_to_atom"
                | "integer_to_list"
                | "list_to_integer"
                | "tuple_to_list"
                | "list_to_tuple"
        ),
        // Julia's Base is in scope in every file without an import, so its
        // functions are the most-called names in a Julia project and none of
        // them is declared anywhere the scan can see: on DataFrames.jl they
        // are 2315 calls filed as "unresolved", which reads as a resolver
        // that failed. This list is checked only when the project declares
        // nothing by that name, so a package that defines its own `eachcol`
        // still wins.
        "julia" => matches!(
            base,
            "length"
                | "throw"
                | "ArgumentError"
                | "BoundsError"
                | "ErrorException"
                | "isempty"
                | "typeof"
                | "eltype"
                | "enumerate"
                | "Vector"
                | "Matrix"
                | "Array"
                | "Set"
                | "Dict"
                | "Pair"
                | "Symbol"
                | "String"
                | "Int"
                | "Bool"
                | "Float64"
                | "Ref"
                | "push"
                | "pop"
                | "append"
                | "copy"
                | "copyto"
                | "deepcopy"
                | "similar"
                | "collect"
                | "map"
                | "filter"
                | "reduce"
                | "foreach"
                | "zip"
                | "all"
                | "any"
                | "first"
                | "last"
                | "fill"
                | "get"
                | "getfield"
                | "setfield"
                | "getproperty"
                | "propertynames"
                | "size"
                | "axes"
                | "view"
                | "isequal"
                | "isnothing"
                | "ismissing"
                | "eachindex"
                | "firstindex"
                | "lastindex"
                | "findfirst"
                | "findall"
                | "sort"
                | "reverse"
                | "join"
                | "split"
                | "string"
                | "print"
                | "println"
                | "error"
                | "zeros"
                | "ones"
                | "min"
                | "max"
                | "minimum"
                | "maximum"
                | "sum"
                | "count"
                | "haskey"
                | "keys"
                | "values"
                | "convert"
                | "promote"
                | "hash"
                | "show"
        ),
        _ => false,
    }
}

/// Does this file belong to the imported module?
///
/// What "the module" means depends on the language, and the import target says
/// which: a Go package is a directory (recorded with a trailing slash), so
/// `internal/states/statemgr/mgr.go` belongs to `statemgr` and not to the
/// `internal/states/` that imported it; a Python module is one file.
/// Decide whether an import really points inside the repository.
///
/// A Python absolute import cannot be judged while walking files — `import
/// pytest` and `import myapp.utils` look identical until the walk is over — so
/// the parser records the candidate paths and the answer is settled here,
/// against the files and directories the scan actually found.
fn resolved_import_package(
    context: &IndexContext,
    scanned_candidates: &mut BTreeMap<String, bool>,
    package: ImportedPackage,
) -> ImportedPackage {
    let ImportedPackage::Local(candidates) = &package else {
        return package;
    };
    let scanned = candidates.iter().any(|candidate| {
        // Every call with an import in scope asks this, and the answer
        // depends only on what the scan holds: django-oscar asked it
        // enough times that formatting the suffix to compare was 54% of
        // its scan.
        if let Some(answer) = scanned_candidates.get(candidate) {
            return *answer;
        }
        let answer = match candidate.strip_suffix('/') {
            Some(directory) => context.directory_nodes.contains_key(directory),
            None => {
                context.file_nodes.contains_key(candidate.as_str())
                    // A project's own package is often not at the root:
                    // Flask's `import flask` names `src/flask/__init__.py`.
                    // Calling that external would be a false claim, and a
                    // false claim is worse than an unresolved one.
                    || context
                        .file_nodes
                        .keys()
                        .any(|path| path_ends_with_segment(path, candidate))
            }
        };
        scanned_candidates.insert(candidate.clone(), answer);
        answer
    });
    if scanned {
        package
    } else {
        ImportedPackage::External
    }
}

/// Whether a path ends with a candidate that starts at a segment boundary:
/// `src/flask/__init__.py` ends with `flask/__init__.py`, and
/// `src/notflask/__init__.py` does not.
fn path_ends_with_segment(path: &str, candidate: &str) -> bool {
    let Some(prefix) = path.len().checked_sub(candidate.len()) else {
        return false;
    };
    prefix > 0 && path.as_bytes()[prefix - 1] == b'/' && path.ends_with(candidate)
}

fn declared_in_module(path: &str, target: &str) -> bool {
    match target.strip_suffix('/') {
        Some(_) => path
            .strip_prefix(target)
            .is_some_and(|rest| !rest.is_empty() && !rest.contains('/')),
        None => path == target,
    }
}

/// Record a call that an import proves leaves the repository. It still gets an
/// edge — the dependency is real — but the target is marked `external`, not
/// `ambiguous`, so the graph stops claiming an in-repo name collision that a
/// resolver could one day settle.
/// A call that leaves the project. `resolution` says what provides it: a
/// dependency is `external`, and the language's own library is `builtin` --
/// `BTreeMap::new` is not a package this project depends on.
fn add_external_call_placeholder(context: &mut IndexContext, call: PendingCall, resolution: &str) {
    let key = (call.language.clone(), call.label.clone());
    let label = call.label.clone();
    let language = call.language.clone();
    let line = call.span.start_line;
    let column = call.span.start_column;
    let file = call.span.path.clone();
    let call_id = if let Some(id) = context.unresolved_call_placeholders.get(&key) {
        *id
    } else {
        let mut metadata = BTreeMap::new();
        metadata.insert("language".to_string(), call.language);
        metadata.insert("parser".to_string(), "tree-sitter".to_string());
        metadata.insert("item_kind".to_string(), "call".to_string());
        metadata.insert("resolution".to_string(), resolution.to_string());
        let id = context.graph.add_node_with_metadata(
            NodeKind::ExternalDependency,
            call.label,
            Some(call.span),
            metadata,
        );
        context.unresolved_call_placeholders.insert(key, id);
        id
    };
    add_edge_once_with_metadata(
        context,
        call.caller,
        call_id,
        EdgeKind::Calls,
        Confidence::Heuristic,
        BTreeMap::from([
            ("call_label".to_string(), label),
            ("resolution".to_string(), resolution.to_string()),
            ("language".to_string(), language),
            ("file".to_string(), file),
            ("line".to_string(), line.to_string()),
            ("column".to_string(), column.to_string()),
        ]),
    );
}

/// Every narrowing a call edge can record in `resolution_basis`. Published
/// so a reader-facing surface can be held to explaining all of them: a
/// basis with no words leaves the explanation quieter than the fact.
pub const RESOLUTION_BASES: &[&str] = &[
    "same_file",
    "import",
    "package",
    "module_file",
    "lexical_scope",
    "module_export",
    "receiver_type",
    "owner_type",
    "overload",
    "name",
];

/// Whether this owner is a namespace the runtime provides rather than one
/// the project declares. A project can add to it -- kong patches `ngx` --
/// but a call means that definition only when it names the same global.
fn patches_runtime_global(language: &str, owner: &str) -> bool {
    match language {
        "lua" => matches!(
            owner,
            "ngx"
                | "os"
                | "io"
                | "string"
                | "table"
                | "math"
                | "coroutine"
                | "debug"
                | "package"
                | "jit"
                | "bit"
                | "utf8"
                | "_G"
        ),
        "javascript" | "typescript" | "tsx" => matches!(
            owner,
            "window"
                | "globalThis"
                | "global"
                | "document"
                | "console"
                | "Object"
                | "Array"
                | "String"
                | "Number"
                | "Math"
                | "JSON"
                | "Promise"
                | "Reflect"
                | "Symbol"
                | "Date"
        ),
        "python" => matches!(owner, "os" | "sys" | "json" | "re" | "time" | "logging"),
        // A nix module is a function of what the evaluator hands it, and
        // those names are nixpkgs': `lib.optionalString` is not the
        // `optionalString` home-manager's termite module binds in a `let`,
        // and `builtins.typeOf` is the language itself. A name the project
        // does add under one of them -- `lib.hm.booleans.yesNo` -- keeps
        // the whole prefix as its owner and is left alone.
        "nix" => matches!(
            owner,
            "builtins" | "lib" | "pkgs" | "config" | "options" | "stdenv" | "inputs"
        ),
        _ => false,
    }
}

/// Whether the file is the module the call names. OCaml, Lua and Python
/// each name a module after the file that holds it, so `Json.assoc` is
/// `assoc` in json.ml and `kong.response.exit` is written in
/// kong/pdk/response.lua. That is the language's own rule rather than a
/// guess about where a name might live.
fn module_named_file(language: &str, path: &str, module: &str) -> bool {
    let file = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();
    let module = module.to_ascii_lowercase();
    match language {
        "ocaml" => file == format!("{module}.ml") || file == format!("{module}.mli"),
        "lua" => file == format!("{module}.lua"),
        "python" => file == format!("{module}.py"),
        _ => false,
    }
}

/// Whether every candidate is the same method of the same type: a set of
/// overloads rather than a choice between unrelated definitions. Requires
/// an owner, so free functions sharing a name never qualify.
///
/// The directory is part of the test because a type name is not unique:
/// terraform declares `Diagnostics.HasErrors` in both `internal/policy`
/// and `internal/tfdiags`, two different types that happen to agree on a
/// name, and Go has no overloads at all. Where a language does overload,
/// the signatures sit together — a C# partial class or a Swift extension
/// splits a type across files, not across packages.
/// Whether every candidate is an arm of one macro: the same name defined
/// more than once in one file, which is how a header writes `#ifdef` /
/// `#else`. nlohmann defines `JSON_THROW` twice in each copy of its
/// header, and the caller means the macro, not one of the two arms.
/// Whether a name is a type parameter rather than a value the project
/// declares: `F`, `G`, `M`, `A1`. Scala and Java write them constantly,
/// and a call through one names a type the syntax does not.
fn names_a_type_parameter(name: &str) -> bool {
    let name = name.trim();
    let mut characters = name.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    first.is_ascii_uppercase() && characters.all(|character| character.is_ascii_digit())
}

fn one_macros_arms(graph: &CodeGraph, targets: &[NodeId]) -> bool {
    let mut label: Option<String> = None;
    let mut path: Option<String> = None;
    for target in targets {
        let Some(node) = graph_node(graph, *target) else {
            return false;
        };
        if node.metadata.get("definition_form").map(String::as_str) != Some("macro") {
            return false;
        }
        let Some(node_path) = node.span.as_ref().map(|span| span.path.clone()) else {
            return false;
        };
        if label.get_or_insert_with(|| node.label.clone()) != &node.label {
            return false;
        }
        if path.get_or_insert_with(|| node_path.clone()) != &node_path {
            return false;
        }
    }
    label.is_some()
}

fn one_methods_overloads(graph: &CodeGraph, targets: &[NodeId]) -> bool {
    let mut owner: Option<String> = None;
    let mut label: Option<String> = None;
    let mut directory: Option<String> = None;
    // `expect class Buffer` in commonMain and `actual class Buffer` in
    // jvmMain are one class written twice, and a source set is a directory
    // of its own -- so the directory two halves of one declaration sit in
    // is exactly what differs. okio spreads 768 calls over pairs like that.
    let across_platforms = targets.iter().all(|target| {
        graph_node(graph, *target).is_some_and(|node| node.metadata.contains_key("platform_form"))
    });
    for target in targets {
        let Some(node) = graph_node(graph, *target) else {
            return false;
        };
        let Some(node_owner) = node.metadata.get("owner_type") else {
            return false;
        };
        let Some(node_directory) = node.span.as_ref().map(|span| {
            span.path
                .rsplit_once('/')
                .map(|(head, _)| head)
                .unwrap_or("")
        }) else {
            return false;
        };
        if owner.get_or_insert_with(|| node_owner.clone()) != node_owner {
            return false;
        }
        if label.get_or_insert_with(|| node.label.clone()) != &node.label {
            return false;
        }
        if !across_platforms
            && directory.get_or_insert_with(|| node_directory.to_string()) != node_directory
        {
            return false;
        }
    }
    owner.is_some()
}

/// The class name a PHP call writes, without the namespace it may spell in
/// full: `\\App\\Models\\Song::find` and `Song::find` name one class.
fn php_class_name(class: &str) -> &str {
    class
        .trim()
        .trim_start_matches('\\')
        .rsplit('\\')
        .next()
        .unwrap_or(class)
}

/// Whether the project declares the constant a ruby call is written through.
/// A nested declaration answers a call written from inside its namespace:
/// `Namespace.new` inside `module UserSettings` means the
/// `UserSettings::Namespace` the project declares.
fn declares_ruby_constant(declared: &BTreeSet<String>, receiver: &str) -> bool {
    if declared.contains(receiver) {
        return true;
    }
    let suffix = format!("::{receiver}");
    declared.iter().any(|constant| constant.ends_with(&suffix))
}

/// The constructor a class declares, when it declares one. Every language
/// names it in its own way, and it is the method building the class runs.
fn class_constructor(graph: &CodeGraph, class: NodeId) -> Option<NodeId> {
    let class_node = graph_node(graph, class)?;
    let class_name = class_node.label.as_str();
    let class_path = class_node.span.as_ref().map(|span| span.path.as_str());
    graph
        .nodes
        .iter()
        .find(|node| {
            node.kind == NodeKind::Function
                && matches!(
                    node.label.as_str(),
                    "__construct" | "constructor" | "__init__" | "new"
                )
                && node
                    .metadata
                    .get("owner_type")
                    .is_some_and(|owner| owner == class_name || owner.ends_with(class_name))
                // The class and its constructor are written in one file in
                // every language that names it this way.
                && node.span.as_ref().map(|span| span.path.as_str()) == class_path
        })
        .map(|node| node.id)
}

/// A julia file is not a module. `module DataFrames` is written once and
/// `include`s the rest, so the functions in an included file belong to the
/// module of the file that included it -- DataFrames declares 1289 of them
/// and only 98 sat inside the `module` block that names them all. Every
/// name two of its files shared was then a choice the graph could not
/// make, though multiple dispatch means they are one function.
/// The name the imports that reach a file call it by. A lua file is a
/// module and so is a python one, but neither states its own name: only
/// `require "kong.tools.table"` and `import oscar.core.loading` do, and
/// without reading them the file could be asked about by path and by
/// nothing else. A name several imports disagree about is left off.
pub(crate) fn name_files_by_their_imports(context: &mut IndexContext) {
    let mut file_ids: BTreeMap<NodeId, ()> = BTreeMap::new();
    for node in &context.graph.nodes {
        if node.kind == NodeKind::File {
            file_ids.insert(node.id, ());
        }
    }
    // An import node sits between the file that writes it and the file it
    // names.
    let mut named: BTreeMap<NodeId, BTreeMap<String, usize>> = BTreeMap::new();
    for edge in &context.graph.edges {
        if edge
            .metadata
            .get("relation")
            .is_none_or(|relation| relation != "local_import_file")
            || !file_ids.contains_key(&edge.target)
        {
            continue;
        }
        let Some(target) = graph_node(&context.graph, edge.source)
            .and_then(|node| node.metadata.get("import_target"))
            .map(String::as_str)
        else {
            continue;
        };
        // A relative path is how the file is spelled already; a dotted
        // module name is the thing no other node carries.
        if !target.contains('.')
            || target.contains('/')
            || target.contains('\\')
            || target.starts_with('.')
        {
            continue;
        }
        // Zig writes `@import("Server.zig")`, which names the file by the
        // name the label already carries. That is not a module name.
        if graph_node(&context.graph, edge.target)
            .map(|node| node.label.rsplit('/').next().unwrap_or(&node.label))
            .is_some_and(|basename| basename == target)
        {
            continue;
        }
        *named
            .entry(edge.target)
            .or_default()
            .entry(target.to_string())
            .or_insert(0usize) += 1;
    }
    for (file, names) in named {
        // A file can be imported under more than one name -- oscar's
        // sandbox reaches `oscar.core.loading` as `core.loading` too -- and
        // the one most of its importers write is the one to answer for. A
        // tie takes the first by name, so the graph is the same every run.
        let Some(name) = names
            .iter()
            .max_by(|left, right| left.1.cmp(right.1).then(right.0.cmp(left.0)))
            .map(|(name, _)| name.clone())
        else {
            continue;
        };
        if let Some(node) = graph_node_mut(&mut context.graph, file) {
            node.metadata.insert("module_name".to_string(), name);
        }
    }
}

/// A definition written inside another is reached through the one that
/// holds it, and nothing said so: flask's `route` returns a `decorator`
/// that calls `add_url_rule`, and asking for the way from `route` to
/// `add_url_rule` found no path at all. Every decorator, factory and
/// callback-returning function was a dead end.
///
/// The metadata names the holder, so the span picks which one it is when a
/// file writes several by the same name -- the nearest definition whose
/// span covers the inner one.
pub(crate) fn link_definitions_to_the_ones_that_hold_them(context: &mut IndexContext) {
    let mut links: Vec<(NodeId, NodeId)> = Vec::new();
    // The definitions that could hold another are looked up by the name the
    // inner one states, once: reading the node list per nested definition is
    // a pass over the graph each time, and dune nests them everywhere --
    // that was two thirds of its scan.
    let mut functions_by_label: BTreeMap<&str, Vec<&Node>> = BTreeMap::new();
    for node in &context.graph.nodes {
        if node.kind == NodeKind::Function {
            functions_by_label
                .entry(node.label.as_str())
                .or_default()
                .push(node);
        }
    }
    for node in &context.graph.nodes {
        if node.kind != NodeKind::Function {
            continue;
        }
        let (Some(holder), Some(span)) =
            (node.metadata.get("enclosing_function"), node.span.as_ref())
        else {
            continue;
        };
        let holder = functions_by_label
            .get(holder.as_str())
            .map(Vec::as_slice)
            .unwrap_or_default()
            .iter()
            .copied()
            .filter(|candidate| candidate.id != node.id)
            .filter_map(|candidate| candidate.span.as_ref().map(|outer| (candidate, outer)))
            .filter(|(_, outer)| {
                outer.path == span.path
                    && outer.start_line <= span.start_line
                    && outer.end_line >= span.end_line
            })
            // The nearest one, when a file nests several by the same name.
            .min_by_key(|(_, outer)| outer.end_line.saturating_sub(outer.start_line))
            .map(|(candidate, _)| candidate.id);
        if let Some(holder) = holder {
            links.push((holder, node.id));
        }
    }
    for (holder, inner) in links {
        add_edge_once_with_metadata(
            context,
            holder,
            inner,
            EdgeKind::References,
            Confidence::Exact,
            BTreeMap::from([("relation".to_string(), "encloses".to_string())]),
        );
    }
}

pub(crate) fn assign_julia_included_modules(context: &mut IndexContext) {
    let mut module_of_file: BTreeMap<String, String> = BTreeMap::new();
    let mut ambiguous: BTreeSet<String> = BTreeSet::new();
    for node in &context.graph.nodes {
        if node.kind != NodeKind::Module
            || node.metadata.get("language").map(String::as_str) != Some("julia")
        {
            continue;
        }
        let Some(path) = node.span.as_ref().map(|span| span.path.clone()) else {
            continue;
        };
        // A file that writes two modules says nothing about which one an
        // include belongs to.
        if module_of_file
            .insert(path.clone(), node.label.clone())
            .is_some_and(|existing| existing != node.label)
        {
            ambiguous.insert(path);
        }
    }
    for path in &ambiguous {
        module_of_file.remove(path);
    }

    let scanned: BTreeSet<&str> = context
        .graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::File)
        .map(|node| node.label.as_str())
        .collect();
    let mut includes: Vec<(String, String)> = Vec::new();
    for pending in &context.pending_local_imports {
        let Some(node) = graph_node(&context.graph, pending.import_node) else {
            continue;
        };
        if node.metadata.get("language").map(String::as_str) != Some("julia") {
            continue;
        }
        let Some(includer) = node.span.as_ref().map(|span| span.path.clone()) else {
            continue;
        };
        for candidate in &pending.candidates {
            if scanned.contains(candidate.as_str()) {
                includes.push((includer.clone(), candidate.clone()));
                break;
            }
        }
    }

    // An include chain is a tree from the file that states the module, and
    // a file included from two modules is left alone.
    let mut queue: VecDeque<String> = module_of_file.keys().cloned().collect();
    let mut settled: BTreeSet<String> = module_of_file.keys().cloned().collect();
    while let Some(includer) = queue.pop_front() {
        let Some(module) = module_of_file.get(&includer).cloned() else {
            continue;
        };
        for (from, to) in &includes {
            if from != &includer || settled.contains(to) {
                continue;
            }
            settled.insert(to.clone());
            module_of_file.insert(to.clone(), module.clone());
            queue.push_back(to.clone());
        }
    }

    for node in &mut context.graph.nodes {
        if node.kind != NodeKind::Function
            || node.metadata.contains_key("owner_type")
            || node.metadata.get("language").map(String::as_str) != Some("julia")
        {
            continue;
        }
        let Some(module) = node
            .span
            .as_ref()
            .and_then(|span| module_of_file.get(&span.path))
        else {
            continue;
        };
        node.metadata
            .insert("owner_type".to_string(), module.clone());
    }
}

pub(crate) fn resolve_pending_calls(context: &mut IndexContext) {
    // Asked once per call and per candidate, so it is read off the node
    // list once rather than three times per question.
    let type_names = type_nodes_by_name(&context.graph);
    // What a module hands out under a name another module declares. Only a
    // file that states one is kept.
    let re_exports: BTreeMap<String, String> = context
        .graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::File)
        .filter_map(|node| {
            node.metadata
                .get("re_exports")
                .map(|list| (node.label.clone(), list.clone()))
        })
        .collect();
    let pending_calls = std::mem::take(&mut context.pending_calls);
    // What the scan holds does not change while calls are resolved, so each
    // candidate path is looked for once.
    let mut scanned_candidates: BTreeMap<String, bool> = BTreeMap::new();
    // Every class, module or container the project declares something in.
    // A call written through one of these names is answered by what belongs
    // to it and by nothing else: dune's `List.map` is `Stdlib`'s, however
    // many other `map` the repository has.
    let declared_owners: BTreeSet<String> = context
        .graph
        .nodes
        .iter()
        .filter_map(|node| node.metadata.get("owner_type").cloned())
        .collect();
    // What each file includes, for the languages where a header is how a
    // file reaches a declaration. nlohmann keeps three copies of its
    // library -- the sources, the amalgamated `single_include`, and an ABI
    // fixture -- so every macro and every method is declared several times
    // and only the include says which copy a caller means.
    let mut included = included_files(&context.graph);
    // A header that the walk had not reached yet when the including file
    // was read is still on the pending list rather than in the graph, and
    // calls resolve before that list does. nlohmann's headers include each
    // other in both directions, so most of them are of this kind.
    {
        let scanned: BTreeSet<&str> = context
            .graph
            .nodes
            .iter()
            .filter(|node| node.kind == NodeKind::File)
            .map(|node| node.label.as_str())
            .collect();
        let mut pending_includes: Vec<(String, String)> = Vec::new();
        for pending in &context.pending_local_imports {
            let Some(node) = graph_node(&context.graph, pending.import_node) else {
                continue;
            };
            if !matches!(
                node.metadata.get("language").map(String::as_str),
                Some("c") | Some("cpp")
            ) {
                continue;
            }
            let Some(source) = node.span.as_ref().map(|span| span.path.clone()) else {
                continue;
            };
            for candidate in &pending.candidates {
                if let Some(path) = scanned.get(candidate.as_str()) {
                    pending_includes.push((source, (*path).to_string()));
                    break;
                }
            }
        }
        for (source, header) in pending_includes {
            included.entry(source).or_default().insert(header);
        }
    }
    // The files that export something. Only there does "not exported"
    // mean private: a CommonJS file hands its functions out through
    // `module.exports`, which is not an export statement, so nothing in it
    // can be called private on that evidence.
    let exporting_files: BTreeSet<String> = context
        .graph
        .nodes
        .iter()
        .filter(|node| {
            node.metadata.get("visibility").map(String::as_str) == Some("public")
                && matches!(
                    node.metadata.get("language").map(String::as_str),
                    Some("typescript") | Some("javascript") | Some("tsx")
                )
        })
        .filter_map(|node| node.span.as_ref().map(|span| span.path.clone()))
        .collect();
    // Every class this project declares, by the short name a call writes.
    // A PHP static call names the class it goes through, and a class the
    // project never declares is a package's or the language's.
    // Every module this project declares, by the file that is it. An erlang
    // call names its module outright -- `gun:open(..)` -- so a module the
    // project does not have is a dependency's or OTP's, and OTP is already
    // answered before this. cowboy writes 1764 such calls to `gun`,
    // `ranch`, `cow_hpack` and `quicer`, every one of them reported as a
    // resolver failure.
    let erlang_modules: BTreeSet<String> = context
        .graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::File)
        .filter_map(|node| {
            let name = node.label.rsplit('/').next()?;
            let (stem, extension) = name.rsplit_once('.')?;
            matches!(extension, "erl" | "hrl").then(|| stem.to_string())
        })
        .collect();

    // The files an ocaml module answers from: the file named after it, the
    // files that declare it, and whatever those include. `Fiber` is
    // `src/fiber/src/fiber.ml`, which is `include Core`, so `Fiber.return`
    // is `core.ml`'s. dune answers 20163 of its 21194 module-qualified
    // calls inside that closure; the 796 that land outside are `String.sub`
    // in bigstringaf and `Unix.lstat` in stdune's `path.ml` -- a standard
    // library module answered by an unrelated file that happens to declare
    // the name.
    let mut ocaml_module_files: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut ocaml_module_includes: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut ocaml_nested_modules: BTreeSet<(String, String)> = BTreeSet::new();
    for node in &context.graph.nodes {
        if node.metadata.get("language").map(String::as_str) != Some("ocaml") {
            continue;
        }
        if node.kind == NodeKind::File
            && let Some(stem) = node
                .label
                .rsplit('/')
                .next()
                .and_then(|name| name.rsplit_once('.'))
                .filter(|(_, extension)| matches!(*extension, "ml" | "mli"))
                .map(|(stem, _)| stem)
        {
            let mut module = stem.to_string();
            if let Some(first) = module.get_mut(..1) {
                first.make_ascii_uppercase();
            }
            ocaml_module_files
                .entry(module)
                .or_default()
                .insert(node.label.clone());
        }
        if node.kind == NodeKind::Module
            && let Some(span) = node.span.as_ref()
        {
            // A module declared inside a file is that file's own: from
            // anywhere else it is `Csexp_rpc.Unix`, not `Unix`. Treating
            // the declaration as a project-wide answer let `csexp_rpc.ml`
            // answer `Unix.close` for stdune's `fd.ml` and closed a
            // dependency cycle that does not exist.
            ocaml_nested_modules.insert((span.path.clone(), node.label.clone()));
            if let Some(extends) = node.metadata.get("extends") {
                ocaml_module_includes
                    .entry(node.label.clone())
                    .or_default()
                    .extend(
                        extends
                            .split(',')
                            .filter_map(|part| part.trim().rsplit('.').next())
                            .filter(|part| !part.is_empty())
                            .map(ToString::to_string),
                    );
            }
        }
    }
    // What a module includes answers for it too, three levels deep: beyond
    // that the chains in dune are aliases of aliases and add nothing.
    for _ in 0..3 {
        let grown: Vec<(String, BTreeSet<String>)> = ocaml_module_includes
            .iter()
            .map(|(module, included)| {
                let mut files = ocaml_module_files.get(module).cloned().unwrap_or_default();
                for other in included {
                    if let Some(more) = ocaml_module_files.get(other) {
                        files.extend(more.iter().cloned());
                    }
                }
                (module.clone(), files)
            })
            .collect();
        for (module, files) in grown {
            ocaml_module_files.entry(module).or_default().extend(files);
        }
    }

    // Every module an ocaml file declares, by the file that declares it.
    // `Process.run` is that module's function, and letting a same-file
    // `run` answer it said 2366 of dune's calls belong to the definition
    // that contains them -- which is what `doctor` reports as a definition
    // calling itself, 1027 times on dune and 822 on cats. A file may
    // declare the module inside itself, and then the same-file answer is
    // the right one.
    let ocaml_modules_by_file: BTreeSet<(String, String)> = context
        .graph
        .nodes
        .iter()
        .filter(|node| {
            node.kind == NodeKind::Module
                && node.metadata.get("language").map(String::as_str) == Some("ocaml")
        })
        .filter_map(|node| Some((node.span.as_ref()?.path.clone(), node.label.clone())))
        .collect();

    let php_classes: BTreeSet<String> = context
        .graph
        .nodes
        .iter()
        .filter(|node| node.metadata.get("language").map(String::as_str) == Some("php"))
        .filter_map(|node| {
            if node.kind == NodeKind::Type {
                Some(node.label.clone())
            } else {
                node.metadata.get("owner_type").cloned()
            }
        })
        .map(|class| php_class_name(&class).to_string())
        .collect();
    // Every constant this project declares. A ruby call names its receiver
    // and the label drops it, so the constant is the evidence that says
    // whose method is meant -- and a constant the project never declares
    // belongs to a gem.
    let ruby_constants: BTreeSet<String> = context
        .graph
        .nodes
        .iter()
        .filter(|node| {
            node.metadata.get("language").map(String::as_str) == Some("ruby")
                && matches!(node.kind, NodeKind::Type | NodeKind::Module)
        })
        .map(|node| node.label.clone())
        .chain(
            context
                .graph
                .nodes
                .iter()
                .filter(|node| node.metadata.get("language").map(String::as_str) == Some("ruby"))
                .filter_map(|node| node.metadata.get("owner_type").cloned()),
        )
        .collect();

    for call in pending_calls {
        // `Uuid::generate()` names the class the call goes through, and koel
        // declares no `Uuid`: the class comes from a package, so its
        // `generate` is not the `TestableIdentifier::generate` the project
        // declares -- which 61 call sites reached. `self`, `static` and
        // `parent` name the class the call is inside and never reach here.
        if call.language == "php"
            && let Some((owner, _)) = split_qualified_call(&call.label)
            && !php_classes.contains(php_class_name(owner))
        {
            add_external_call_placeholder(context, call, "external");
            continue;
        }
        // `Rails.application.configure` and mastodon's own
        // `UserSettings::Namespace#configure` share a name and nothing else,
        // and a ruby call's label keeps only the name. The constant the call
        // is written through says which is meant: one the project never
        // declares belongs to a gem, and a gem's method is not among this
        // project's own. `Addressable::URI.parse(href).normalize` was
        // answered by `HashtagNormalizer#normalize`, `FastImage.size` by a
        // connection pool's, and `Chewy::Stash::Specification.reset!` by a
        // delivery tracker's.
        if call.language == "ruby"
            && !builtin_call_target(&call.language, &call.label)
            && call
                .receiver
                .as_deref()
                .is_some_and(|receiver| !declares_ruby_constant(&ruby_constants, receiver))
        {
            add_external_call_placeholder(context, call, "external");
            continue;
        }
        // A qualified name the language itself provides is answered by the
        // language. `Object.create(...)` in axios shares only its tail with
        // the repository's `instance.create`, and matching on that tail
        // invented a dependency cycle between two files that never call
        // each other.
        // The same holds for a name the file's environment hands it:
        // `defineProps` inside a `<script setup>` block is the macro that
        // compiler expands, even in the repository that also exports a
        // function by that name -- vue does.
        let all_targets = if ((call.label.contains('.') || call.label.contains(':'))
            && builtin_call_target(&call.language, &call.label))
            || environment_provides_call(&call.language, &call.span.path, &call.label)
        {
            Vec::new()
        } else {
            resolve_function_targets(&context.function_symbols, &call.label)
        };
        let caller_path = graph_node(&context.graph, call.caller)
            .and_then(|node| node.span.as_ref())
            .map(|span| span.path.as_str());
        // Each of these narrowings used to build a vector of its own, once
        // per call: terraform makes 100000 of them, so the narrowing
        // happens in place.
        let mut language_targets = all_targets;
        // Nothing in the project is called that. Whatever the call means,
        // it is not one of these definitions, and saying `unresolved` reads
        // as a resolver that failed rather than a name that was never here:
        // mastodon writes 20059 such calls -- `where`, `present?`,
        // `before_action` -- and terraform 8681.
        let no_definition_has_that_name = language_targets.is_empty();
        language_targets.retain(|target| {
            graph_node(&context.graph, *target)
                .and_then(|node| node.metadata.get("language"))
                .is_some_and(|language| languages_share_symbols(language, &call.language))
        });
        // A definition that patches a runtime global answers only calls
        // written through that global. kong replaces `ngx.exit` in
        // globalpatches.lua, and `kong.response.exit(...)` -- a different
        // function, built inside the PDK factory -- was answered by it on
        // the shared tail alone.
        let call_owner = split_qualified_call(&call.label).map(|(owner, _)| owner);
        language_targets.retain(|target| {
            let Some(owner) = graph_node(&context.graph, *target)
                .and_then(|node| split_qualified_call(&node.label))
                .map(|(owner, _)| owner)
            else {
                return true;
            };
            !patches_runtime_global(&call.language, owner) || call_owner == Some(owner)
        });
        // And the other way round: a call written through the runtime's own
        // namespace is answered by the runtime. `table.concat` is Lua's,
        // whatever a project names its own helpers -- kong declares `concat`
        // in kong/tools/table.lua and it answered 221 calls to it, `decode`
        // in an LDAP plugin and it answered 291 calls to `cjson.decode`.
        if let Some(owner) = call_owner
            && patches_runtime_global(&call.language, owner)
        {
            language_targets.retain(|target| {
                graph_node(&context.graph, *target)
                    .and_then(|node| split_qualified_call(&node.label))
                    .is_some_and(|(target_owner, _)| target_owner == owner)
            });
        }
        // `value.parse::<usize>()` is `str::parse`, and this repository has
        // a `pub(crate) fn parse` of its own; ripgrep's 374 `.unwrap()`
        // calls found its private `fn unwrap`. A Rust call written through a
        // receiver is answered by a method, and by no method at all when the
        // name is one every type already has.
        // `::open(...)` names the global namespace outright, which is
        // where C++ says a class member is not: spdlog calls the POSIX
        // `::open` and the graph answered with its own `file_helper::open`,
        // closing a cycle with `fopen_s` that the program does not have.
        if matches!(call.language.as_str(), "c" | "cpp") && call.label.starts_with("::") {
            language_targets.retain(|target| {
                graph_node(&context.graph, *target)
                    .is_some_and(|node| !node.metadata.contains_key("owner_type"))
            });
        }
        // `accounts.each`, `stack.empty?`, `map.include?`: ruby writes the
        // receiver and the label keeps only the method, so a project method
        // named after one every collection has answered calls on values it
        // never saw -- mastodon's `Trends::History#each` had 268 callers,
        // its connection pool's `empty?` 134 and its IP map's `include?`
        // 126. A bare call means `self` and is left alone; only a call
        // through a value the syntax does not name is refused.
        if call.language == "ruby"
            && call.receiver_is_a_value
            && ruby_method_of_every_value(&call.label)
        {
            language_targets.clear();
        }
        // The same holds when the receiver never reached the label: a chain
        // reduces `args.into_iter().map` to `map`, and the call is still on
        // a value of the language's own.
        if receiver_call_is_universal(&call.language, &call.label)
            || (call.receiver_is_a_value
                && call.language != "ruby"
                && method_of_every_value(&call.language, &call.label))
        {
            language_targets.clear();
        } else if call.language == "rust" && call.label.contains('.') {
            language_targets.retain(|target| {
                graph_node(&context.graph, *target)
                    .is_some_and(|node| node.metadata.contains_key("owner_type"))
            });
        }
        // The program does not call its own tests. flask has exactly one
        // function named `close` — a helper in tests/test_helpers.py — so
        // `builder.close()` in src/flask/app.py resolved to it with full
        // confidence, and 1143 such links existed across the corpora. A
        // caller outside tests, examples and fixtures cannot mean one.
        // The call's own file, not the caller node's: a call whose caller
        // carries no span -- a lua module's top level, a script's body --
        // read as program code wherever it was written, and then this rule
        // refused it every helper its own suite declares. kong writes 287
        // `helpers.get_db_utils` in `spec/` and not one reached
        // `spec/internal/db.lua`.
        let caller_is_test = is_test_like_source_path(&call.span.path);
        // A program never calls its own suite, but it does call what it
        // vendors and what a tool generated for it: dune uses the `lwd` it
        // vendors from its own source, and reading the two the same way
        // cost 1274 of dune's resolved calls and 127 of redis's.
        if !caller_is_test {
            language_targets.retain(|target| {
                graph_node(&context.graph, *target)
                    .and_then(|node| node.span.as_ref())
                    .is_none_or(|span| {
                        !is_test_like_source_path(&span.path)
                            || is_shipped_but_not_written(&span.path)
                    })
            });
        }
        // What narrowed the call, when the module's own files did.
        let mut narrowed_to_the_module = false;
        // A module answers for its own files and for what they include,
        // and for nothing else. `Unix.lstat` is not stdune's `path.ml`
        // because that file declares an `lstat`; 796 of dune's resolutions
        // were of that kind, one of them closing a dependency cycle that
        // does not exist.
        if call.language == "ocaml"
            && let Some((module, _)) = call.label.split_once('.')
            && !module.contains('.')
            && module
                .chars()
                .next()
                .is_some_and(|first| first.is_uppercase())
        {
            // A module with no file and no node here is the standard
            // library's or a dependency's: dune writes 890 such calls and
            // 237 of them were answered by an unrelated project file --
            // `Printf.printf` by stdune's `console.ml` 161 times, and
            // `Unix.close` by the scheduler's own `close`, which closed a
            // dependency cycle between stdune and the scheduler that does
            // not exist.
            let answers = ocaml_module_files.get(module);
            let declared_here = caller_path.is_some_and(|path| {
                ocaml_nested_modules.contains(&(path.to_string(), module.to_string()))
            });
            let before = language_targets.len();
            match answers {
                Some(answers) if !answers.is_empty() => {
                    language_targets.retain(|target| {
                        graph_node(&context.graph, *target)
                            .and_then(|node| node.span.as_ref())
                            .is_some_and(|span| {
                                answers.contains(&span.path)
                                    || (declared_here && Some(span.path.as_str()) == caller_path)
                            })
                    });
                }
                _ if declared_here => {
                    language_targets.retain(|target| {
                        graph_node(&context.graph, *target)
                            .and_then(|node| node.span.as_ref())
                            .is_some_and(|span| Some(span.path.as_str()) == caller_path)
                    });
                }
                _ => language_targets.clear(),
            }
            narrowed_to_the_module = language_targets.len() < before;
        }
        // `super.x` -- `base.x` in C# -- written inside `x` means the
        // parent's implementation and never this one. It has to be settled before the same-file
        // preference below, which would otherwise answer with the caller's
        // own `x` sitting in the same file -- openzeppelin wrote 174 calls
        // from a definition to itself that way.
        // `base` is the word C# uses; everywhere else it is a name a
        // program may bind to anything, and oscar and zod both do.
        if let Some((qualifier, method)) = split_qualified_call(&call.label)
            && (qualifier == "super" || (qualifier == "base" && call.language == "csharp"))
        {
            let inherited: Vec<String> = graph_node(&context.graph, call.caller)
                .and_then(|node| node.metadata.get("owner_type").cloned())
                .map(|owner| ancestor_type_names(&context.graph, &type_names, &owner))
                .unwrap_or_default();
            // Nothing inherited declares it: the parent is outside the
            // project -- an interface, a library base -- and the honest
            // answer is that the call leaves.
            language_targets.retain(|target| {
                graph_node(&context.graph, *target).is_some_and(|node| {
                    node.label == method
                        && node
                            .metadata
                            .get("owner_type")
                            .is_some_and(|owner| inherited.contains(owner))
                })
            });
        }
        let names_another_ocaml_module = call.language == "ocaml"
            && call.label.split_once('.').is_some_and(|(module, _)| {
                module
                    .chars()
                    .next()
                    .is_some_and(|first| first.is_uppercase())
                    && caller_path.is_some_and(|path| {
                        !module_named_file("ocaml", path, module)
                            && !ocaml_modules_by_file
                                .contains(&(path.to_string(), module.to_string()))
                    })
            });
        // `a.b.method()` reaches the method through a field, so the name
        // belongs to the type of `b` — which a scan of declarations never
        // records. The file that holds `a` holds no answer: terraform's
        // `s.mu.Lock` was answering with the caller's own `State.Lock`, and
        // `s.state.Module` with `SyncState.Module` when the field is a
        // `*State`. A chain of lowercase links is a chain of values; an
        // uppercase link is a name a module or a package can answer for.
        let written_through_a_chain_of_values = {
            let mut links = call.label.split('.').collect::<Vec<_>>();
            links.pop();
            links.len() >= 2
                // A Go package is lowercase and a Go type is not written in
                // front of a call, so two dots there are always a field
                // reached through a value, whatever the case of its name:
                // gqlgen's generated stubs call `r.QueryResolver.EchoIntToInt`,
                // a func held in a field, and the file answered with the very
                // method the call sits in.
                && (call.language == "go"
                    || links.iter().all(|link| {
                        link.chars()
                            .next()
                            .is_some_and(|first| first.is_lowercase() || first == '_')
                    }))
        };
        let local_targets = caller_path
            .map(|path| {
                language_targets
                    .iter()
                    .copied()
                    .filter(|target| {
                        graph_node(&context.graph, *target)
                            .and_then(|node| node.span.as_ref())
                            .is_some_and(|span| span.path == path)
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        // A module shares nothing ambiently: a bare name a JS, TS or Python
        // module calls is either declared in that file or imported into it.
        // `const h = originalH` in vue's Teleport spec and `const {
        // trigger } = useContextMenu()` in koel's context menus bind a name
        // the file never imports, and matching by name alone sent those
        // calls into other modules -- 204 of koel's cross-file calls and
        // 201 of vue's. A file that imports nothing may be a classic script
        // loaded by a page, where a bare name really can come from
        // anywhere, so the rule asks only files that state an import -- and
        // never one whose imports cannot be listed, `from x import *` or a
        // notebook's `%run other.ipynb`.
        let mut name_is_not_imported = false;
        if matches!(
            call.language.as_str(),
            "javascript" | "typescript" | "tsx" | "python"
        ) && !call.label.contains('.')
            && local_targets.is_empty()
            && !context
                .file_wildcard_imports
                .contains(call.span.path.as_str())
            && let Some(imported) = context.file_imported_names.get(call.span.path.as_str())
            && !imported.contains_key(&call.label)
        {
            language_targets.clear();
            name_is_not_imported = true;
        }
        // A python method is reached through an object -- `grid.print()` --
        // never by its bare name, whatever the file imports. pytudes writes
        // a `print` method on its grid class, and 58 notebook calls to the
        // builtin `print` were answered by it.
        if call.language == "python" && !call.label.contains('.') {
            language_targets.retain(|target| {
                graph_node(&context.graph, *target)
                    .is_some_and(|node| !node.metadata.contains_key("owner_type"))
            });
        }
        // A method is reached through an object even where the language
        // lets `this` go unwritten -- and then the object is the caller's
        // own. `f(...)` in cats' Chain.scala is a function the body was
        // handed, not `FlatMapped#f` over in FreeT.scala, yet 833 calls
        // across the repository read as that one method. A method the
        // caller's file declares, or one its own type inherits, is still
        // reachable without a receiver, and so is a name the file imports
        // outright.
        if bare_call_stays_with_its_object(&call.language)
            && !call.receiver_is_a_value
            && !call.label.contains('.')
            && !call.label.contains("::")
            && !context
                .file_imported_names
                .get(call.span.path.as_str())
                .is_some_and(|names| names.contains_key(&call.label))
        {
            let reachable_owners: Vec<String> = graph_node(&context.graph, call.caller)
                .and_then(|node| node.metadata.get("owner_type").cloned())
                .map(|owner| {
                    let mut names = ancestor_type_names(&context.graph, &type_names, &owner);
                    names.push(owner);
                    names
                })
                .unwrap_or_default();
            language_targets.retain(|target| {
                let Some(node) = graph_node(&context.graph, *target) else {
                    return true;
                };
                let Some(owner) = node.metadata.get("owner_type") else {
                    return true;
                };
                // `new Context(...)` names a class, not a method of the
                // caller's own: a constructor takes its class's name, and
                // Polly builds 208 of its `Context` that way.
                owner == &node.label
                    || node.span.as_ref().map(|span| span.path.as_str()) == caller_path
                    || reachable_owners.contains(owner)
            });
        }
        // `F.map(fa)(f)`: the receiver is a value whose type is a type
        // parameter, so no definition in the project can be named by it.
        // cats writes 178 of those, and each was reported as a choice
        // between every `map` the repository declares.
        // A name the scope states a type for is a value, whatever it is
        // called: scala names an implicit after the type it carries, and
        // `class MapAdditiveMonoid[K, V](implicit V: AdditiveSemigroup[V])`
        // makes `V.plus` that instance's rather than a type parameter's.
        // The type has to be one the project declares -- `F: Functor[F]`
        // states a type from outside it, and no definition here is that
        // one's either.
        let receiver_is_a_declared_type = call
            .receiver_type
            .as_deref()
            .is_some_and(|stated| type_node_named(&context.graph, &type_names, stated).is_some());
        if matches!(call.language.as_str(), "scala" | "kotlin" | "java")
            && !receiver_is_a_declared_type
            && let Some((owner, _)) = split_qualified_call(&call.label)
            && names_a_type_parameter(owner)
        {
            language_targets.clear();
        }
        // A nix file's `let` bindings are its own, and the language has no
        // global namespace to reach another file's through: a bare name is
        // a primop, a `with` scope's, or this file's. home-manager binds
        // `map` in modules/lib/dag.nix and it answered 132 calls to the
        // primop, and termite.nix's `optionalString` answered 27 more.
        if call.language == "nix" && !call.label.contains('.') {
            language_targets.retain(|target| {
                graph_node(&context.graph, *target)
                    .and_then(|node| node.span.as_ref())
                    .map(|span| span.path.as_str())
                    == caller_path
            });
        }
        // OCaml has no global namespace either: a bare name is the standard
        // library's, this file's, or one an `open` brought into scope.
        // Nobody in dune opens `Predicate_lang`, yet the `not` it declares
        // answered 436 calls to the language's own.
        if call.language == "ocaml" && !call.label.contains('.') {
            let opened = context.file_open_modules.get(call.span.path.as_str());
            language_targets.retain(|target| {
                let Some(path) = graph_node(&context.graph, *target)
                    .and_then(|node| node.span.as_ref())
                    .map(|span| span.path.as_str())
                else {
                    return true;
                };
                Some(path) == caller_path
                    || opened.is_some_and(|modules| {
                        modules
                            .iter()
                            .any(|module| module_named_file("ocaml", path, module))
                    })
            });
        }
        // A qualified call names where it comes from. When the calling file
        // imports that qualifier, the import list answers the question that
        // matching by name only guesses at: an in-repo package narrows the
        // candidates to that package, and an external one rules every local
        // declaration out — `strings.Contains` is not the repository's own
        // `Contains`.
        let imported_package = match split_qualified_call(&call.label) {
            Some((owner, _)) => context
                .file_import_qualifiers
                .get(call.span.path.as_str())
                .and_then(|qualifiers| {
                    // The owner is the last segment before the method --
                    // `a::b::Type::method` is `Type`'s. An import qualifier is
                    // the first: `protoimpl.X.MessageStateOf` comes from
                    // `protoimpl`, and looking `X` up in the import list found
                    // nothing, so 3234 calls in terraform's generated protobuf
                    // code were reported unresolved rather than as calls into
                    // the package the file imports.
                    qualifiers.get(owner).or_else(|| {
                        let head = call
                            .label
                            .split_once("::")
                            .or_else(|| call.label.split_once('.'))
                            .map(|(head, _)| head)?;
                        (head != owner).then(|| qualifiers.get(head))?
                    })
                })
                .cloned(),
            // Java writes the receiver in a field of its own, so the
            // qualifier a call is written through never reaches the label:
            // `Arrays.asList(..)` keeps only `asList`, and gson declares an
            // `asList` that answered 77 of them.
            None if call.language == "java" && call.receiver.is_some() => call
                .receiver
                .as_deref()
                .and_then(|receiver| {
                    context
                        .file_import_qualifiers
                        .get(call.span.path.as_str())
                        .and_then(|qualifiers| qualifiers.get(receiver))
                })
                .cloned(),
            // `from collections import OrderedDict` binds a bare name, so
            // the call site has no qualifier to look up — the name itself
            // has to say where it came from. A definition the file makes
            // itself wins over the import, which is Python's own rule.
            // A php `use` binds a class name, but PSR-4 maps a namespace
            // onto a directory and cannot always tell the project's own
            // from a dependency's: guzzle writes `use GuzzleHttp\Client;`
            // for a class in its own `src/`. A name the project declares
            // is the project's whatever the import list says, and without
            // this 425 `new Client(..)` calls stopped reaching
            // `Client::__construct`.
            None if call.language == "php" && php_classes.contains(call.label.as_str()) => None,
            None if local_targets.is_empty() => context
                .file_imported_names
                .get(call.span.path.as_str())
                .and_then(|names| names.get(&call.label))
                .cloned(),
            None => None,
        }
        .map(|package| resolved_import_package(context, &mut scanned_candidates, package));
        if imported_package == Some(ImportedPackage::External) {
            // `use std::collections::BTreeMap;` and `use walkdir::WalkDir;`
            // both rule out this project's own declarations, but only the
            // second names a dependency: the standard library is what the
            // language provides, and `builtin` is what the graph calls that.
            let provided_by_the_language = context
                .file_import_package_ids
                .get(call.span.path.as_str())
                .zip(split_qualified_call(&call.label))
                .is_some_and(|(packages, (owner, _))| {
                    let head = call
                        .label
                        .split_once("::")
                        .or_else(|| call.label.split_once('.'))
                        .map(|(head, _)| head);
                    [Some(owner), head].into_iter().flatten().any(|name| {
                        packages.get(name).is_some_and(|package| {
                            matches!(package.as_str(), "cargo:std" | "cargo:core" | "cargo:alloc")
                        })
                    })
                });
            let resolution = if provided_by_the_language {
                "builtin"
            } else {
                "external"
            };
            add_external_call_placeholder(context, call, resolution);
            continue;
        }
        // `mgr := b.StateMgr()` states the receiver's type in the callee's
        // signature rather than in its own line, so the parser records which
        // call bound the name and the join happens here, where every
        // definition is known.
        // A name bound to what a foreign package hands back is that
        // package's, and so is every call written on it: terraform binds
        // `f := os.Open(..)` and then calls `f.Close()`, which is not the
        // repository's own `Close`.
        if !written_through_a_chain_of_values
            && let Some(package) = call
                .receiver_type
                .as_deref()
                .and_then(|bound_by| bound_by.strip_suffix("()"))
                .and_then(|callee| callee.rsplit_once('.'))
                .map(|(package, _)| package)
            && context
                .file_import_qualifiers
                .get(call.span.path.as_str())
                .and_then(|qualifiers| qualifiers.get(package))
                == Some(&ImportedPackage::External)
        {
            add_external_call_placeholder(context, call, "external");
            continue;
        }

        let receiver_type = match call.receiver_type.as_deref() {
            // The type stated for `s` says nothing about `s.mu.Lock`: the
            // method belongs to the field, and the chain is what says so.
            Some(_) if written_through_a_chain_of_values => None,
            Some(bound_by) if bound_by.ends_with("()") => {
                // The name in front may be a package rather than a value, and
                // then it is the file's imports that say which directory
                // declares the function whose type the binding takes.
                let declared_in = bound_by
                    .strip_suffix("()")
                    .and_then(|callee| callee.rsplit_once('.'))
                    .and_then(|(package, _)| {
                        match context
                            .file_import_qualifiers
                            .get(call.span.path.as_str())
                            .and_then(|qualifiers| qualifiers.get(package))
                            .cloned()
                            .map(|package| {
                                resolved_import_package(context, &mut scanned_candidates, package)
                            }) {
                            Some(ImportedPackage::Local(candidates)) => Some(candidates),
                            _ => None,
                        }
                    });
                what_a_call_hands_back(context, caller_path, bound_by, declared_in.as_deref())
            }
            stated => stated.map(str::to_string),
        };
        let receiver_type = receiver_type.as_deref();

        // The same holds for the owner a call writes into its own label:
        // cats writes `StaticMethods.pow(a, k)` inside a `pow` of its own,
        // and the file answered rather than the object the call names.
        let the_label_names_another_owner = split_qualified_call(&call.label)
            .map(|(owner, _)| owner)
            .is_some_and(|owner| {
                let owned_by = |target: &NodeId| {
                    graph_node(&context.graph, *target)
                        .and_then(|node| node.metadata.get("owner_type"))
                        .is_some_and(|declared| declared == owner)
                };
                !local_targets.iter().any(owned_by) && language_targets.iter().any(owned_by)
            });

        // A receiver whose type is stated names the owner of the method, and
        // that is a stronger fact than the file the call happens to sit in.
        // Without this, `parser := configs.NewParser(fs)` was answered by
        // whatever `LoadConfigDir` the calling file declared.
        let the_receiver_names_another_owner = receiver_type
            .map(|stated| stated.rsplit(['.', '*']).next().unwrap_or(stated))
            .is_some_and(|owner| {
                let owned_by = |target: &NodeId| {
                    graph_node(&context.graph, *target)
                        .and_then(|node| node.metadata.get("owner_type"))
                        .is_some_and(|declared| declared == owner)
                };
                !local_targets.iter().any(owned_by) && language_targets.iter().any(owned_by)
            });

        // What narrowed the candidates down. A call kept by the syntax —
        // the file it sits in, the import that named the module, the
        // receiver's declared type — is a different kind of fact from one
        // that matched a name and nothing else, and every call edge used to
        // claim the same `heuristic` confidence regardless.
        let mut basis = "name";
        let mut targets = match &imported_package {
            Some(ImportedPackage::Local(candidates)) => {
                let in_module = language_targets
                    .iter()
                    .copied()
                    .filter(|target| {
                        graph_node(&context.graph, *target)
                            .and_then(|node| node.span.as_ref())
                            .is_some_and(|span| {
                                candidates
                                    .iter()
                                    .any(|candidate| declared_in_module(&span.path, candidate))
                            })
                    })
                    .collect::<Vec<_>>();
                // The module may hand the name out from another rather than
                // declare it: `spec/helpers.lua` binds `local cmd =
                // reload_module("spec.internal.cmd")` and returns
                // `start_kong = cmd.start_kong`, and 689 of kong's calls
                // stood between the spec files that declare that name
                // locally.
                let handed_out: Vec<String> = if in_module.is_empty() {
                    let method = call.label.rsplit(['.', ':']).next().unwrap_or("");
                    candidates
                        .iter()
                        .filter_map(|candidate| re_exports.get(candidate))
                        .flat_map(|list| list.split(';'))
                        .filter_map(|entry| entry.split_once('>'))
                        .filter(|(name, _)| *name == method)
                        .map(|(_, module)| format!("{}.lua", module.replace('.', "/")))
                        .collect()
                } else {
                    Vec::new()
                };
                let from_another_module = if handed_out.is_empty() {
                    Vec::new()
                } else {
                    language_targets
                        .iter()
                        .copied()
                        .filter(|target| {
                            graph_node(&context.graph, *target)
                                .and_then(|node| node.span.as_ref())
                                .is_some_and(|span| {
                                    handed_out.iter().any(|path| span.path.ends_with(path))
                                })
                        })
                        .collect::<Vec<_>>()
                };
                // Narrow only when the import actually names something the
                // scan found: an import whose module was never scanned must
                // not erase the candidates matched by name.
                if !from_another_module.is_empty() {
                    basis = "module_re_export";
                    from_another_module
                } else if in_module.is_empty() {
                    language_targets
                } else {
                    basis = "import";
                    in_module
                }
            }
            // A call written through a module this file neither is nor
            // declares is that module's, whatever this file happens to
            // name the same way.
            _ if !local_targets.is_empty()
                && !names_another_ocaml_module
                && !written_through_a_chain_of_values
                && !the_receiver_names_another_owner
                && !the_label_names_another_owner =>
            {
                basis = "same_file";
                local_targets
            }
            _ => language_targets,
        };

        // A Go call written without a qualifier can only mean something in the
        // caller's own package, and a package is a directory. Matching by name
        // across the repository left 5491 such calls ambiguous on terraform,
        // 3373 of which have exactly one candidate next door.
        if call.language == "go"
            && !call.label.contains('.')
            && targets.len() > 1
            && let Some(directory) = caller_path.and_then(|path| path.rsplit_once('/'))
        {
            let same_package = targets
                .iter()
                .copied()
                .filter(|target| {
                    graph_node(&context.graph, *target)
                        .and_then(|node| node.span.as_ref())
                        .is_some_and(|span| {
                            span.path
                                .rsplit_once('/')
                                .is_some_and(|(candidate, _)| candidate == directory.0)
                        })
                })
                .collect::<Vec<_>>();
            if !same_package.is_empty() {
                targets = same_package;
                basis = "package";
            }
        }
        // A definition nested in another one is only visible inside it, so a
        // local helper cannot be the target of a call from anywhere else. On
        // shellcheck 869 of 989 ambiguous calls had a `where` binding among
        // their candidates — one label, `f`, had 167 of them.
        {
            let caller_scope = graph_node(&context.graph, call.caller).map(|node| {
                (
                    node.label.clone(),
                    node.metadata.get("enclosing_function").cloned(),
                )
            });
            let visible = targets
                .iter()
                .copied()
                .filter(|target| {
                    let Some(node) = graph_node(&context.graph, *target) else {
                        return true;
                    };
                    let Some(enclosing) = node.metadata.get("enclosing_function") else {
                        // Top level: visible to the whole module.
                        return true;
                    };
                    // A definition whose enclosing declaration is its own type
                    // is not hidden by it. Solidity writes a contract's methods
                    // inside the contract, and `contract Child is Base` calls
                    // `Base`'s by their bare names from another contract
                    // entirely: openzeppelin declares 3477 methods that way and
                    // 2150 of its calls could reach none of them. C++ writes a
                    // method in its class body and it is still `obj.method()`.
                    if node.metadata.get("owner_type") == Some(enclosing) {
                        return true;
                    }
                    caller_scope
                        .as_ref()
                        .is_some_and(|(label, caller_enclosing)| {
                            label == enclosing
                                || caller_enclosing.as_deref() == Some(enclosing.as_str())
                        })
                })
                .collect::<Vec<_>>();
            // Dropping every candidate leaves the call unresolved, which
            // is what a call to something out of scope is: `db.close()` in
            // an example was answered by a `close` defined inside a test,
            // and `ngx.req.get_method` by a function kong builds in a
            // factory. Keeping them because nothing better was found is
            // how a resolver invents a dependency.
            if visible.len() < targets.len() {
                targets = visible;
                if !targets.is_empty() {
                    basis = "lexical_scope";
                }
            }
        }

        // A definition kept to its own file cannot answer a call from
        // another one. `e.tag.toLowerCase()` in one vue package was
        // answered by a `const toLowerCase` in another, `ngx.req` by a
        // `local` in kong, and 2681 of redis's calls by a `static` in a
        // file that never sees them.
        if !targets.is_empty() {
            let reachable = targets
                .iter()
                .copied()
                .filter(|target| {
                    graph_node(&context.graph, *target).is_none_or(|node| {
                        definition_is_reachable(node, caller_path, &exporting_files)
                    })
                })
                .collect::<Vec<_>>();
            if reachable.len() < targets.len() {
                targets = reachable;
                if !targets.is_empty() {
                    basis = "module_export";
                }
            }
        }

        // A type the project does not declare is outside it, whatever its
        // name looks like: cats writes `a: BigDecimal` and `a.pow(k)` is
        // scala's, not the algebra instance that happens to share the file.
        // An extension declared for that type is the exception -- the
        // project does declare that one, which is why it is recorded.
        if let Some(stated) = receiver_type
            && !stated.contains('.')
            && !stated.ends_with("()")
            && type_node_named(&context.graph, &type_names, stated).is_none()
            && !targets.iter().any(|target| {
                graph_node(&context.graph, *target)
                    .and_then(|node| node.metadata.get("reached_through"))
                    .is_some_and(|extended| extended == stated)
            })
        {
            add_external_call_placeholder(context, call, "external");
            continue;
        }

        // A receiver whose declared type comes from outside the repository
        // cannot be calling anything in it: `t.Fatalf()` on a `*testing.T` is
        // 7564 of terraform's calls, and reporting them as unresolved suggests
        // a resolver that failed rather than a dependency that left.
        if let Some((package, _)) =
            receiver_type.and_then(|receiver_type| receiver_type.split_once('.'))
            && context
                .file_import_qualifiers
                .get(call.span.path.as_str())
                .and_then(|qualifiers| qualifiers.get(package))
                == Some(&ImportedPackage::External)
        {
            add_external_call_placeholder(context, call, "external");
            continue;
        }

        // The receiver's declared type names the method's owner directly, so
        // it settles the choice that the label alone cannot: `b.Configure()`
        // inside `func (b *Backend)` is `Backend.Configure`. When the type is
        // qualified — `diags tfdiags.Diagnostics` — the package narrows it the
        // rest of the way: terraform declares `Diagnostics` in more than one,
        // so the owner's name alone still left a choice.
        if let Some(receiver_type) = receiver_type
            && targets.len() > 1
        {
            let (package, owner) = match receiver_type.rsplit_once('.') {
                Some((package, owner)) => (Some(package), owner),
                None => (None, receiver_type),
            };
            let package_candidates = package.and_then(|package| {
                match context
                    .file_import_qualifiers
                    .get(call.span.path.as_str())
                    .and_then(|qualifiers| qualifiers.get(package))
                    .cloned()
                    .map(|package| {
                        resolved_import_package(context, &mut scanned_candidates, package)
                    }) {
                    Some(ImportedPackage::Local(candidates)) => Some(candidates),
                    _ => None,
                }
            });
            let owned = targets
                .iter()
                .copied()
                .filter(|target| {
                    let Some(node) = graph_node(&context.graph, *target) else {
                        return false;
                    };
                    if node
                        .metadata
                        .get("owner_type")
                        .is_none_or(|declared| declared != owner)
                    {
                        return false;
                    }
                    package_candidates.as_deref().is_none_or(|candidates| {
                        node.span.as_ref().is_some_and(|span| {
                            candidates
                                .iter()
                                .any(|candidate| declared_in_module(&span.path, candidate))
                        })
                    })
                })
                .collect::<Vec<_>>();
            // An extension method is reached through the type it extends,
            // which is not the class that declares it:
            // `cancellationToken.ThrowIfCancelled()` is `AsyncUtils`' and
            // names `CancellationToken`. A method the type itself declares
            // wins over one declared for it, which is what both C# and
            // Kotlin do -- okio declares `Buffer.write` and an extension of
            // the same name, and reading them as one choice cost 79 answers.
            let owned = if owned.is_empty() {
                targets
                    .iter()
                    .copied()
                    .filter(|target| {
                        graph_node(&context.graph, *target)
                            .and_then(|node| node.metadata.get("reached_through"))
                            .is_some_and(|extended| extended == owner)
                    })
                    .collect::<Vec<_>>()
            } else {
                owned
            };
            if !owned.is_empty() {
                basis = "receiver_type";
                targets = owned;
            } else {
                // A method the receiver's type inherits is still reached
                // through it: Polly states 545 receivers whose type the
                // project declares and whose method is a base class's, and
                // cats writes `fa: CommutativeSemigroup[A]` for a `combine`
                // that `Semigroup` declares.
                let inherited_from = ancestor_type_names(&context.graph, &type_names, owner);
                let inherited = targets
                    .iter()
                    .copied()
                    .filter(|target| {
                        graph_node(&context.graph, *target)
                            .and_then(|node| node.metadata.get("owner_type"))
                            .is_some_and(|declared| {
                                inherited_from.iter().any(|name| name == declared)
                            })
                    })
                    .collect::<Vec<_>>();
                if !inherited.is_empty() {
                    basis = "receiver_type";
                    targets = inherited;
                } else if type_node_named(&context.graph, &type_names, owner)
                    .and_then(|node| node.metadata.get("extends"))
                    .is_some_and(|bases| {
                        bases.split(',').map(str::trim).any(|base| {
                            !base.is_empty()
                                && type_node_named(
                                    &context.graph,
                                    &type_names,
                                    base.rsplit('.').next().unwrap_or(base),
                                )
                                .is_none()
                        })
                    })
                {
                    // A type that embeds one the project does not declare
                    // reaches that one's methods through it: terraform
                    // embeds `sync.Mutex` and writes `p.Lock()`, which is
                    // the mutex's and not the 232 `Lock` methods it has.
                    add_external_call_placeholder(context, call, "external");
                    continue;
                }
            }
        }

        // A qualified call (`CodeGraph::new`, `Foo.bar`) matches many bare
        // `new`/`bar` declarations; keep only methods whose owning type is the
        // one named in the call, which turns an ambiguous set into one edge.
        // A ruby call keeps only the method in its label and states the
        // constant it was written through beside it, so the owner has to be
        // asked for rather than read off the label.
        let named_owner = split_qualified_call(&call.label)
            .map(|(owner, _)| owner.to_string())
            .or_else(|| call.receiver.clone());
        if let Some(owner) = named_owner.as_deref()
            && targets.len() > 1
        {
            // The file may have renamed the type the call is written
            // through: `using Assert = ..XUnitAssert;` makes every
            // `Assert.AreEqual` that class's, and Newtonsoft's tests write
            // 2199 of them.
            let owner = context
                .file_type_aliases
                .get(call.span.path.as_str())
                .and_then(|aliases| aliases.get(owner))
                .map(String::as_str)
                .unwrap_or(owner);
            let owned = targets
                .iter()
                .copied()
                .filter(|target| {
                    graph_node(&context.graph, *target)
                        .and_then(|node| node.metadata.get("owner_type"))
                        .is_some_and(|declared| declared == owner)
                })
                .collect::<Vec<_>>();
            if !owned.is_empty() {
                basis = "owner_type";
                targets = owned;
            } else {
                // A method the named class inherits is still reached
                // through it, so the parents answer before the name does.
                // `Path.Build.append_source` names a module written inside
                // path.ml, whose definitions belong to `Path`: the head of
                // the path answers when the whole of it does not.
                let mut reachable = ancestor_type_names(&context.graph, &type_names, owner);
                // The owner is the last segment of what the call writes, so
                // a nested module path has to be read from the label: the
                // definitions of `Path.Build.append_source` sit in path.ml
                // and belong to `Path`.
                if let Some((prefix, _)) = call
                    .label
                    .rsplit_once("::")
                    .or_else(|| call.label.rsplit_once('.'))
                    && let Some((head, _)) = prefix.split_once(['.', ':'])
                    && !head.is_empty()
                {
                    reachable.push(head.to_string());
                }
                let inherited = targets
                    .iter()
                    .copied()
                    .filter(|target| {
                        graph_node(&context.graph, *target)
                            .and_then(|node| node.metadata.get("owner_type"))
                            .is_some_and(|declared| reachable.contains(declared))
                    })
                    .collect::<Vec<_>>();
                // A module written inside the caller's file is reachable
                // whatever the graph managed to name it: dune's
                // memo_tests_env.ml declares a `module Memo` of its own,
                // and `Memo.of_thunk` there is that one. Only OCaml and
                // julia need the escape -- everywhere else a definition
                // carries the type it belongs to, so a `new` of another
                // type in the same file is not what `SearcherBuilder::new`
                // means, and ripgrep's printer declares three of those.
                let in_file = matches!(call.language.as_str(), "ocaml" | "julia")
                    && targets.iter().copied().any(|target| {
                        graph_node(&context.graph, target)
                            .and_then(|node| node.span.as_ref())
                            .map(|span| span.path.as_str())
                            == caller_path
                    });
                if !inherited.is_empty() {
                    basis = "owner_type";
                    targets = inherited;
                } else if !in_file
                    && (declared_owners.contains(owner)
                        || type_node_named(&context.graph, &type_names, owner).is_some())
                {
                    // The project declares the class or module the call
                    // names, and none of these definitions belongs to it:
                    // `Account.new` is not the `new` action of the
                    // twenty-three controllers that have one, and dune's
                    // `List.map` is not the `map` of fifty-nine other
                    // modules. What the call means is either the class
                    // itself, which the constructor path below answers, or
                    // something outside the project.
                    targets.clear();
                }
            }
        }
        // The type a field chain names is written unqualified, and an
        // unqualified type name in Go means the caller's own package:
        // terraform declares `LocalValue` in two of them, and
        // `v.LocalValue.String` is the one written next door. Only a chain
        // whose type the owner rule already recognised is narrowed this way:
        // where the name in front is a field rather than a type, the package
        // holds several `Close` and picking one invents a cycle.
        if call.language == "go"
            && written_through_a_chain_of_values
            && basis == "owner_type"
            && targets.len() > 1
            && let Some((directory, _)) = caller_path.and_then(|path| path.rsplit_once('/'))
        {
            let same_package = targets
                .iter()
                .copied()
                // The definition the call sits in is the nearest candidate
                // the package would reach for, and it is not what a field
                // hands back: gqlgen writes `r.QueryResolver.Users` inside
                // `stubQuery.Users`, which the package would answer with
                // itself.
                .filter(|target| *target != call.caller)
                .filter(|target| {
                    graph_node(&context.graph, *target)
                        .and_then(|node| node.span.as_ref())
                        .is_some_and(|span| {
                            span.path
                                .rsplit_once('/')
                                .is_some_and(|(candidate, _)| candidate == directory)
                        })
                })
                .collect::<Vec<_>>();
            if !same_package.is_empty() {
                targets = same_package;
                basis = "package";
            }
        }

        // OCaml names a module after the file that holds it, and that is
        // enough to find a definition rather than only to narrow between
        // several: `List.map` is `map` in list.ml. The rule was already
        // being used the second way and settles 11214 of dune's ambiguous
        // calls, but a call with no candidate at all never reached it, so
        // dune's own `stdune` answered none of its 683 `List.map`.
        if targets.is_empty()
            && let Some((module, method)) = split_qualified_call(&call.label)
        {
            let in_module_file: Vec<NodeId> =
                resolve_function_targets(&context.function_symbols, method)
                    .into_iter()
                    .filter(|target| {
                        graph_node(&context.graph, *target)
                            .and_then(|node| node.metadata.get("language"))
                            .is_some_and(|language| language == &call.language)
                    })
                    .filter(|target| {
                        graph_node(&context.graph, *target)
                            .and_then(|node| node.span.as_ref())
                            .is_some_and(|span| {
                                module_named_file(&call.language, &span.path, module)
                            })
                    })
                    // Looking a name up again must not walk around what the
                    // first look-up refused: the program does not call its
                    // own suite, however the module is named.
                    .filter(|target| {
                        caller_is_test
                            || graph_node(&context.graph, *target)
                                .and_then(|node| node.span.as_ref())
                                .is_none_or(|span| {
                                    !is_test_like_source_path(&span.path)
                                        || is_shipped_but_not_written(&span.path)
                                })
                    })
                    .collect();
            if !in_module_file.is_empty() {
                targets = in_module_file;
                basis = "module_file";
            }
        }
        let mut ambiguous_candidates_are_types = false;
        if targets.is_empty() {
            let type_targets = resolve_function_targets(&context.type_symbols, &call.label)
                .into_iter()
                .filter(|target| {
                    graph_node(&context.graph, *target)
                        .and_then(|node| node.metadata.get("language"))
                        .is_some_and(|language| language == &call.language)
                })
                // A type written inside another one is not what a bare name
                // in a different file means. `object Ior { case class Right }`
                // was the only `Right` cats declares, so every `Right(...)`
                // built for scala's `Either` reached it -- 134 of its 151
                // references -- and made `Ior.Right` and `Ior.Left` the two
                // largest hubs in the project. Reaching it by its bare name
                // takes an import that names it, which the qualified path
                // below answers instead.
                .filter(|target| {
                    let Some(node) = graph_node(&context.graph, *target) else {
                        return false;
                    };
                    if !node.metadata.contains_key("owner_type") {
                        return true;
                    }
                    let same_file = node
                        .span
                        .as_ref()
                        .is_some_and(|span| span.path == call.span.path);
                    // `import cats.data.Validated.{Valid, Invalid}` binds the
                    // bare name, and then it does mean the nested type. A
                    // file whose import list cannot be complete is given the
                    // benefit of the doubt rather than a wrong answer.
                    same_file
                        || context
                            .file_imported_names
                            .get(call.span.path.as_str())
                            .is_some_and(|names| names.contains_key(&call.label))
                        || context.file_wildcard_imports.contains(&call.span.path)
                })
                .collect::<Vec<_>>();
            if type_targets.len() == 1 {
                // Building a class runs its constructor, and that is where
                // a framework hands it what it needs: koel writes 391
                // `__construct` methods and nothing called any of them,
                // because `new SongService($repository)` reached the class
                // and stopped there.
                if let Some(constructor) = class_constructor(&context.graph, type_targets[0]) {
                    add_edge_once_with_metadata(
                        context,
                        call.caller,
                        constructor,
                        EdgeKind::Calls,
                        Confidence::Syntactic,
                        BTreeMap::from([
                            ("call_label".to_string(), call.label.clone()),
                            ("resolution".to_string(), "constructor".to_string()),
                            ("language".to_string(), call.language.clone()),
                            ("file".to_string(), call.span.path.clone()),
                            ("line".to_string(), call.span.start_line.to_string()),
                        ]),
                    );
                }
                add_edge_once_with_metadata(
                    context,
                    call.caller,
                    type_targets[0],
                    EdgeKind::References,
                    Confidence::Syntactic,
                    BTreeMap::from([
                        ("relation".to_string(), "constructor_reference".to_string()),
                        ("type_label".to_string(), call.label),
                        ("language".to_string(), call.language),
                        // The file beside the line, as a call edge carries
                        // it: an edge a reader can place without looking
                        // its source node up first.
                        ("file".to_string(), call.span.path.clone()),
                        ("line".to_string(), call.span.start_line.to_string()),
                    ]),
                );
                continue;
            }
            // Several declarations answer to the name: a Scala case class
            // and its companion object, or the 53 `Ops` traits in cats.
            // Which one the call builds is a real question the syntax does
            // not settle, and reporting it as unresolved claims nothing was
            // found when in fact several were. Let it join the ambiguous
            // path below, which records the candidates.
            // A name the language provides is a definite answer, and
            // several project declarations sharing it is not: cats is
            // cross-built, so its `compat/Seq.scala` exists once per Scala
            // version, yet a bare `Seq(...)` is the standard library's.
            let is_builtin = builtin_call_target(&call.language, &call.label)
                || (call.language == "ruby" && ruby_core_method(&call.label))
                || objc_platform_receiver(&call.language, call.receiver.as_deref())
                || environment_provides_call(&call.language, &call.span.path, &call.label)
                || test_runner_provides_call(&call.language, &call.span.path, &call.label)
                || standard_library_module_call(&call.language, &call.label);
            if type_targets.len() > 1 && !is_builtin {
                targets = type_targets;
                ambiguous_candidates_are_types = true;
            } else {
                let key = (call.language.clone(), call.label.clone());
                // A ruby class that descends from outside the project
                // answers with its base's methods, and none of them is
                // among what this project declares: `where`, `present?`,
                // `redirect_to` and `permit` are ActiveRecord's and
                // ActionController's. mastodon writes 3684 such calls from
                // inside a class whose ancestry leaves it, and reporting
                // them as unresolved says a resolver failed where a gem
                // simply provides them -- the same thing the constant
                // receiver already says for a call written through one.
                let inherits_from_outside = call.language == "ruby"
                    && graph_node(&context.graph, call.caller)
                        .and_then(|node| node.metadata.get("owner_type").cloned())
                        .is_some_and(|owner| {
                            let ancestors =
                                ancestor_type_names(&context.graph, &type_names, &owner);
                            !ancestors.is_empty()
                                && ancestors.iter().any(|name| {
                                    type_node_named(&context.graph, &type_names, name).is_none()
                                })
                        });
                // A shell command that is not a function this project
                // declares and not the shell's own is on PATH: the shell
                // has no third way to name one, so `unresolved` says a
                // resolver failed where nothing was ever there to find.
                // Not one of redis's 424 or shellcheck's 58 unresolved
                // shell calls names a function either project declares.
                // `gun:open(..)` names the module that answers it, and a
                // module this project has no file for is a dependency's.
                // A capitalised head is an erlang variable holding a module
                // name, which says nothing about where it points.
                let names_a_foreign_erlang_module = call.language == "erlang"
                    && call
                        .label
                        .split_once(':')
                        .filter(|(module, _)| {
                            module
                                .chars()
                                .next()
                                .is_some_and(|first| first.is_lowercase())
                        })
                        .is_some_and(|(module, _)| !erlang_modules.contains(module));
                let comes_from_the_environment = names_a_foreign_erlang_module
                    || call.language == "bash"
                    // `lifecycle::signal_stage` names the package that
                    // answers it, and it is not this one. dplyr writes 189
                    // such calls and reporting them says a resolver failed
                    // to find what the source says is elsewhere.
                    || (call.language == "r" && call.label.contains("::"));
                let resolution = if is_builtin {
                    "builtin"
                } else if inherits_from_outside
                    || comes_from_the_environment
                    // A call through a value the body binds, and a name the
                    // file never imports, are each said more precisely
                    // below; this is the case where nothing more is known
                    // than that no definition here is called that.
                    || (no_definition_has_that_name
                        && !call.callee_is_value
                        && !name_is_not_imported
                        // `Transport:send(..)` holds the module in a
                        // variable, so the name in the label is not the
                        // name of anything: erlang capitalises variables.
                        && !(call.language == "erlang"
                            && call.label.split_once(':').is_some_and(|(module, _)| {
                                module
                                    .chars()
                                    .next()
                                    .is_some_and(|first| first.is_uppercase())
                            })))
                {
                    "external"
                } else {
                    "unresolved"
                };
                let call_id = if let Some(id) = context.unresolved_call_placeholders.get(&key) {
                    *id
                } else {
                    let mut metadata = BTreeMap::new();
                    metadata.insert("language".to_string(), call.language.clone());
                    metadata.insert("parser".to_string(), "tree-sitter".to_string());
                    metadata.insert("item_kind".to_string(), "call".to_string());
                    metadata.insert("resolution".to_string(), resolution.to_string());
                    let id = context.graph.add_node_with_metadata(
                        NodeKind::ExternalDependency,
                        call.label.clone(),
                        Some(call.span.clone()),
                        metadata,
                    );
                    context.unresolved_call_placeholders.insert(key, id);
                    id
                };
                // The same provenance the other branches record: without it these
                // edges alone could not say what was called or where, so a UI
                // could not open the call site and the semantic pass could not ask
                // about it.
                let mut edge_metadata = BTreeMap::from([
                    ("call_label".to_string(), call.label),
                    ("resolution".to_string(), resolution.to_string()),
                    ("language".to_string(), call.language),
                    ("file".to_string(), call.span.path.clone()),
                    ("line".to_string(), call.span.start_line.to_string()),
                    ("column".to_string(), call.span.start_column.to_string()),
                ]);
                // A call through a value the body binds has nothing to
                // find, which is not the same as a resolver that failed.
                if call.callee_is_value && resolution == "unresolved" {
                    edge_metadata
                        .insert("unresolved_reason".to_string(), "local_value".to_string());
                } else if name_is_not_imported && resolution == "unresolved" {
                    // Nor is a name the file never imports: the module
                    // cannot reach it, so it is a value the body binds or
                    // something the runtime provides.
                    edge_metadata
                        .insert("unresolved_reason".to_string(), "not_imported".to_string());
                }
                add_edge_once_with_metadata(
                    context,
                    call.caller,
                    call_id,
                    EdgeKind::Calls,
                    Confidence::Heuristic,
                    edge_metadata,
                );
                continue;
            }
        }

        // Overloads are not a choice. `JsonConvert.SerializeObject` has six
        // signatures, and a caller means the method, not one of them —
        // 7554 calls across the corpora were reported as ambiguous when
        // every candidate was the same method of the same type. Swift and
        // C# spread a type over several files through extensions and
        // partial classes, so the test is the owner, not the file.
        // OCaml names a module after the file that holds it: `Json.assoc`
        // is `assoc` in json.ml, and `Stdune.Json.assoc` is the same file.
        // That is the language's own rule rather than a guess about where
        // a name might live, and it settles 11214 of dune's ambiguous
        // calls on its own.
        if targets.len() > 1
            && let Some((module, _)) = split_qualified_call(&call.label)
        {
            let in_module_file = targets
                .iter()
                .copied()
                .filter(|target| {
                    graph_node(&context.graph, *target)
                        .and_then(|node| node.span.as_ref())
                        .is_some_and(|span| module_named_file(&call.language, &span.path, module))
                })
                .collect::<Vec<_>>();
            // Several definitions can sit in the named file -- kong writes
            // `exit` twice in pdk/response.lua, once per subsystem -- and
            // narrowing to them is still the answer to where the call goes.
            if !in_module_file.is_empty() && in_module_file.len() < targets.len() {
                targets = in_module_file;
                basis = "module_file";
            }
        }

        // Go resolves an unqualified name inside its own package, and a
        // package is a directory: `is_bin_in_path()` in gqlgen names the
        // one its own directory declares, whatever the other twenty are.
        // That is the language's rule rather than a guess about where a
        // name lives.
        if targets.len() > 1
            && call.language == "go"
            && !call.label.contains('.')
            && let Some(directory) = call
                .span
                .path
                .rsplit_once('/')
                .map(|(directory, _)| directory)
        {
            let in_package = targets
                .iter()
                .copied()
                .filter(|target| {
                    graph_node(&context.graph, *target)
                        .and_then(|node| node.span.as_ref())
                        .is_some_and(|span| {
                            span.path
                                .rsplit_once('/')
                                .is_some_and(|(candidate, _)| candidate == directory)
                        })
                })
                .collect::<Vec<_>>();
            if !in_package.is_empty() && in_package.len() < targets.len() {
                targets = in_package;
                basis = "package";
            }
        }

        // A C or C++ file reaches a declaration through the headers it
        // includes, and nothing else: nlohmann's `JSON_THROW` is declared
        // in `detail/macro_scope.hpp` and again in the amalgamated
        // `single_include/nlohmann/json.hpp`, and a caller under
        // `include/` includes only the first.
        if matches!(call.language.as_str(), "c" | "cpp") && targets.len() > 1 {
            let reachable = targets
                .iter()
                .copied()
                .filter(|target| {
                    let Some(path) = graph_node(&context.graph, *target)
                        .and_then(|node| node.span.as_ref())
                        .map(|span| span.path.as_str())
                    else {
                        return false;
                    };
                    Some(path) == caller_path
                        || caller_path.is_some_and(|caller| {
                            included
                                .get(caller)
                                .is_some_and(|headers| headers.contains(path))
                        })
                })
                .collect::<Vec<_>>();
            if !reachable.is_empty() && reachable.len() < targets.len() {
                targets = reachable;
                basis = "include";
            }
        }
        // A method on a type from a package the file never imports cannot
        // be the one meant. terraform declares `Diagnostics.HasErrors` in
        // `internal/policy` and in `internal/tfdiags`, and every file that
        // calls it imports exactly one of the two -- 3019 of its ambiguous
        // calls have candidates in more than one package and name only one
        // of those packages in their imports. A package is a directory, and
        // the caller's own needs no import.
        if call.language == "go" && targets.len() > 1 {
            let caller_directory = caller_path
                .and_then(|path| path.rsplit_once('/'))
                .map(|(directory, _)| directory);
            let imported: BTreeSet<&str> = context
                .file_import_qualifiers
                .get(call.span.path.as_str())
                .map(|qualifiers| {
                    qualifiers
                        .values()
                        .filter_map(|package| match package {
                            ImportedPackage::Local(candidates) => Some(candidates),
                            ImportedPackage::External => None,
                        })
                        .flatten()
                        .map(|candidate| candidate.trim_end_matches('/'))
                        .collect()
                })
                .unwrap_or_default();
            if !imported.is_empty() {
                let reachable = targets
                    .iter()
                    .copied()
                    .filter(|target| {
                        graph_node(&context.graph, *target)
                            .and_then(|node| node.span.as_ref())
                            .and_then(|span| span.path.rsplit_once('/'))
                            .is_some_and(|(directory, _)| {
                                Some(directory) == caller_directory || imported.contains(directory)
                            })
                    })
                    .collect::<Vec<_>>();
                if !reachable.is_empty() && reachable.len() < targets.len() {
                    targets = reachable;
                    basis = "import";
                }
            }
        }

        // The module's files are what chose, and `name` would say the
        // name matched and nothing else did.
        if narrowed_to_the_module && basis == "name" && !targets.is_empty() {
            basis = "module_file";
        }
        let overloads = targets.len() > 1
            && (one_methods_overloads(&context.graph, &targets)
                || one_macros_arms(&context.graph, &targets));
        if overloads && basis == "name" {
            basis = "overload";
        }

        // A syntactic label such as `build`, `read`, or `close` is often
        // shared by hundreds of methods. Connecting the caller to every
        // matching declaration invents dependencies and makes E grow toward
        // O(call-sites * duplicate-labels). Preserve the uncertainty as one
        // bounded node instead; semantic enrichment can replace it later.
        if targets.len() > 1 && !overloads {
            // Which definitions the call might have meant. Without this the
            // placeholder is the only thing that records the ambiguity, so
            // none of the candidates has an incoming edge and every one of
            // them reads as a function nobody calls: cats declares `eqv` 72
            // times, 46 of them with no caller, while 39 unsettled calls
            // reach for that name. Matching the name again in the insight
            // instead of saying so here would be four times too wide --
            // terraform's 77707 candidates against 333455 same-name
            // declarations -- because the narrowing that got here is gone
            // by then.
            for target in &targets {
                add_node_metadata(
                    &mut context.graph,
                    *target,
                    "may_be_called_by",
                    "unsettled_call",
                );
            }
            let key = (call.language.clone(), call.label.clone());
            let call_id = if let Some(id) = context.unresolved_call_placeholders.get(&key) {
                *id
            } else {
                let mut metadata = BTreeMap::new();
                metadata.insert("language".to_string(), call.language.clone());
                metadata.insert("parser".to_string(), "tree-sitter".to_string());
                metadata.insert("item_kind".to_string(), "call".to_string());
                metadata.insert("resolution".to_string(), "ambiguous".to_string());
                metadata.insert("candidate_count".to_string(), targets.len().to_string());
                let sample = targets
                    .iter()
                    .filter_map(|target| graph_node(&context.graph, *target))
                    .take(5)
                    .map(|node| {
                        node.span
                            .as_ref()
                            .map(|span| format!("{}:{}", span.path, node.label))
                            .unwrap_or_else(|| node.label.clone())
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                metadata.insert("candidate_sample".to_string(), sample);
                if ambiguous_candidates_are_types {
                    // Say what the candidates are, so a reader knows the
                    // call builds one of several types rather than picking
                    // between same-named functions.
                    metadata.insert("candidate_kind".to_string(), "type".to_string());
                    metadata.insert("relation".to_string(), "constructor_reference".to_string());
                }
                let id = context.graph.add_node_with_metadata(
                    NodeKind::ExternalDependency,
                    call.label.clone(),
                    Some(call.span.clone()),
                    metadata,
                );
                context.unresolved_call_placeholders.insert(key, id);
                id
            };
            add_edge_once_with_metadata(
                context,
                call.caller,
                call_id,
                EdgeKind::Calls,
                Confidence::Heuristic,
                BTreeMap::from([
                    ("call_label".to_string(), call.label),
                    ("resolution".to_string(), "ambiguous".to_string()),
                    ("language".to_string(), call.language),
                    ("file".to_string(), call.span.path.clone()),
                    ("line".to_string(), call.span.start_line.to_string()),
                    ("column".to_string(), call.span.start_column.to_string()),
                ]),
            );
            continue;
        }

        // One method's overloads are one method, and one call site is one
        // call: linking it to each declaration turned Newtonsoft's 838
        // `DeserializeObject` call sites into 6704 edges and filled that
        // project's hotspot list with six copies of the same name.
        let overload_count = targets.len();
        if overloads && overload_count > 1 {
            targets.sort_by_key(|target| {
                graph_node(&context.graph, *target)
                    .and_then(|node| node.span.as_ref())
                    .map(|span| (span.path.clone(), span.start_line))
            });
            targets.truncate(1);
        }

        // The call site, not the caller's declaration: a semantic pass asking
        // "what is defined here?" has to ask at the call, and click-to-source
        // on a call edge should land on the call.
        let mut metadata = BTreeMap::new();
        metadata.insert("call_label".to_string(), call.label.clone());
        metadata.insert("resolution".to_string(), "resolved".to_string());
        metadata.insert("language".to_string(), call.language);
        metadata.insert("file".to_string(), call.span.path.clone());
        metadata.insert("line".to_string(), call.span.start_line.to_string());
        metadata.insert("column".to_string(), call.span.start_column.to_string());
        metadata.insert("resolution_basis".to_string(), basis.to_string());
        if overloads && overload_count > 1 {
            metadata.insert("overload_count".to_string(), overload_count.to_string());
        }
        // Matching a name across the repository is a guess; everything else
        // here followed something the syntax states outright.
        let confidence = if basis == "name" {
            Confidence::Heuristic
        } else {
            Confidence::Syntactic
        };

        for target in targets {
            add_edge_once_with_metadata(
                context,
                call.caller,
                target,
                EdgeKind::Calls,
                confidence,
                metadata.clone(),
            );
        }
    }
}

/// Which files each file includes, read from the imports the scan
/// resolved. A C translation unit reaches a declaration only through a
/// header it includes, and that is what tells two types of the same name
/// apart.
fn included_files(graph: &CodeGraph) -> BTreeMap<String, BTreeSet<String>> {
    let mut by_id: BTreeMap<NodeId, &str> = BTreeMap::new();
    for node in &graph.nodes {
        if node.kind == NodeKind::File {
            by_id.insert(node.id, node.label.as_str());
        }
    }
    // An import node sits between the file and what it imports.
    let mut import_targets: BTreeMap<NodeId, &str> = BTreeMap::new();
    for edge in &graph.edges {
        if edge
            .metadata
            .get("relation")
            .is_some_and(|relation| relation == "local_import_file")
            && let Some(path) = by_id.get(&edge.target)
        {
            import_targets.insert(edge.source, path);
        }
    }
    let mut included: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for edge in &graph.edges {
        if edge.kind != EdgeKind::Imports {
            continue;
        }
        let (Some(source), Some(target)) =
            (by_id.get(&edge.source), import_targets.get(&edge.target))
        else {
            continue;
        };
        included
            .entry((*source).to_string())
            .or_default()
            .insert((*target).to_string());
    }
    included
}

/// Whether a file describes the database schema at a point in time
/// rather than the program: a Rails migration declares the model classes
/// it touches so that the data it moves keeps working, and a Laravel or
/// Django migration does the same.
fn declares_a_schema_snapshot(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    normalized.split('/').any(|segment| {
        matches!(
            segment,
            "migrate" | "post_migrate" | "migrations" | "versions"
        )
    })
}

pub(crate) fn resolve_pending_type_references(context: &mut IndexContext) {
    let pending = std::mem::take(&mut context.pending_type_references);
    let mut seen = BTreeSet::new();
    // What each file includes, for the languages where a header is how a
    // file reaches a declaration: redis declares `client` in `server.h`
    // and again in `redis-benchmark.c`, and only the include says which
    // one a file means.
    let included = included_files(&context.graph);

    for reference in pending {
        let source_path = graph_node(&context.graph, reference.source)
            .and_then(|node| node.span.as_ref())
            .map(|span| span.path.as_str());
        let targets = resolve_function_targets(&context.type_symbols, &reference.label)
            .into_iter()
            .filter(|target| {
                let Some(node) = graph_node(&context.graph, *target) else {
                    return false;
                };
                let same_language = node
                    .metadata
                    .get("language")
                    .is_some_and(|language| languages_share_symbols(language, &reference.language));
                let same_file = source_path
                    .is_some_and(|path| node.span.as_ref().is_some_and(|span| span.path == path));
                // A configuration's declarations refer to each other inside
                // one file as a matter of course — `subnet_id =
                // module.vpc.id` sits beside the module it names — and that
                // reference is the dependency a reader is looking for.
                let within_a_file_is_a_fact = matches!(
                    reference.language.as_str(),
                    "hcl" | "nix" | "proto" | "graphql" | "solidity"
                );
                same_language && (within_a_file_is_a_fact || !same_file)
            })
            .collect::<Vec<_>>();

        // A Terraform module is a directory, and so is a Go package and a
        // Kotlin source set: a name written inside one means what that
        // directory declares. okio declares `Buffer` three times -- once
        // for every platform it builds for -- in a directory each.
        // terraform's fixtures declare `var.input` in 40 directories and
        // its backends declare `Backend` in seventeen, and only the one
        // next door is what the expression means.
        let targets = if matches!(reference.language.as_str(), "hcl" | "go" | "kotlin")
            && targets.len() > 1
        {
            let directory = source_path
                .and_then(|path| path.rsplit_once('/'))
                .map(|(dir, _)| dir);
            let neighbours = targets
                .iter()
                .copied()
                .filter(|target| {
                    graph_node(&context.graph, *target)
                        .and_then(|node| node.span.as_ref())
                        .is_some_and(|span| {
                            span.path.rsplit_once('/').map(|(dir, _)| dir) == directory
                        })
                })
                .collect::<Vec<_>>();
            if neighbours.is_empty() {
                targets
            } else {
                neighbours
            }
        } else {
            targets
        };

        // A C file reaches a declaration by including the header that
        // holds it: redis declares `client` in `server.h` and again in
        // `redis-benchmark.c`, and the file that includes one of them
        // means that one.
        let targets = if matches!(reference.language.as_str(), "c" | "cpp") && targets.len() > 1 {
            let reachable: Vec<NodeId> = source_path
                .and_then(|path| included.get(path))
                .map(|headers| {
                    targets
                        .iter()
                        .copied()
                        .filter(|target| {
                            graph_node(&context.graph, *target)
                                .and_then(|node| node.span.as_ref())
                                .is_some_and(|span| headers.contains(&span.path))
                        })
                        .collect()
                })
                .unwrap_or_default();
            if reachable.len() == 1 {
                reachable
            } else {
                targets
            }
        } else {
            targets
        };

        // A name written on its own means the declaration that answers to
        // exactly that name: mastodon's `Account` is the model, not the
        // `Mastodon::CLI::Maintenance::Account` stub or the fifteen a
        // migration declares, all of which end with the same word.
        let targets = if targets.len() > 1 {
            let exact: Vec<NodeId> = targets
                .iter()
                .copied()
                .filter(|target| {
                    graph_node(&context.graph, *target)
                        .is_some_and(|node| node.label == reference.label)
                })
                .collect();
            if exact.len() == 1 { exact } else { targets }
        } else {
            targets
        };

        // A migration declares the class it needs to describe the schema
        // at that point in time: mastodon writes `class Account <
        // ApplicationRecord; end` in fifteen of them, so the model every
        // other file means had sixteen declarations and no reference could
        // choose one.
        let targets = if targets.len() > 1 {
            let outside: Vec<NodeId> = targets
                .iter()
                .copied()
                .filter(|target| {
                    graph_node(&context.graph, *target)
                        .and_then(|node| node.span.as_ref())
                        .is_some_and(|span| !declares_a_schema_snapshot(&span.path))
                })
                .collect();
            if outside.len() == 1 { outside } else { targets }
        } else {
            targets
        };

        // Type labels are exact only when one declaration exists in the
        // scanned language. Multiple declarations remain unresolved instead
        // of manufacturing fan-out edges.
        if targets.len() != 1 {
            continue;
        }
        let target = targets[0];
        // A variable's own `validation` block reads the variable; that is
        // the declaration stating its rule, not a dependency on itself.
        if target == reference.source {
            continue;
        }
        if !seen.insert((reference.source, target)) {
            continue;
        }
        add_edge_once_with_metadata(
            context,
            reference.source,
            target,
            EdgeKind::References,
            Confidence::Syntactic,
            BTreeMap::from([
                ("relation".to_string(), "type_reference".to_string()),
                ("type_label".to_string(), reference.label),
                ("language".to_string(), reference.language),
                ("file".to_string(), reference.span.path.clone()),
                ("line".to_string(), reference.span.start_line.to_string()),
            ]),
        );
    }
}

pub(crate) fn graph_node(graph: &CodeGraph, id: NodeId) -> Option<&codegraph_core::Node> {
    id.0.checked_sub(1)
        .and_then(|index| graph.nodes.get(index as usize))
        .filter(|node| node.id == id)
        .or_else(|| graph.nodes.iter().find(|node| node.id == id))
}

/// The definition a Rust `use` names when no module file answers it.
/// `use crate::parse_cli_node_id;` and `use crate::cli;` are written the
/// same way, and only what the project holds tells them apart.
fn rust_imported_item(context: &IndexContext, import: &PendingLocalImport) -> Option<NodeId> {
    if graph_node(&context.graph, import.import_node)
        .and_then(|node| node.metadata.get("language"))
        .map(String::as_str)
        != Some("rust")
    {
        return None;
    }
    let [target] = resolve_function_targets(&context.function_symbols, &import.target)[..] else {
        return None;
    };
    Some(target)
}

/// Whether a definition can answer a call from `caller_path`. What
/// "private" reaches differs by language, and so does what the fact rests
/// on: an export list, an interface file beside the module, a `static`, a
/// `local`.
fn definition_is_reachable(
    node: &codegraph_core::Node,
    caller_path: Option<&str>,
    exporting_files: &BTreeSet<String>,
) -> bool {
    let Some(path) = node.span.as_ref().map(|span| span.path.as_str()) else {
        return true;
    };
    if caller_path == Some(path) {
        return true;
    }
    if node.metadata.get("visibility").map(String::as_str) != Some("private") {
        return true;
    }
    let language = node
        .metadata
        .get("language")
        .map(String::as_str)
        .unwrap_or_default();
    match language {
        "javascript" | "typescript" | "tsx" => {
            // A method is reached through its type, whose own export
            // governs it, and a file that exports nothing says nothing
            // about what is private: CommonJS hands its functions out
            // through `module.exports`.
            node.metadata.contains_key("owner_type") || !exporting_files.contains(path)
        }
        // A `static` function belongs to the translation unit compiling
        // it -- unless it sits in a header, which every file that includes
        // it compiles for itself.
        "c" | "cpp" => matches!(
            Path::new(path)
                .extension()
                .and_then(|extension| extension.to_str()),
            Some("h") | Some("hpp") | Some("hh") | Some("hxx") | Some("inc")
        ),
        // An interface file states a module's whole surface, a `local` Lua
        // function is its file's, an unexported Erlang or Haskell name
        // cannot be written from outside, and none of Java, Kotlin, Scala,
        // PHP or Zig spreads a type across files.
        "ocaml" | "lua" | "erlang" | "haskell" | "php" | "java" | "kotlin" | "scala" | "zig" => {
            false
        }
        // A Rust item without `pub` belongs to its module, which its child
        // modules can see and nothing else can: ripgrep declares a private
        // `fn unwrap` in one crate, and 374 `.unwrap()` calls from every
        // other crate resolved to it, making it the project's top hotspot.
        "rust" => caller_path.is_some_and(|caller| {
            rust_module_children(path).is_some_and(|dir| caller.starts_with(&dir))
        }),
        // Go's lowercase reaches its whole package, Python's and Dart's
        // underscore is a convention, Ruby's `private` only refuses an
        // explicit receiver, and C# and Swift spread a type across files
        // with partial classes and extensions.
        _ => true,
    }
}

/// The directory holding the child modules of a Rust file: `src/foo.rs` and
/// `src/foo/mod.rs` both own `src/foo/`.
fn rust_module_children(path: &str) -> Option<String> {
    let (directory, file) = path.rsplit_once('/')?;
    let stem = file.strip_suffix(".rs")?;
    Some(if stem == "mod" || stem == "lib" || stem == "main" {
        format!("{directory}/")
    } else {
        format!("{directory}/{stem}/")
    })
}

/// [`graph_node`] for a caller that means to change what it finds.
pub(crate) fn graph_node_mut(
    graph: &mut CodeGraph,
    id: NodeId,
) -> Option<&mut codegraph_core::Node> {
    let index = id.0.checked_sub(1).map(|index| index as usize);
    if index.is_some_and(|index| graph.nodes.get(index).is_some_and(|node| node.id == id)) {
        return graph.nodes.get_mut(index.unwrap_or_default());
    }
    graph.nodes.iter_mut().find(|node| node.id == id)
}

/// Attach an identity that survives an edit. Numeric `n42` ids stay as the
/// compact in-memory key; `stable_id` is the durable handle for agents,
/// bookmarks, and cross-scan investigation memory.
///
/// It is built from what the node is -- its kind, file, name and language --
/// and not from where in the file it sits: a function added at the top of a
/// file used to change the id of every function below it, so a saved handle
/// went stale on an edit that had nothing to do with it. When one file
/// declares a name more than once, the order they are declared in tells them
/// apart.
pub(crate) fn annotate_stable_node_ids(graph: &mut CodeGraph) {
    let mut used = BTreeSet::new();
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for node in &mut graph.nodes {
        let path = node
            .span
            .as_ref()
            .map(|span| span.path.as_str())
            .or_else(|| (node.kind == NodeKind::File).then_some(node.label.as_str()))
            .unwrap_or("");
        let identity = format!(
            "{}\0{}\0{}\0{}\0{}",
            node_kind_name(&node.kind),
            path,
            node.label,
            node.metadata
                .get("language")
                .map(String::as_str)
                .unwrap_or(""),
            node.metadata
                .get("item_kind")
                .map(String::as_str)
                .unwrap_or("")
        );
        let ordinal = seen.entry(identity.clone()).or_insert(0);
        let canonical = format!("{identity}\0{ordinal}");
        *ordinal += 1;
        let mut hash = stable_fnv1a64(canonical.as_bytes());
        while !used.insert(hash) {
            hash = hash.wrapping_add(1);
        }
        node.metadata
            .insert("stable_id".to_string(), format!("cg-{hash:016x}"));
    }
}

pub(crate) fn stable_fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Link a namespace import to the namespace the project declares. C# writes
/// `using Polly.Telemetry;` and declares that namespace in another file
/// entirely; 417 of Polly's 742 usings name one of its own.
pub(crate) fn resolve_pending_namespace_imports(context: &mut IndexContext) {
    let pending = std::mem::take(&mut context.pending_namespace_imports);
    for import in pending {
        let Some(namespace_id) = context
            .namespace_nodes
            .get(&(import.language, import.namespace.clone()))
            .copied()
        else {
            continue;
        };
        add_node_metadata(
            &mut context.graph,
            import.import_node,
            "import_scope",
            "local",
        );
        add_node_metadata(
            &mut context.graph,
            import.import_node,
            "import_target",
            import.namespace.clone(),
        );
        add_node_metadata(
            &mut context.graph,
            import.import_node,
            "resolution",
            "resolved",
        );
        add_node_metadata(
            &mut context.graph,
            import.import_node,
            "resolved_namespace",
            import.namespace.clone(),
        );
        let mut metadata = BTreeMap::new();
        metadata.insert("relation".to_string(), "namespace_import".to_string());
        metadata.insert("source".to_string(), "syntax".to_string());
        metadata.insert("resolution".to_string(), "namespace_import".to_string());
        metadata.insert("target".to_string(), import.namespace);
        add_edge_once_with_metadata(
            context,
            import.import_node,
            namespace_id,
            EdgeKind::References,
            Confidence::Syntactic,
            metadata,
        );
    }
}

pub(crate) fn resolve_pending_local_imports(context: &mut IndexContext) {
    let pending_imports = std::mem::take(&mut context.pending_local_imports);
    // What the scan holds does not change while imports are resolved.
    let files_by_name = files_by_name(&context.file_nodes);

    for import in pending_imports {
        // The node's id is its position: walking every node to find it cost
        // terraform's 19000 imports a pass over 133000 nodes each.
        let source_path = graph_node(&context.graph, import.import_node)
            .and_then(|node| node.span.as_ref())
            .map(|span| span.path.clone());
        let resolved = resolve_local_import_candidate(
            &context.file_nodes,
            &files_by_name,
            &import.candidates,
            source_path.as_deref(),
            import.allow_suffix_fallback,
        );

        if let Some((candidate, file_id)) = resolved {
            add_node_metadata(
                &mut context.graph,
                import.import_node,
                "import_scope",
                "local",
            );
            add_node_metadata(
                &mut context.graph,
                import.import_node,
                "import_target",
                import.target.clone(),
            );
            add_node_metadata(
                &mut context.graph,
                import.import_node,
                "resolution",
                "resolved",
            );
            add_node_metadata(
                &mut context.graph,
                import.import_node,
                "resolved_path",
                candidate,
            );
            let mut metadata = BTreeMap::new();
            metadata.insert("relation".to_string(), "local_import_file".to_string());
            metadata.insert("source".to_string(), "syntax".to_string());
            metadata.insert("resolution".to_string(), "local_import_file".to_string());
            // An import the interpreter never runs cannot be a runtime
            // dependency of the file that writes it.
            if context
                .graph
                .nodes
                .get(
                    import
                        .import_node
                        .0
                        .checked_sub(1)
                        .map(|index| index as usize)
                        .unwrap_or(usize::MAX),
                )
                .filter(|node| node.id == import.import_node)
                .and_then(|node| node.metadata.get("type_only"))
                .is_some_and(|value| value == "true")
            {
                metadata.insert("type_only".to_string(), "true".to_string());
            }
            metadata.insert("target".to_string(), import.target);
            add_edge_once_with_metadata(
                context,
                import.import_node,
                file_id,
                EdgeKind::References,
                Confidence::Syntactic,
                metadata,
            );
        } else if let Some((candidate, directory_id)) = import
            .candidates
            .iter()
            .filter_map(|candidate| {
                let directory = candidate.strip_suffix('/')?;
                context
                    .directory_nodes
                    .get(directory)
                    .map(|id| (candidate.clone(), *id))
            })
            .next()
        {
            // A workspace package resolves to a directory rather than a
            // file: Vue's `@vue/runtime-test` is packages/runtime-test, and
            // without this the graph called an import of the project's own
            // package an undeclared outside dependency.
            add_node_metadata(
                &mut context.graph,
                import.import_node,
                "import_scope",
                "workspace",
            );
            add_node_metadata(
                &mut context.graph,
                import.import_node,
                "import_target",
                import.target.clone(),
            );
            add_node_metadata(
                &mut context.graph,
                import.import_node,
                "resolution",
                "resolved",
            );
            add_node_metadata(
                &mut context.graph,
                import.import_node,
                "resolved_path",
                candidate,
            );
            add_edge_once_with_metadata(
                context,
                import.import_node,
                directory_id,
                EdgeKind::References,
                Confidence::Syntactic,
                BTreeMap::from([
                    ("relation".to_string(), "local_import_package".to_string()),
                    ("source".to_string(), "syntax".to_string()),
                    ("resolution".to_string(), "local_import_package".to_string()),
                    ("target".to_string(), import.target),
                ]),
            );
        } else if import.mark_unresolved {
            // `use crate::parse_cli_node_id;` names a function, not a
            // module file: Rust writes both the same way, and only the
            // project can say which it is. An item import is resolved, to
            // the item.
            if let Some(item) = rust_imported_item(context, &import) {
                add_node_metadata(
                    &mut context.graph,
                    import.import_node,
                    "resolution",
                    "resolved",
                );
                add_edge_once_with_metadata(
                    context,
                    import.import_node,
                    item,
                    EdgeKind::References,
                    Confidence::Syntactic,
                    BTreeMap::from([
                        ("relation".to_string(), "local_import_item".to_string()),
                        ("source".to_string(), "syntax".to_string()),
                        ("resolution".to_string(), "local_import_item".to_string()),
                        ("target".to_string(), import.target),
                    ]),
                );
                continue;
            }
            // The project may have said outright that it builds this
            // file: redis lists `src/release.h` in its own `.gitignore`.
            let built = context.build_products.as_ref().is_some_and(|products| {
                import
                    .candidates
                    .iter()
                    .chain(std::iter::once(&import.target))
                    .any(|candidate| products.builds(candidate))
            });
            add_node_metadata(
                &mut context.graph,
                import.import_node,
                "resolution",
                "unresolved",
            );
            if built {
                add_node_metadata(
                    &mut context.graph,
                    import.import_node,
                    "target_is_a_build_product",
                    "true",
                );
            }
            add_node_metadata(
                &mut context.graph,
                import.import_node,
                "candidate_paths",
                import.candidates.join(","),
            );
        }
    }

    // OCaml forbids a cycle between modules, so a file cannot import the
    // one that includes it: stdune's `string.ml` includes `String_split`,
    // and `string_split.ml` opens `String` -- the language's own, since it
    // cannot be the module that includes it. The graph read the pair as a
    // dependency cycle and reported it as a warning.
    let mut imported_files: BTreeMap<(String, String), (usize, bool)> = BTreeMap::new();
    for (index, edge) in context.graph.edges.iter().enumerate() {
        let relation_is_import = edge
            .metadata
            .get("relation")
            .is_some_and(|relation| relation == "local_import_file");
        if !relation_is_import {
            continue;
        }
        let (Some(source), Some(target)) = (
            graph_node(&context.graph, edge.source),
            graph_node(&context.graph, edge.target),
        ) else {
            continue;
        };
        if source.metadata.get("language").map(String::as_str) != Some("ocaml") {
            continue;
        }
        let Some(from) = source.span.as_ref().map(|span| span.path.clone()) else {
            continue;
        };
        let includes = source.label.trim_start().starts_with("include ");
        imported_files.insert((from, target.label.clone()), (index, includes));
    }
    let mut drop_edges: BTreeSet<usize> = BTreeSet::new();
    for ((from, to), (index, includes)) in &imported_files {
        if *includes {
            continue;
        }
        if imported_files
            .get(&(to.clone(), from.clone()))
            .is_some_and(|(_, other_includes)| *other_includes)
        {
            drop_edges.insert(*index);
        }
    }
    if !drop_edges.is_empty() {
        let mut index = 0;
        context.graph.edges.retain(|_| {
            let keep = !drop_edges.contains(&index);
            index += 1;
            keep
        });
        // The context remembers how much of the edge list it has read; a
        // shorter list makes that a promise it cannot keep.
        context.edge_keys.clear();
        context.edge_keys_synced = 0;
    }
}

pub(crate) fn resolve_local_import_candidate(
    file_nodes: &BTreeMap<String, NodeId>,
    files_by_name: &BTreeMap<String, Vec<(String, NodeId)>>,
    candidates: &[String],
    source_path: Option<&str>,
    allow_suffix_fallback: bool,
) -> Option<(String, NodeId)> {
    // No file imports itself. `from flask import Flask` written in a fixture
    // named `flask.py` matched that fixture, and the graph read the match as
    // a dependency cycle.
    let is_the_source = |path: &str| source_path == Some(path);
    for candidate in candidates {
        if let Some(file_id) = file_nodes.get(candidate).copied()
            && !is_the_source(candidate)
        {
            return Some((candidate.clone(), file_id));
        }
        if let Some((path, file_id)) = resolve_directory_import_candidate(file_nodes, candidate)
            && !is_the_source(&path)
        {
            return Some((path, file_id));
        }
    }
    if !allow_suffix_fallback {
        return None;
    }
    // A project's own package is often not at the root: flask's tutorial
    // imports `flaskr.db` from examples/tutorial/flaskr/db.py. One path
    // ending in the candidate is evidence; several are a coincidence, and
    // the import stays unresolved rather than picking one.
    for candidate in candidates {
        // Only a file whose own name ends the candidate can match it, and
        // there are a handful of those against terraform's 19000 files.
        let file_name = candidate.rsplit('/').next().unwrap_or(candidate);
        let Some(same_name) = files_by_name.get(file_name) else {
            continue;
        };
        let mut matches = same_name
            .iter()
            .filter(|(path, _)| path_ends_with_segment(path, candidate) && !is_the_source(path));
        if let Some((path, file_id)) = matches.next()
            && matches.next().is_none()
        {
            return Some((path.clone(), *file_id));
        }
    }
    None
}

/// Files grouped by their own name, so an import that names a path ending
/// in one is looked for among the few files that could match rather than
/// among all of them.
pub(crate) fn files_by_name(
    file_nodes: &BTreeMap<String, NodeId>,
) -> BTreeMap<String, Vec<(String, NodeId)>> {
    let mut index: BTreeMap<String, Vec<(String, NodeId)>> = BTreeMap::new();
    for (path, id) in file_nodes {
        let name = path.rsplit('/').next().unwrap_or(path.as_str());
        index
            .entry(name.to_string())
            .or_default()
            .push((path.clone(), *id));
    }
    index
}

pub(crate) fn resolve_directory_import_candidate(
    file_nodes: &BTreeMap<String, NodeId>,
    candidate: &str,
) -> Option<(String, NodeId)> {
    let prefix = candidate.strip_suffix('/')?;
    let mut package_files = file_nodes.iter().filter(|(path, _)| {
        is_go_file(path)
            && if prefix.is_empty() {
                !path.contains('/')
            } else {
                path.strip_prefix(prefix)
                    .and_then(|rest| rest.strip_prefix('/'))
                    .is_some_and(|rest| !rest.contains('/'))
            }
    });

    package_files
        .find(|(path, _)| !path.ends_with("_test.go"))
        .or_else(|| package_files.next())
        .map(|(path, file_id)| (path.clone(), *file_id))
}

pub(crate) fn is_go_file(path: &str) -> bool {
    path.ends_with(".go")
}

pub(crate) fn resolve_pending_entrypoint_targets(context: &mut IndexContext) {
    let pending_targets = std::mem::take(&mut context.pending_entrypoint_targets);

    for pending in pending_targets {
        for candidate in entrypoint_target_candidates(&pending) {
            if let Some(file_id) = context.file_nodes.get(&candidate.path).copied() {
                add_entrypoint_reference(
                    context,
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
                    context,
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

/// Resolve route handlers that were not found in the declaring file against
/// the global function registry, so routes wired in one module and handled in
/// another (a common split-module layout) still link to their handlers.
/// Link source import facts to manifest package hub nodes wherever the
/// package identity is stable (Rust use paths, npm/dart module specifiers,
/// Python module roots, PHP vendor namespaces, and Go module prefixes), so
/// manifests, lockfiles, and code imports share one canonical package node.
pub(crate) fn link_imports_to_package_hubs(context: &mut IndexContext) {
    let mut hubs: BTreeMap<String, NodeId> = BTreeMap::new();
    for node in &context.graph.nodes {
        if node
            .metadata
            .get("item_kind")
            .is_some_and(|kind| kind == "dependency")
            && let Some(id) = node.metadata.get("package_id")
        {
            hubs.entry(id.clone()).or_insert(node.id);
        }
    }
    if hubs.is_empty() {
        return;
    }
    let go_hubs: Vec<(String, NodeId)> = hubs
        .iter()
        .filter(|(id, _)| id.starts_with("go:"))
        .map(|(id, node)| (id.clone(), *node))
        .collect();

    let mut links: Vec<(NodeId, NodeId, String)> = Vec::new();
    for node in &context.graph.nodes {
        if node
            .metadata
            .get("item_kind")
            .is_none_or(|kind| kind != "import")
            || node
                .metadata
                .get("import_scope")
                .is_some_and(|scope| scope == "local")
            || node.metadata.contains_key("package_id")
        {
            continue;
        }
        let Some(language) = node.metadata.get("language") else {
            continue;
        };
        let mut matched = None;
        for candidate in import_package_id_candidates(language, &node.label) {
            if let Some(hub) = hubs.get(&candidate) {
                matched = Some((*hub, candidate));
                break;
            }
        }
        if matched.is_none()
            && language == "go"
            && let Some(path) = first_quoted_value(&node.label)
        {
            // The import path may extend past the module root; take the
            // longest declared module prefix.
            let mut best: Option<(&str, NodeId)> = None;
            for (id, hub) in &go_hubs {
                let module = &id["go:".len()..];
                // `format!` here builds a string per import and per
                // declared module: terraform has 13680 imports and a
                // hundred modules, and the prefix can be checked without
                // building anything.
                let under_module = path.strip_prefix(module);
                if (path == *module || under_module.is_some_and(|rest| rest.starts_with('/')))
                    && best.is_none_or(|(current, _)| module.len() > current.len())
                {
                    best = Some((module, *hub));
                }
            }
            matched = best.map(|(module, hub)| (hub, format!("go:{module}")));
        }
        if let Some((hub, id)) = matched
            && hub != node.id
        {
            links.push((node.id, hub, id));
        }
    }

    for (import_node, hub, id) in links {
        add_node_metadata(&mut context.graph, import_node, "package_id", &id);
        add_edge_once_with_metadata(
            context,
            import_node,
            hub,
            EdgeKind::DependsOn,
            Confidence::Heuristic,
            BTreeMap::from([
                ("relation".to_string(), "package_import".to_string()),
                ("source".to_string(), "import_resolution".to_string()),
            ]),
        );
    }
}

/// The file an import in this file brings a class in from, by the name it
/// is used under. PHP writes `use App\\Http\\Controllers\\API\\
/// ScrobbleController;` and sometimes `use ... as EnrollTwoFactorController;`,
/// and the resolved import already names the file.
fn imported_class_file(graph: &CodeGraph, file: &str, name: &str) -> Option<String> {
    let file_id = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::File && node.label == file)
        .map(|node| node.id)?;
    graph
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Imports && edge.source == file_id)
        .filter_map(|edge| graph.nodes.iter().find(|node| node.id == edge.target))
        .find_map(|import| {
            let path = import.metadata.get("resolved_path")?;
            let statement = import.label.trim().trim_end_matches(';');
            let used_as = statement
                .rsplit_once(" as ")
                .map(|(_, alias)| alias.trim())
                .unwrap_or_else(|| {
                    statement
                        .rsplit(['\\', '/', ' '])
                        .next()
                        .unwrap_or_default()
                        .trim()
                });
            (used_as == name).then(|| path.clone())
        })
}

/// How closely a declared class has to match the class a route names.
#[derive(Clone, Copy)]
enum OwnerMatch {
    Exact,
    Tail,
    TailIgnoringCase,
}

/// The routes a project's own layout declares. Next.js, Nuxt and
/// SvelteKit name a URL by where a file sits, so a project written that
/// way had no entrypoints at all: no routes, and nothing for `workflow`,
/// `journey` or the coverage finding to start from.
///
/// The manifest is what says the project is written that way -- `app/` is
/// a PHP directory as often as a Next.js one -- so this runs once the scan
/// has read every manifest.
/// The environment reads whose key is a name the project binds to a
/// string: `os.Getenv(envLogFile)` reads `TF_LOG_PATH` wherever terraform
/// declares that constant, and 45 of its 62 computed reads name one. A
/// name nothing binds -- a loop variable, a parameter -- still has no
/// name to give, and stays computed.
pub(crate) fn resolve_pending_computed_environment_reads(context: &mut IndexContext) {
    let pending = std::mem::take(&mut context.pending_computed_environment_reads);
    // The scan files one read per line, and so does this: reading the same
    // variable twice in one function is two lines a reader may want to open.
    let mut sites: BTreeSet<(NodeId, NodeId, u32)> = BTreeSet::new();
    for read in pending {
        let label = computed_environment_key_name(&read.key_expression)
            .and_then(|name| context.string_constants.get(&name).cloned().flatten())
            .filter(|value| environment_key_is_a_name(value))
            .unwrap_or_else(|| COMPUTED_ENVIRONMENT_KEY.to_string());
        let item_id = shared_effect_entity(
            context,
            "environment",
            NodeKind::Environment,
            &label,
            read.span.clone(),
            BTreeMap::from([
                ("parser".to_string(), "tree-sitter".to_string()),
                ("item_kind".to_string(), "environment_read".to_string()),
            ]),
        );
        if !sites.insert((read.source, item_id, read.span.start_line)) {
            continue;
        }
        let mut metadata = read.metadata;
        if label != COMPUTED_ENVIRONMENT_KEY {
            // What settled the name, so a reader can see why the key is
            // not written on the line the span points at.
            metadata.insert("resolution".to_string(), "named_constant".to_string());
        }
        context.graph.add_edge_with_metadata(
            read.source,
            item_id,
            EdgeKind::ReadsEnvironment,
            Confidence::Heuristic,
            metadata,
        );
    }
}

/// The name a computed key is written as: `os.Getenv(envLogFile)` names
/// `envLogFile`. A key built from anything else -- a call, an index, a
/// concatenation -- names nothing to look up.
fn computed_environment_key_name(expression: &str) -> Option<String> {
    let (_, rest) = expression.rsplit_once('(')?;
    let name = rest.split(')').next()?.trim();
    (!name.is_empty()
        && name
            .chars()
            .all(|character| character.is_alphanumeric() || character == '_')
        && !name.starts_with(|character: char| character.is_ascii_digit()))
    .then(|| name.to_string())
}

/// Whether a constant's value could be an environment variable's name.
/// terraform binds `name` to `"%s"` in a test helper, which names no
/// variable however the read is written.
fn environment_key_is_a_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_alphanumeric() || matches!(character, '_' | '.' | '-'))
        && !value.starts_with(|character: char| character.is_ascii_digit())
}

pub(crate) fn resolve_pending_file_routes(context: &mut IndexContext) {
    let pending = std::mem::take(&mut context.pending_file_routes);
    if pending.is_empty() {
        return;
    }
    let declared: BTreeSet<String> = context
        .graph
        .nodes
        .iter()
        .filter_map(|node| node.metadata.get("package_id").cloned())
        .collect();
    for route_file in pending {
        // A file that states its own route needs nothing else to confirm
        // it; a route that follows from the path alone does.
        let stated = route_file.declared.is_some();
        let Some(route) = route_file
            .declared
            .clone()
            .or_else(|| file_based_route(&route_file.label))
        else {
            continue;
        };
        if !stated && !declared.contains(&format!("npm:{}", route.package)) {
            continue;
        }
        // A file the framework runs has no URL: it is an entrypoint of its
        // own, and what it renders is reached through it.
        if route.shape == FileRouteShape::Entry {
            let mut metadata = BTreeMap::new();
            metadata.insert("item_kind".to_string(), "framework_entry".to_string());
            metadata.insert("entrypoint_kind".to_string(), "framework_entry".to_string());
            metadata.insert("source".to_string(), "framework".to_string());
            metadata.insert("framework".to_string(), route.framework.to_string());
            metadata.insert("path".to_string(), route.path.clone());
            metadata.insert("target".to_string(), route_file.label.clone());
            let entrypoint_id = context.graph.add_node_with_metadata(
                NodeKind::Entrypoint,
                format!("{} entry:{}", route.framework, route_file.label),
                Some(SourceSpan {
                    path: route_file.label.clone(),
                    start_line: 1,
                    start_column: 1,
                    end_line: 1,
                    end_column: 1,
                }),
                metadata,
            );
            add_edge_once(
                context,
                route_file.file,
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
                route_file.file,
                "entrypoint_file",
                "framework_entry_file",
                Confidence::Exact,
                None,
            );
            continue;
        }
        // A handler module names each method it serves with a function of
        // that name; a page is served on GET.
        // A Razor Page's handlers live in the `.cshtml.cs` beside it,
        // named for the method they serve: `OnGet`, `OnPostAsync`.
        let handler_path = match route.shape {
            FileRouteShape::PageModel => format!("{}.cs", route_file.label),
            _ => route_file.label.clone(),
        };
        let handlers: Vec<(String, NodeId)> = match route.shape {
            FileRouteShape::PageModel => context
                .graph
                .nodes
                .iter()
                .filter(|node| {
                    node.kind == NodeKind::Function
                        && node
                            .span
                            .as_ref()
                            .is_some_and(|span| span.path == handler_path)
                })
                .filter_map(|node| {
                    razor_handler_method(&node.label).map(|method| (method.to_string(), node.id))
                })
                .collect(),
            FileRouteShape::Handler => context
                .graph
                .nodes
                .iter()
                .filter(|node| {
                    node.kind == NodeKind::Function
                        && node
                            .span
                            .as_ref()
                            .is_some_and(|span| span.path == route_file.label)
                })
                .filter_map(|node| {
                    file_route_method(&node.label).map(|method| (method.to_string(), node.id))
                })
                .collect(),
            _ => Vec::new(),
        };
        // Two handlers can serve one method -- a Razor Page writes
        // `OnPost` and `OnPostUpdate` -- and that is one route the page
        // serves, reached by both, not two routes with the same name.
        let handlers: Vec<(String, NodeId)> = if route.shape == FileRouteShape::PageModel {
            let mut seen: BTreeSet<String> = BTreeSet::new();
            handlers
                .into_iter()
                .filter(|(method, _)| seen.insert(method.clone()))
                .collect()
        } else {
            handlers
        };
        let methods: Vec<(String, Option<NodeId>)> = if handlers.is_empty() {
            let method = match route.shape {
                FileRouteShape::AnyMethod => "ANY",
                _ => "GET",
            };
            vec![(method.to_string(), None)]
        } else {
            handlers
                .into_iter()
                .map(|(method, id)| (method, Some(id)))
                .collect()
        };
        for (method, handler) in methods {
            let mut metadata = BTreeMap::new();
            metadata.insert("item_kind".to_string(), "framework_route".to_string());
            metadata.insert("entrypoint_kind".to_string(), "route".to_string());
            metadata.insert("source".to_string(), "framework".to_string());
            metadata.insert("framework".to_string(), route.framework.to_string());
            metadata.insert("route_form".to_string(), "file_path".to_string());
            metadata.insert("method".to_string(), method.clone());
            metadata.insert("path".to_string(), route.path.clone());
            metadata.insert("target".to_string(), route_file.label.clone());
            let entrypoint_id = context.graph.add_node_with_metadata(
                NodeKind::Entrypoint,
                format!("route {method} {}", route.path),
                Some(SourceSpan {
                    path: route_file.label.clone(),
                    start_line: 1,
                    start_column: 1,
                    end_line: 1,
                    end_column: 1,
                }),
                metadata,
            );
            add_edge_once(
                context,
                route_file.file,
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
                route_file.file,
                "entrypoint_file",
                "framework_route_file",
                Confidence::Exact,
                None,
            );
            if let Some(handler) = handler {
                add_entrypoint_reference(
                    context,
                    entrypoint_id,
                    handler,
                    "entrypoint_function",
                    "framework_route_handler",
                    Confidence::Exact,
                    Some(&method),
                );
            }
        }
    }
}

/// The classes a name inherits from, nearest first. `class
/// AdditionalFooterTextsController < Admin::SettingsController` states
/// where its `show` comes from, and a route naming it reaches nothing
/// without following the chain.
/// Whether a bare call in this language can only mean a method the caller
/// already has: its own, one it inherits, or one its file declares.
///
/// Only a language whose call label keeps the receiver can be asked: Java,
/// Kotlin, Swift and PHP write `pet.getName()` and the label keeps
/// `getName` alone, so a label without a receiver does not mean the source
/// had none. C# and Dart keep it, but lose it again on a fluent chain that
/// starts `.AddRetry(` on its own line, and answer a bare name through
/// `using static` besides -- so both would refuse calls that are real.
/// Python and Ruby require the receiver outright and are answered earlier,
/// and Go, Rust and the rest reach package-level functions by a bare name.
fn bare_call_stays_with_its_object(language: &str) -> bool {
    matches!(language, "scala" | "java")
}

fn ancestor_type_names(graph: &CodeGraph, types: &TypeNodesByName, owner: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    // A contract composes rather than descends: solidity writes `is
    // AbstractSigner, EIP712, Paymaster` and reaches
    // `EIP712._hashTypedDataV4` through the second of them, so the walk
    // follows every base a declaration names rather than the first.
    let mut queue: VecDeque<String> = VecDeque::from([owner.to_string()]);
    // A hierarchy this deep is already unusual, and a cycle cannot be one.
    while names.len() < 8 {
        let Some(current) = queue.pop_front() else {
            break;
        };
        let Some(declared) = type_node_named(graph, types, &current)
            .and_then(|node| node.metadata.get("extends").cloned())
        else {
            continue;
        };
        for parent in declared
            .split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty())
        {
            if parent == owner || parent == current || names.iter().any(|seen| seen == parent) {
                continue;
            }
            names.push(parent.to_string());
            queue.push_back(parent.to_string());
        }
    }
    names
}

/// The class a name states, matched as written and then by what it ends
/// with: a Rails route names `Admin::SettingsController` and the file
/// declares exactly that, while a PHP route names `AlbumController` for a
/// class the file writes under a namespace.
/// Every name a type answers to, and the node that answers it: the label
/// itself, and the tail of a qualified label. A module that states what it
/// re-exports answers the same way, after the types.
///
/// Built once and then asked. Reading it off the node list per question is
/// three passes over every node in the graph, and the questions are asked
/// per call: terraform has 132813 nodes and 102779 calls, and its scan took
/// 38 seconds for that reason.
pub(crate) type TypeNodesByName = BTreeMap<String, NodeId>;

pub(crate) fn type_nodes_by_name(graph: &CodeGraph) -> TypeNodesByName {
    let mut named: TypeNodesByName = BTreeMap::new();
    let mut tails: TypeNodesByName = BTreeMap::new();
    let mut modules: TypeNodesByName = BTreeMap::new();
    for node in &graph.nodes {
        match node.kind {
            NodeKind::Type => {
                named.entry(node.label.clone()).or_insert(node.id);
                let tail = node
                    .label
                    .rsplit(['\\', ':'])
                    .next()
                    .unwrap_or(&node.label)
                    .to_string();
                tails.entry(tail).or_insert(node.id);
            }
            NodeKind::Module if node.metadata.contains_key("extends") => {
                modules.entry(node.label.clone()).or_insert(node.id);
                let tail = node
                    .label
                    .rsplit(['\\', ':'])
                    .next()
                    .unwrap_or(&node.label)
                    .to_string();
                modules.entry(tail).or_insert(node.id);
            }
            _ => {}
        }
    }
    // The order the search used: the label, then a tail, then a module.
    for (name, id) in tails.into_iter().chain(modules) {
        named.entry(name).or_insert(id);
    }
    named
}

fn type_node_named<'graph>(
    graph: &'graph CodeGraph,
    types: &TypeNodesByName,
    name: &str,
) -> Option<&'graph Node> {
    let tail = name.rsplit(['\\', ':']).next().unwrap_or(name);
    types
        .get(name)
        .or_else(|| types.get(tail))
        .and_then(|id| graph_node(graph, *id))
}

pub(crate) fn resolve_pending_route_handlers(context: &mut IndexContext) {
    let type_names = type_nodes_by_name(&context.graph);
    let pending = std::mem::take(&mut context.pending_route_handlers);
    for reference in pending {
        // A route's handler is written in the language the route is
        // declared in: the codegraph server's `GET /api/scan` names a Rust
        // function, and the `scan` in its own JavaScript bundle is not a
        // candidate for it.
        let language = graph_node(&context.graph, reference.entrypoint)
            .and_then(|node| node.metadata.get("language"))
            .cloned();
        let mut targets = resolve_function_targets(&context.function_symbols, &reference.handler)
            .into_iter()
            .filter(|target| {
                language.as_deref().is_none_or(|language| {
                    graph_node(&context.graph, *target)
                        .and_then(|node| node.metadata.get("language"))
                        .is_some_and(|declared| languages_share_symbols(declared, language))
                })
            })
            .collect::<Vec<_>>();
        // The route says which class serves it, and that settles which
        // method it means: koel writes 139 controllers whose method is
        // `__invoke`, and the name alone chooses none of them.
        if let Some(owner) = reference.owner.as_deref() {
            // A Ruby class states the modules it sits in --
            // `ActivityPub::LikesController` -- and a Rails route names
            // the controller path, which the inflector turns into that
            // name. Comparing what the two end with settles the method
            // without knowing the project's namespaces.
            let owner_is = |target: &NodeId, owner: &str, match_kind: OwnerMatch| {
                let owner_tail = owner.rsplit("::").next().unwrap_or(owner);
                graph_node(&context.graph, *target)
                    .and_then(|node| node.metadata.get("owner_type"))
                    .is_some_and(|declared| {
                        let declared_tail = declared.rsplit("::").next().unwrap_or(declared);
                        match match_kind {
                            OwnerMatch::Exact => declared == owner,
                            OwnerMatch::Tail => declared_tail == owner_tail,
                            // An acronym rule only changes which letters a
                            // name capitalises: mastodon states
                            // `inflect.acronym 'OEmbed'`, so `oembed#show`
                            // is `Api::OEmbedController#show` and
                            // `oauth_metadata#show` under `module:
                            // :well_known` is
                            // `WellKnown::OAuthMetadataController#show`.
                            // Camelising a route's controller cannot know
                            // that, and the letters are the same either way.
                            OwnerMatch::TailIgnoringCase => {
                                declared_tail.eq_ignore_ascii_case(owner_tail)
                            }
                        }
                    })
            };
            // The name as written wins: mastodon declares both
            // `PrivacyController` and `Settings::PrivacyController`, and
            // a route naming the first means the first.
            let mut owned: Vec<NodeId> = Vec::new();
            for match_kind in [
                OwnerMatch::Exact,
                OwnerMatch::Tail,
                OwnerMatch::TailIgnoringCase,
            ] {
                if !owned.is_empty() {
                    break;
                }
                owned = targets
                    .iter()
                    .copied()
                    .filter(|target| owner_is(target, owner, match_kind))
                    .collect();
            }
            // A controller that declares no `show` is served by the one
            // its parent declares: mastodon writes eleven settings pages
            // whose actions are all `Admin::SettingsController`'s, and a
            // route that reaches nothing is where a flow stops.
            if owned.is_empty() {
                for ancestor in ancestor_type_names(&context.graph, &type_names, owner) {
                    owned = targets
                        .iter()
                        .copied()
                        .filter(|target| owner_is(target, &ancestor, OwnerMatch::Exact))
                        .collect();
                    if !owned.is_empty() {
                        break;
                    }
                }
            }
            if !owned.is_empty() {
                targets = owned;
            }
            // Two classes can share a name -- koel has an API and a
            // Subsonic `ScrobbleController` -- and one of them is what the
            // route file imported. An alias is the same statement:
            // `EnrollController as EnrollTwoFactorController` names a file.
            let declaring_file = graph_node(&context.graph, reference.entrypoint)
                .and_then(|node| node.span.as_ref().map(|span| span.path.clone()));
            if let Some(file) = declaring_file
                && let Some(path) = imported_class_file(&context.graph, &file, owner)
            {
                let declared: Vec<NodeId> = targets
                    .iter()
                    .copied()
                    .filter(|target| {
                        graph_node(&context.graph, *target)
                            .and_then(|node| node.span.as_ref())
                            .is_some_and(|span| span.path == path)
                    })
                    .collect();
                if !declared.is_empty() {
                    targets = declared;
                }
            }
        }
        // A decorator sits directly above the function it registers, so the
        // handler is in the route's own file — which `detectors` already
        // tried. Reaching here means it was not, and linking to every
        // same-named function invented the links wholesale: one `@app.route`
        // in a flask docstring claimed about 140 different `index`
        // functions as its handler. Take the single candidate when the name
        // leaves no choice, and otherwise leave the handler unresolved,
        // which `unresolved_framework_route_handler` already reports.
        let [handler_id] = targets[..] else {
            // Nothing in the project answers. When the route wrote the
            // handler under a name the file imports -- `views.index`,
            // where `views` is `django.contrib.sitemaps.views` -- the
            // handler belongs to that package and the project was never
            // going to declare it.
            let imported = graph_node(&context.graph, reference.entrypoint)
                .and_then(|node| {
                    let qualifier = node.metadata.get("handler_qualifier")?.clone();
                    let file = node.span.as_ref().map(|span| span.path.clone())?;
                    Some((qualifier, file))
                })
                .is_some_and(|(qualifier, file)| {
                    context
                        .file_import_qualifiers
                        .get(&file)
                        .is_some_and(|names| names.contains_key(&qualifier))
                        || context
                            .file_imported_names
                            .get(&file)
                            .is_some_and(|names| names.contains_key(&qualifier))
                });
            if imported {
                add_node_metadata(
                    &mut context.graph,
                    reference.entrypoint,
                    "handler_scope",
                    "external",
                );
            }
            continue;
        };
        add_entrypoint_reference(
            context,
            reference.entrypoint,
            handler_id,
            "entrypoint_function",
            "framework_route_handler",
            Confidence::Heuristic,
            Some(&reference.handler),
        );
    }
}

pub(crate) fn resolve_pending_compose_config_targets(context: &mut IndexContext) {
    let pending_targets = std::mem::take(&mut context.pending_compose_config_targets);

    for pending in pending_targets {
        let Some(path) = normalize_manifest_relative_path(&pending.manifest_label, &pending.target)
        else {
            continue;
        };
        let Some(file_id) = context.file_nodes.get(&path).copied() else {
            continue;
        };
        let mut metadata = BTreeMap::new();
        metadata.insert("relation".to_string(), "config_file".to_string());
        metadata.insert(
            "resolution".to_string(),
            "compose_env_file_path".to_string(),
        );
        metadata.insert("source".to_string(), "compose".to_string());
        add_edge_once_with_metadata(
            context,
            pending.config,
            file_id,
            EdgeKind::References,
            Confidence::Exact,
            metadata,
        );
    }
}

pub(crate) fn resolve_pending_compose_volume_targets(context: &mut IndexContext) {
    let pending_targets = std::mem::take(&mut context.pending_compose_volume_targets);

    for pending in pending_targets {
        let Some(path) = compose_volume_local_source_path(&pending.manifest_label, &pending.target)
        else {
            continue;
        };
        let target_id = context
            .file_nodes
            .get(&path)
            .copied()
            .or_else(|| context.directory_nodes.get(&path).copied());
        let Some(target_id) = target_id else {
            continue;
        };
        let mut metadata = BTreeMap::new();
        metadata.insert("relation".to_string(), "volume_source".to_string());
        metadata.insert(
            "resolution".to_string(),
            "compose_volume_source_path".to_string(),
        );
        metadata.insert("source".to_string(), "compose".to_string());
        add_edge_once_with_metadata(
            context,
            pending.volume,
            target_id,
            EdgeKind::References,
            Confidence::Exact,
            metadata,
        );
    }
}

pub(crate) fn resolve_pending_kubernetes_config_refs(context: &mut IndexContext) {
    let pending_refs = std::mem::take(&mut context.pending_kubernetes_config_refs);

    for pending in pending_refs {
        let key = KubernetesConfigKey {
            namespace: pending.namespace,
            config_kind: pending.config_kind,
            name: pending.name,
        };
        let Some(config_id) = context.kubernetes_configs.get(&key).copied() else {
            continue;
        };
        let mut metadata = BTreeMap::new();
        metadata.insert("relation".to_string(), "config_definition".to_string());
        metadata.insert(
            "resolution".to_string(),
            "kubernetes_config_ref".to_string(),
        );
        metadata.insert("source".to_string(), "kubernetes".to_string());
        add_edge_once_with_metadata(
            context,
            pending.config_ref,
            config_id,
            EdgeKind::References,
            Confidence::Exact,
            metadata,
        );
    }
}

pub(crate) fn resolve_pending_kubernetes_service_refs(context: &mut IndexContext) {
    let pending_refs = std::mem::take(&mut context.pending_kubernetes_service_refs);

    for pending in pending_refs {
        let key = KubernetesServiceKey {
            namespace: pending.namespace,
            name: pending.name,
        };
        let Some(service_id) = context.kubernetes_services.get(&key).copied() else {
            continue;
        };
        let mut metadata = BTreeMap::new();
        metadata.insert("relation".to_string(), "service_definition".to_string());
        metadata.insert(
            "resolution".to_string(),
            "kubernetes_service_ref".to_string(),
        );
        metadata.insert("source".to_string(), "kubernetes".to_string());
        add_edge_once_with_metadata(
            context,
            pending.service_ref,
            service_id,
            EdgeKind::References,
            Confidence::Exact,
            metadata,
        );
    }
}

pub(crate) fn resolve_pending_github_actions_local_actions(context: &mut IndexContext) {
    let pending_actions = std::mem::take(&mut context.pending_github_actions_local_actions);

    for pending in pending_actions {
        let Some(target_id) = github_actions_local_action_target(context, &pending.target) else {
            continue;
        };
        let mut metadata = BTreeMap::new();
        metadata.insert(
            "relation".to_string(),
            "github_actions_local_action".to_string(),
        );
        metadata.insert(
            "resolution".to_string(),
            "github_actions_local_action_path".to_string(),
        );
        metadata.insert("source".to_string(), "github-actions".to_string());
        add_edge_once_with_metadata(
            context,
            pending.action,
            target_id,
            EdgeKind::References,
            Confidence::Exact,
            metadata,
        );
    }
}

pub(crate) fn resolve_pending_document_path_refs(context: &mut IndexContext) {
    let pending_refs = std::mem::take(&mut context.pending_document_path_refs);

    for pending in pending_refs {
        let target = pending.candidates.iter().find_map(|candidate| {
            context
                .file_nodes
                .get(candidate)
                .copied()
                .or_else(|| context.directory_nodes.get(candidate).copied())
                .map(|id| (candidate.clone(), id))
        });
        let Some((resolved_path, target_id)) = target else {
            continue;
        };

        let mut metadata = BTreeMap::new();
        metadata.insert("relation".to_string(), pending.relation.to_string());
        metadata.insert("resolution".to_string(), "document_path".to_string());
        metadata.insert("source".to_string(), "markdown".to_string());
        metadata.insert("target".to_string(), pending.target);
        metadata.insert("resolved_path".to_string(), resolved_path);
        metadata.insert("line".to_string(), pending.line.to_string());
        if let Some(text) = pending.text {
            metadata.insert("text".to_string(), text);
        }
        if let Some(line_ref) = pending.line_ref {
            metadata.insert("line_ref".to_string(), line_ref);
        }
        add_edge_once_with_metadata(
            context,
            pending.source,
            target_id,
            EdgeKind::References,
            Confidence::Exact,
            metadata,
        );
    }
}

/// Count incoming documentation references per node so heavily cited code
/// and documents surface their backlink counts.
pub(crate) fn annotate_document_backlinks(context: &mut IndexContext) {
    let mut backlinks: BTreeMap<NodeId, usize> = BTreeMap::new();
    for edge in &context.graph.edges {
        if edge
            .metadata
            .get("relation")
            .is_some_and(|relation| relation.starts_with("markdown_"))
        {
            *backlinks.entry(edge.target).or_insert(0) += 1;
        }
    }
    for (node_id, count) in backlinks {
        add_node_metadata(
            &mut context.graph,
            node_id,
            "doc_backlinks",
            count.to_string(),
        );
    }
}

pub(crate) fn resolve_pending_document_symbol_refs(context: &mut IndexContext) {
    let pending_refs = std::mem::take(&mut context.pending_document_symbol_refs);

    for pending in pending_refs {
        let targets = resolve_function_targets(&context.function_symbols, &pending.symbol);
        // A prose mention carries no scope: nothing in `render` written in
        // a README says which of vue core's 699 functions of that name it
        // means, and linking to all of them said the document referenced
        // every one. Across the corpora that manufactured 76000 edges out
        // of 3600 ambiguous mentions. Link only when the name leaves no
        // choice — the same rule type references already follow.
        let [target] = targets[..] else {
            continue;
        };
        // Documentation describes what a project offers, not how it tests
        // itself: 11 of nlohmann/json's 14 prose mentions named a helper
        // inside its test suite. A document written among the tests may
        // mean one, and is left alone.
        let document_path = graph_node(&context.graph, pending.source)
            .and_then(|node| node.span.as_ref().map(|span| span.path.clone()))
            .or_else(|| graph_node(&context.graph, pending.source).map(|node| node.label.clone()));
        let target_is_test = graph_node(&context.graph, target)
            .and_then(|node| node.span.as_ref())
            .is_some_and(|span| is_test_like_source_path(&span.path));
        if target_is_test
            && !document_path
                .as_deref()
                .is_some_and(is_test_like_source_path)
        {
            continue;
        }

        let mut metadata = BTreeMap::new();
        metadata.insert("relation".to_string(), pending.relation.to_string());
        metadata.insert("resolution".to_string(), "document_symbol".to_string());
        metadata.insert("source".to_string(), "markdown".to_string());
        metadata.insert("symbol".to_string(), pending.symbol);
        metadata.insert("line".to_string(), pending.line.to_string());

        add_edge_once_with_metadata(
            context,
            pending.source,
            target,
            EdgeKind::References,
            Confidence::Heuristic,
            metadata,
        );
    }
}

pub(crate) fn resolve_pending_sql_foreign_keys(context: &mut IndexContext) {
    let pending_refs = std::mem::take(&mut context.pending_sql_foreign_keys);

    for pending in pending_refs {
        let target_table_key = sql_identifier_key(&pending.target_table);
        let target_id = pending
            .target_column
            .as_deref()
            .and_then(|column| {
                context
                    .sql_columns
                    .get(&sql_column_key(&target_table_key, column))
                    .copied()
            })
            .or_else(|| context.sql_tables.get(&target_table_key).copied());
        let Some(target_id) = target_id else {
            continue;
        };

        let mut metadata = BTreeMap::new();
        metadata.insert("relation".to_string(), "sql_foreign_key".to_string());
        metadata.insert("source".to_string(), "sql".to_string());
        metadata.insert("source_table".to_string(), pending.source_table);
        metadata.insert("target_table".to_string(), pending.target_table);
        metadata.insert("line".to_string(), pending.line.to_string());
        if let Some(source_column) = pending.source_column {
            metadata.insert("source_column".to_string(), source_column);
        }
        if let Some(target_column) = pending.target_column {
            metadata.insert("target_column".to_string(), target_column);
        }
        add_edge_once_with_metadata(
            context,
            pending.source,
            target_id,
            EdgeKind::References,
            Confidence::Exact,
            metadata,
        );
    }
}

pub(crate) fn resolve_pending_sql_query_table_refs(context: &mut IndexContext) {
    let pending_refs = std::mem::take(&mut context.pending_sql_query_table_refs);
    let mut unresolved_tables: BTreeMap<NodeId, BTreeSet<String>> = BTreeMap::new();

    for pending in pending_refs {
        let table_key = sql_identifier_key(&pending.table);
        let Some(table_id) = context.sql_tables.get(&table_key).copied() else {
            unresolved_tables
                .entry(pending.query)
                .or_default()
                .insert(pending.table);
            continue;
        };

        add_edge_once_with_metadata(
            context,
            pending.query,
            table_id,
            EdgeKind::References,
            Confidence::Heuristic,
            BTreeMap::from([
                (
                    "relation".to_string(),
                    "app_sql_table_reference".to_string(),
                ),
                ("source".to_string(), "source_sql_literal".to_string()),
                ("operation".to_string(), pending.operation),
                ("role".to_string(), pending.role),
                ("table".to_string(), pending.table),
                ("line".to_string(), pending.line.to_string()),
            ]),
        );
    }

    for (query_id, tables) in unresolved_tables {
        let has_resolved_table_ref = context.graph.edges.iter().any(|edge| {
            edge.source == query_id
                && edge.kind == EdgeKind::References
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|relation| relation == "app_sql_table_reference")
        });
        add_node_metadata(
            &mut context.graph,
            query_id,
            "resolution",
            if has_resolved_table_ref {
                "partial"
            } else {
                "unresolved"
            },
        );
        add_node_metadata(
            &mut context.graph,
            query_id,
            "unresolved_tables",
            tables.into_iter().collect::<Vec<_>>().join(","),
        );
    }
}

/// Link joined tables with `sql_join` edges once both sides are indexed.
pub(crate) fn resolve_pending_sql_joins(context: &mut IndexContext) {
    let pending = std::mem::take(&mut context.pending_sql_joins);
    for join in pending {
        let left_key = sql_identifier_key(&join.left);
        let right_key = sql_identifier_key(&join.right);
        let (Some(left_id), Some(right_id)) = (
            context.sql_tables.get(&left_key).copied(),
            context.sql_tables.get(&right_key).copied(),
        ) else {
            continue;
        };
        let mut metadata = BTreeMap::from([
            ("relation".to_string(), "sql_join".to_string()),
            ("source".to_string(), "sql".to_string()),
            ("line".to_string(), join.line.to_string()),
        ]);
        if let Some(condition) = join.condition {
            metadata.insert("condition".to_string(), condition);
        }
        add_edge_once_with_metadata(
            context,
            left_id,
            right_id,
            EdgeKind::References,
            Confidence::Heuristic,
            metadata,
        );
    }
}

/// Link ALTER/DROP TABLE statements to their tables; unknown targets are
/// recorded on the file node for schema-consistency insights.
pub(crate) fn resolve_pending_sql_alter_refs(context: &mut IndexContext) {
    let pending = std::mem::take(&mut context.pending_sql_alter_refs);
    let mut unresolved: BTreeMap<NodeId, BTreeSet<String>> = BTreeMap::new();
    for reference in pending {
        let table_key = sql_identifier_key(&reference.table);
        match context.sql_tables.get(&table_key).copied() {
            Some(table_id) => {
                add_edge_once_with_metadata(
                    context,
                    reference.file,
                    table_id,
                    EdgeKind::References,
                    Confidence::Syntactic,
                    BTreeMap::from([
                        ("relation".to_string(), "sql_schema_change".to_string()),
                        ("operation".to_string(), reference.operation.to_string()),
                        ("source".to_string(), "sql".to_string()),
                        ("line".to_string(), reference.line.to_string()),
                    ]),
                );
            }
            None => {
                unresolved
                    .entry(reference.file)
                    .or_default()
                    .insert(format!("{}:{}", reference.operation, reference.table));
            }
        }
    }
    for (file_id, tables) in unresolved {
        add_node_metadata(
            &mut context.graph,
            file_id,
            "unresolved_sql_alter_tables",
            tables.into_iter().collect::<Vec<_>>().join(","),
        );
    }
}

/// Chain migration files in sequence order per directory and flag duplicate
/// sequence numbers for insight checks.
pub(crate) fn resolve_sql_migration_order(context: &mut IndexContext) {
    let mut migrations = std::mem::take(&mut context.sql_migrations);
    if migrations.is_empty() {
        return;
    }
    migrations.sort_by(|a, b| {
        a.dir
            .cmp(&b.dir)
            .then(a.sequence.cmp(&b.sequence))
            .then(a.label.cmp(&b.label))
    });
    for migration in &migrations {
        context
            .sql_migration_dirs
            .entry(migration.dir.clone())
            .or_default()
            .push(migration.file);
    }
    for migration in &migrations {
        add_node_metadata(
            &mut context.graph,
            migration.file,
            "migration_sequence",
            &migration.sequence_text,
        );
    }
    for pair in migrations.windows(2) {
        if pair[0].dir != pair[1].dir {
            continue;
        }
        if pair[0].sequence == pair[1].sequence {
            add_node_metadata(
                &mut context.graph,
                pair[0].file,
                "duplicate_migration_sequence",
                &pair[1].label,
            );
            add_node_metadata(
                &mut context.graph,
                pair[1].file,
                "duplicate_migration_sequence",
                &pair[0].label,
            );
            continue;
        }
        add_edge_once_with_metadata(
            context,
            pair[0].file,
            pair[1].file,
            EdgeKind::References,
            Confidence::Exact,
            BTreeMap::from([
                ("relation".to_string(), "migration_order".to_string()),
                ("source".to_string(), "sql".to_string()),
                ("from_sequence".to_string(), pair[0].sequence_text.clone()),
                ("to_sequence".to_string(), pair[1].sequence_text.clone()),
            ]),
        );
    }
}

/// Link ORM table mappings to their indexed tables.
pub(crate) fn resolve_pending_orm_table_refs(context: &mut IndexContext) {
    let pending = std::mem::take(&mut context.pending_orm_table_refs);
    for reference in pending {
        let table_key = sql_identifier_key(&reference.table);
        let Some(table_id) = context.sql_tables.get(&table_key).copied() else {
            continue;
        };
        add_edge_once_with_metadata(
            context,
            reference.file,
            table_id,
            EdgeKind::References,
            Confidence::Heuristic,
            BTreeMap::from([
                ("relation".to_string(), "orm_table_mapping".to_string()),
                ("source".to_string(), "orm_metadata".to_string()),
                ("pattern".to_string(), reference.pattern.to_string()),
                ("table".to_string(), reference.table),
                ("line".to_string(), reference.line.to_string()),
            ]),
        );
    }
}

/// Link migration runner/config references to the migration files inside the
/// referenced directory (matched exactly or as a path suffix), capped per
/// reference so one runner line does not fan out unboundedly.
pub(crate) fn resolve_pending_migration_dir_refs(context: &mut IndexContext) {
    const MIGRATION_LINK_LIMIT: usize = 20;
    let pending = std::mem::take(&mut context.pending_migration_dir_refs);
    for reference in pending {
        let mut targets: Vec<NodeId> = Vec::new();
        for (dir, files) in &context.sql_migration_dirs {
            let matches = dir == &reference.dir
                || dir.ends_with(&format!("/{}", reference.dir))
                || reference.dir.ends_with(&format!("/{dir}"));
            if matches {
                targets.extend(files.iter().copied());
            }
        }
        targets.sort();
        targets.dedup();
        for target in targets.into_iter().take(MIGRATION_LINK_LIMIT) {
            add_edge_once_with_metadata(
                context,
                reference.file,
                target,
                EdgeKind::References,
                Confidence::Heuristic,
                BTreeMap::from([
                    ("relation".to_string(), "runs_migrations".to_string()),
                    ("source".to_string(), reference.source_kind.to_string()),
                    ("directory".to_string(), reference.dir.clone()),
                    ("line".to_string(), reference.line.to_string()),
                ]),
            );
        }
    }
}

/// Link MCP server commands/args to the scanned files they run.
pub(crate) fn resolve_pending_mcp_local_refs(context: &mut IndexContext) {
    let pending = std::mem::take(&mut context.pending_mcp_local_refs);
    for reference in pending {
        let Some(target) = context.file_nodes.get(&reference.candidate).copied() else {
            continue;
        };
        add_edge_once_with_metadata(
            context,
            reference.server,
            target,
            EdgeKind::References,
            Confidence::Heuristic,
            BTreeMap::from([
                ("relation".to_string(), "mcp_server_source".to_string()),
                ("source".to_string(), "mcp_config".to_string()),
            ]),
        );
    }
}

pub(crate) fn github_actions_local_action_target(
    context: &IndexContext,
    target: &str,
) -> Option<NodeId> {
    let candidates = [
        target.to_string(),
        format!("{target}/action.yml"),
        format!("{target}/action.yaml"),
        format!("{target}/Dockerfile"),
    ];
    for candidate in candidates {
        if let Some(id) = context.directory_nodes.get(&candidate).copied() {
            return Some(id);
        }
        if let Some(id) = context.file_nodes.get(&candidate).copied() {
            return Some(id);
        }
    }
    None
}

pub(crate) fn entrypoint_target_candidates(
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
        // A `dune` file states the program a directory builds, and the
        // module it names sits beside it: `(executable (name main))` in
        // `bin/dune` is `bin/main.ml`.
        "cabal" => manifest_path_candidate(
            pending,
            &pending.target,
            None,
            Confidence::Exact,
            Confidence::Syntactic,
            "manifest_path",
        )
        .into_iter()
        .collect(),
        "dune" => manifest_path_candidate(
            pending,
            &pending.target,
            None,
            Confidence::Exact,
            Confidence::Syntactic,
            "manifest_path",
        )
        .into_iter()
        .collect(),
        "dart" | "flutter" => manifest_path_candidate(
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
        "make" => command_path_candidate(pending)
            .map(|path| EntrypointTargetCandidate {
                path,
                symbol: None,
                file_confidence: Confidence::Heuristic,
                function_confidence: Confidence::Heuristic,
                resolution: "make_command_path",
            })
            .into_iter()
            .collect(),
        // `//go:generate go run ../../testdata/gqlgen.go` names the
        // program that writes the code beside it.
        "go-generate" => command_path_candidate(pending)
            .map(|path| EntrypointTargetCandidate {
                path,
                symbol: None,
                file_confidence: Confidence::Heuristic,
                function_confidence: Confidence::Heuristic,
                resolution: "go_generate_command_path",
            })
            .into_iter()
            .collect(),
        // A Dockerfile's command runs inside the image, where the paths are
        // the ones `COPY` put there from the build context -- the
        // repository, not the directory the Dockerfile sits in. Mastodon
        // keeps `streaming/Dockerfile` and runs `node ./streaming/index.js`
        // from `WORKDIR /opt/mastodon`, and reading that beside the
        // Dockerfile looked for `streaming/streaming/index.js`.
        "docker" => {
            let mut candidates: Vec<EntrypointTargetCandidate> = command_path_candidate(pending)
                .into_iter()
                .chain(
                    normalized_command_path_candidate("", &pending.target)
                        .map(|candidate| candidate.path),
                )
                .map(|path| EntrypointTargetCandidate {
                    path,
                    symbol: None,
                    file_confidence: Confidence::Heuristic,
                    function_confidence: Confidence::Heuristic,
                    resolution: "docker_command_path",
                })
                .collect();
            candidates.dedup_by(|left, right| left.path == right.path);
            candidates
        }
        "compose" => command_path_candidate(pending)
            .map(|path| EntrypointTargetCandidate {
                path,
                symbol: None,
                file_confidence: Confidence::Heuristic,
                function_confidence: Confidence::Heuristic,
                resolution: "compose_command_path",
            })
            .into_iter()
            .collect(),
        "compose-dockerfile" => manifest_path_candidate(
            pending,
            &pending.target,
            None,
            Confidence::Exact,
            Confidence::Exact,
            "compose_dockerfile",
        )
        .into_iter()
        .collect(),
        "github-actions" => command_path_in_directory(&pending.target, pending.base_dir.as_deref())
            .map(|candidate| EntrypointTargetCandidate {
                path: candidate.path,
                symbol: None,
                file_confidence: Confidence::Heuristic,
                function_confidence: Confidence::Heuristic,
                resolution: "github_actions_run_command_path",
            })
            .into_iter()
            .collect(),
        "gitlab-ci" => command_path_in_directory(&pending.target, pending.base_dir.as_deref())
            .map(|candidate| EntrypointTargetCandidate {
                path: candidate.path,
                symbol: None,
                file_confidence: Confidence::Heuristic,
                function_confidence: Confidence::Heuristic,
                resolution: "gitlab_ci_script_command_path",
            })
            .into_iter()
            .collect(),
        _ => Vec::new(),
    }
}

pub(crate) fn manifest_path_candidate(
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

pub(crate) fn python_entrypoint_candidates(
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

pub(crate) fn command_path_candidate(pending: &PendingEntrypointTarget) -> Option<String> {
    normalized_command_path_candidate(&pending.manifest_label, &pending.target)
        .map(|candidate| candidate.path)
}

pub(crate) fn normalized_command_path_candidate(
    manifest_label: &str,
    command: &str,
) -> Option<CommandPath> {
    command_paths(command).into_iter().find_map(|candidate| {
        normalize_manifest_relative_path(manifest_label, &candidate.path).map(|path| CommandPath {
            path,
            written: candidate.written,
        })
    })
}

/// A path a command names, and whether the command writes it.
pub(crate) struct CommandPath {
    pub(crate) path: String,
    /// `rm -f build.log` and `cp x public/` name a path the command writes or
    /// deletes. The project does not have to hold it already, but the command
    /// still touches it, so it stays a path with a note on it.
    pub(crate) written: bool,
}

/// The paths a command names, in the order it names them.
///
/// `(cd ..; ./runtest)` runs `runtest` one directory up, so a `cd` moves what
/// everything after it is relative to. `echo "see <repo>/src"` names nothing:
/// what follows `echo` is text for a person to read. Chained commands are
/// read one at a time, because each one has its own program.
fn command_paths(command: &str) -> Vec<CommandPath> {
    let mut base = None;
    let mut paths = Vec::new();
    for segment in command.split([';', '&', '|']) {
        let mut tokens = split_command_tokens(segment);
        while tokens
            .first()
            .is_some_and(|token| matches!(token.as_str(), "sudo" | "env" | "exec" | "nohup"))
        {
            tokens.remove(0);
        }
        let Some(program) = tokens.first().cloned() else {
            continue;
        };
        if program == "cd" && tokens.len() > 1 {
            base = Some(tokens[1].clone());
            continue;
        }
        if program == "echo" || names_packages_or_targets(&program) {
            continue;
        }

        let written = writes_its_paths(&program);
        let mut segment_paths = Vec::new();
        let mut redirected = false;
        for token in tokens {
            // `> /dev/null` says where the output goes, not what runs.
            if token
                .chars()
                .all(|character| matches!(character, '>' | '<' | '&' | '0'..='9'))
            {
                redirected = true;
                continue;
            }
            if std::mem::take(&mut redirected) || !is_command_path_candidate(&token) {
                continue;
            }
            segment_paths.push(CommandPath {
                path: match base.as_deref() {
                    Some(base) => format!("{base}/{token}"),
                    None => token,
                },
                written,
            });
        }
        if last_path_is_a_destination(&program)
            && let Some(destination) = segment_paths.last_mut()
        {
            destination.written = true;
        }
        paths.append(&mut segment_paths);
    }
    paths
}

/// Programs whose arguments name packages or build targets rather than files:
/// `brew install alamofire/alamofire/firewalk` names a tap and a formula, and
/// `sbt docs/tlSite` names a project and the task to run in it.
fn names_packages_or_targets(program: &str) -> bool {
    matches!(
        program,
        // `composer require mongodb/mongodb --dev` names a package, and
        // monolog's CI installs three that way.
        "brew" | "sbt" | "gradle" | "mvn" | "gem" | "apt-get" | "composer"
    )
}

/// Programs whose path arguments are what the command writes rather than what
/// it reads: `rm -f t/phase_checks.stats` deletes a file a test run left, and
/// `tail -F servroot/logs/error.log` follows a log the server writes.
fn writes_its_paths(program: &str) -> bool {
    matches!(
        program,
        "rm" | "rmdir" | "mkdir" | "touch" | "tail" | "unzip" | "tar"
    )
}

/// `cp a b` and `mv a b` end with where the copy goes; only what comes before
/// names a file the project has to hold already.
fn last_path_is_a_destination(program: &str) -> bool {
    matches!(program, "cp" | "mv" | "ln" | "install")
}

/// The path a command names, read from the directory the file says it runs
/// in: a GitHub Actions step with `working-directory: pkgs/http` runs
/// `dart run test/x_test.dart` on `pkgs/http/test/x_test.dart`.
pub(crate) fn command_path_in_directory(
    command: &str,
    directory: Option<&str>,
) -> Option<CommandPath> {
    let candidate = root_relative_command_path_candidate(command)?;
    let Some(directory) = directory.map(str::trim).filter(|value| !value.is_empty()) else {
        return Some(candidate);
    };
    let joined = format!("{}/{}", directory.trim_end_matches('/'), candidate.path);
    normalize_relative_path(Path::new(&joined)).map(|path| CommandPath {
        path,
        written: candidate.written,
    })
}

pub(crate) fn root_relative_command_path_candidate(command: &str) -> Option<CommandPath> {
    command_paths(command).into_iter().find_map(|candidate| {
        normalize_relative_path(Path::new(&candidate.path)).map(|path| CommandPath {
            path,
            written: candidate.written,
        })
    })
}

pub(crate) fn cmake_command_bodies(source: &str, command_name: &str) -> Vec<String> {
    cmake_command_sites(source, command_name)
        .into_iter()
        .map(|(body, _)| body)
        .collect()
}

/// Every call of a CMake command together with the line it is written on.
/// A reader following `add_executable(hiredis-test ...)` wants that line,
/// and searching the file for the name later finds `ADD_TEST(NAME
/// hiredis-test` instead.
pub(crate) fn cmake_command_sites(source: &str, command_name: &str) -> Vec<(String, u32)> {
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
        let line = source[..start]
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count() as u32
            + 1;
        bodies.push((body, line));
        search_from = close + 1;
    }

    bodies
}

pub(crate) fn strip_cmake_comments(source: &str) -> String {
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

pub(crate) fn cmake_parenthesized_body(source: &str, open: usize) -> Option<(String, usize)> {
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

pub(crate) fn cmake_command_args(body: &str) -> Vec<String> {
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

pub(crate) fn is_cmake_source_argument(value: &str) -> bool {
    let value = value.trim();
    !value.starts_with('$') && !value.starts_with('<') && has_known_source_extension(value)
}

pub(crate) fn is_cmake_ident_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

pub(crate) fn split_command_tokens(command: &str) -> Vec<String> {
    command
        .split(|character: char| {
            character.is_whitespace() || matches!(character, '"' | '\'' | '`' | ';' | '&' | '|')
        })
        .filter_map(clean_command_token)
        .collect()
}

pub(crate) fn clean_command_token(token: &str) -> Option<String> {
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

pub(crate) fn is_command_path_candidate(token: &str) -> bool {
    if token.starts_with('-')
        // `dune build @doc/runtest` names a build alias, and a leading `@` in
        // a Makefile recipe only says not to echo the line.
        || token.starts_with('@')
        // `src/$(REDIS_BENCHMARK_NAME)` is whatever the make variable
        // holds, wherever the variable appears in the token.
        || token.contains('$')
        || token.contains("://")
        || token.contains('=')
        || token.contains('*')
        // `go generate ./...` walks every package under here. The three
        // dots are the walk, not a directory anybody can open.
        || token.split('/').any(|segment| segment == "...")
    {
        return false;
    }
    token.contains('/') || has_known_source_extension(token)
}

pub(crate) fn has_known_source_extension(path: &str) -> bool {
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

pub(crate) fn normalize_manifest_relative_path(
    manifest_label: &str,
    target: &str,
) -> Option<String> {
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

pub(crate) fn normalize_relative_path(path: &Path) -> Option<String> {
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

pub(crate) fn function_targets_in_file(graph: &CodeGraph, path: &str, symbol: &str) -> Vec<NodeId> {
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

pub(crate) fn function_symbol_matches(label: &str, symbol: &str) -> bool {
    symbol_keys(label)
        .into_iter()
        .any(|key| key == symbol || simple_symbol_name(&key) == symbol)
}

pub(crate) fn add_entrypoint_reference(
    context: &mut IndexContext,
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
    } else if resolution.starts_with("make") {
        "makefile"
    } else if resolution.starts_with("docker") {
        "dockerfile"
    } else if resolution.starts_with("compose") {
        "compose"
    } else if resolution.starts_with("github_actions") {
        "github-actions"
    } else if resolution.starts_with("gitlab_ci") {
        "gitlab-ci"
    } else {
        "manifest"
    };
    metadata.insert("source".to_string(), fact_source.to_string());
    metadata.insert("resolution".to_string(), resolution.to_string());
    if let Some(target_symbol) = target_symbol {
        metadata.insert("target_symbol".to_string(), target_symbol.to_string());
    }
    add_edge_once_with_metadata(
        context,
        source,
        target,
        EdgeKind::References,
        confidence,
        metadata,
    );
}

pub(crate) fn register_function_symbol(
    symbols: &mut BTreeMap<String, Vec<NodeId>>,
    label: &str,
    id: NodeId,
) {
    for key in symbol_keys(label) {
        let values = symbols.entry(key).or_default();
        if !values.contains(&id) {
            values.push(id);
        }
    }
}

pub(crate) fn register_local_function(
    symbols: &mut BTreeMap<String, NodeId>,
    label: &str,
    id: NodeId,
) {
    for key in symbol_keys(label) {
        symbols.entry(key).or_insert(id);
    }
}

pub(crate) fn resolve_local_function(
    symbols: &BTreeMap<String, NodeId>,
    label: &str,
) -> Option<NodeId> {
    let (compact, simple) = symbol_key_parts(label);
    symbols
        .get(compact)
        .or_else(|| symbols.get(simple))
        .copied()
}

/// Split `Type::method` / `Type.method` into its owner and method name. Only
/// the last separator matters, so `a::b::Type::method` yields (`Type`,
/// `method`).
pub(crate) fn split_qualified_call(label: &str) -> Option<(&str, &str)> {
    let label = label.trim().trim_end_matches('!');
    let (owner, method) = label.rsplit_once("::").or_else(|| label.rsplit_once('.'))?;
    // Keep only the final path segment of the owner: `a::b::Type` -> `Type`.
    let owner = owner
        .rsplit("::")
        .next()
        .unwrap_or(owner)
        .rsplit('.')
        .next()
        .unwrap_or(owner);
    (!owner.is_empty() && !method.is_empty()).then_some((owner, method))
}

/// What the function a `:=` names hands back. A Go call written as a bare
/// name means the caller's own package, and a package is a directory, so the
/// answer is read from the definitions next door. Two definitions of that
/// name that state different types are two answers, which is no answer.
fn what_a_call_hands_back(
    context: &IndexContext,
    caller_path: Option<&str>,
    bound_by: &str,
    declared_in: Option<&[String]>,
) -> Option<String> {
    let callee = bound_by.strip_suffix("()")?;
    // `mgr := b.StateMgr()` names a method of a type the signature already
    // stated, and the owner picks the definition where a directory cannot:
    // a bare name means the caller's own package, which is one directory.
    let (owner, callee) = match callee.rsplit_once('.') {
        Some((owner, method)) => (Some(owner.rsplit(['.', '*']).next()?), method),
        None => (None, callee),
    };
    let directory = caller_path
        .and_then(|path| path.rsplit_once('/'))
        .map(|(directory, _)| directory);
    let mut handed_back: Option<String> = None;
    for target in resolve_function_targets(&context.function_symbols, callee) {
        let Some(node) = graph_node(&context.graph, target) else {
            continue;
        };
        let named_by = match owner {
            Some(owner) => {
                node.metadata
                    .get("owner_type")
                    .is_some_and(|declared| declared == owner)
                    || declared_in.is_some_and(|candidates| {
                        node.span.as_ref().is_some_and(|span| {
                            candidates
                                .iter()
                                .any(|candidate| declared_in_module(&span.path, candidate))
                        })
                    })
            }
            None => node
                .span
                .as_ref()
                .and_then(|span| span.path.rsplit_once('/'))
                .is_some_and(|(declared_in, _)| Some(declared_in) == directory),
        };
        if !named_by {
            continue;
        }
        let Some(returns) = node.metadata.get("returns") else {
            continue;
        };
        match &handed_back {
            Some(stated) if stated != returns => return None,
            Some(_) => {}
            None => handed_back = Some(returns.clone()),
        }
    }
    handed_back
}

pub(crate) fn resolve_function_targets(
    symbols: &BTreeMap<String, Vec<NodeId>>,
    label: &str,
) -> Vec<NodeId> {
    // The keys are slices of the label the caller already holds: building
    // them as owned strings cost terraform's 100000 calls two allocations
    // each before a single map lookup happened.
    let (compact, simple) = symbol_key_parts(label);
    // Each key's own list is already without repeats, so only the second
    // one is checked against the first: `Run` names hundreds of
    // definitions in terraform, and checking each against a growing list
    // made the merge cost grow with the square of that.
    let mut targets: Vec<NodeId> = symbols.get(compact).cloned().unwrap_or_default();
    if simple != compact
        && let Some(ids) = symbols.get(simple)
    {
        for id in ids {
            if !targets.contains(id) {
                targets.push(*id);
            }
        }
    }
    targets
}

/// The two names a label can be looked up under: what it says, and the
/// last part of it. `Type::method` is also `method`.
pub(crate) fn symbol_key_parts(label: &str) -> (&str, &str) {
    let compact = label.trim().trim_end_matches('!');
    (compact, simple_symbol_name_ref(compact))
}

pub(crate) fn symbol_keys(label: &str) -> Vec<String> {
    let (compact, simple) = symbol_key_parts(label);
    if compact == simple {
        vec![compact.to_string()]
    } else {
        vec![compact.to_string(), simple.to_string()]
    }
}

/// [`simple_symbol_name`] without the allocation: the answer is a slice of
/// what was passed in.
pub(crate) fn simple_symbol_name_ref(label: &str) -> &str {
    label
        .rsplit([':', '.', '\\', '>'])
        .find(|part| !part.is_empty() && *part != "-")
        .unwrap_or(label)
        .trim()
}

pub(crate) fn simple_symbol_name(label: &str) -> String {
    simple_symbol_name_ref(label).to_string()
}

pub(crate) fn add_edge_once(
    context: &mut IndexContext,
    source: NodeId,
    target: NodeId,
    kind: EdgeKind,
    confidence: Confidence,
) {
    add_edge_once_with_metadata(context, source, target, kind, confidence, BTreeMap::new());
}

pub(crate) fn add_edge_once_with_metadata(
    context: &mut IndexContext,
    source: NodeId,
    target: NodeId,
    kind: EdgeKind,
    confidence: Confidence,
    metadata: BTreeMap<String, String>,
) {
    // Absorb edges appended since the last call (some passes push straight
    // onto graph.edges); the key set then mirrors graph.edges exactly, so the
    // set probe below is equivalent to the old linear scan at O(log E).
    for edge in &context.graph.edges[context.edge_keys_synced..] {
        context
            .edge_keys
            .insert((edge.source, edge.target, edge.kind));
    }
    if !context.edge_keys.insert((source, target, kind)) {
        context.edge_keys_synced = context.graph.edges.len();
        return;
    }
    context
        .graph
        .add_edge_with_metadata(source, target, kind, confidence, metadata);
    context.edge_keys_synced = context.graph.edges.len();
}

pub(crate) fn add_file_metadata(
    graph: &mut CodeGraph,
    file_id: codegraph_core::NodeId,
    key: &str,
    value: impl Into<String>,
) {
    add_node_metadata(graph, file_id, key, value);
}

pub(crate) fn add_node_metadata(
    graph: &mut CodeGraph,
    node_id: codegraph_core::NodeId,
    key: &str,
    value: impl Into<String>,
) {
    // Nodes are appended in id order, so the id is the index: searching the
    // whole graph for one node, once per fact, cost terraform's resolve
    // pass a fifth of its time.
    if let Some(node) = graph_node_mut(graph, node_id) {
        node.metadata.insert(key.to_string(), value.into());
    }
}
