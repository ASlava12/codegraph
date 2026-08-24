//! Post-scan resolution passes over pending queues: calls, imports,
//! entrypoint targets, compose/Kubernetes/CI references, documents, SQL,
//! and the function symbol registry.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use codegraph_core::{CodeGraph, Confidence, EdgeKind, NodeId, NodeKind, is_test_like_source_path};

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
        ),
        "dart" => matches!(base, "print" | "identical" | "assert"),
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
fn add_external_call_placeholder(context: &mut IndexContext, call: PendingCall) {
    let key = (call.language.clone(), call.label.clone());
    let label = call.label.clone();
    let language = call.language.clone();
    let line = call.span.start_line;
    let column = call.span.start_column;
    let call_id = if let Some(id) = context.unresolved_call_placeholders.get(&key) {
        *id
    } else {
        let mut metadata = BTreeMap::new();
        metadata.insert("language".to_string(), call.language);
        metadata.insert("parser".to_string(), "tree-sitter".to_string());
        metadata.insert("item_kind".to_string(), "call".to_string());
        metadata.insert("resolution".to_string(), "external".to_string());
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
            ("resolution".to_string(), "external".to_string()),
            ("language".to_string(), language),
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
fn one_methods_overloads(graph: &CodeGraph, targets: &[NodeId]) -> bool {
    let mut owner: Option<String> = None;
    let mut label: Option<String> = None;
    let mut directory: Option<String> = None;
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
        if directory.get_or_insert_with(|| node_directory.to_string()) != node_directory {
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

pub(crate) fn resolve_pending_calls(context: &mut IndexContext) {
    let pending_calls = std::mem::take(&mut context.pending_calls);
    // What the scan holds does not change while calls are resolved, so each
    // candidate path is looked for once.
    let mut scanned_candidates: BTreeMap<String, bool> = BTreeMap::new();
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
            add_external_call_placeholder(context, call);
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
            add_external_call_placeholder(context, call);
            continue;
        }
        // A qualified name the language itself provides is answered by the
        // language. `Object.create(...)` in axios shares only its tail with
        // the repository's `instance.create`, and matching on that tail
        // invented a dependency cycle between two files that never call
        // each other.
        let all_targets =
            if call.label.contains('.') && builtin_call_target(&call.language, &call.label) {
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
        language_targets.retain(|target| {
            graph_node(&context.graph, *target)
                .and_then(|node| node.metadata.get("language"))
                .is_some_and(|language| language == &call.language)
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
        if receiver_call_is_universal(&call.language, &call.label) {
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
        let caller_is_test = caller_path.is_some_and(is_test_like_source_path);
        if !caller_is_test {
            language_targets.retain(|target| {
                graph_node(&context.graph, *target)
                    .and_then(|node| node.span.as_ref())
                    .is_none_or(|span| !is_test_like_source_path(&span.path))
            });
        }
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
            // `from collections import OrderedDict` binds a bare name, so
            // the call site has no qualifier to look up — the name itself
            // has to say where it came from. A definition the file makes
            // itself wins over the import, which is Python's own rule.
            None if local_targets.is_empty() => context
                .file_imported_names
                .get(call.span.path.as_str())
                .and_then(|names| names.get(&call.label))
                .cloned(),
            None => None,
        }
        .map(|package| resolved_import_package(context, &mut scanned_candidates, package));
        if imported_package == Some(ImportedPackage::External) {
            add_external_call_placeholder(context, call);
            continue;
        }
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
                // Narrow only when the import actually names something the
                // scan found: an import whose module was never scanned must
                // not erase the candidates matched by name.
                if in_module.is_empty() {
                    language_targets
                } else {
                    basis = "import";
                    in_module
                }
            }
            _ if !local_targets.is_empty() => {
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
                    let Some(enclosing) = graph_node(&context.graph, *target)
                        .and_then(|node| node.metadata.get("enclosing_function"))
                    else {
                        // Top level: visible to the whole module.
                        return true;
                    };
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

        // A receiver whose declared type comes from outside the repository
        // cannot be calling anything in it: `t.Fatalf()` on a `*testing.T` is
        // 7564 of terraform's calls, and reporting them as unresolved suggests
        // a resolver that failed rather than a dependency that left.
        if let Some((package, _)) = call
            .receiver_type
            .as_deref()
            .and_then(|receiver_type| receiver_type.split_once('.'))
            && context
                .file_import_qualifiers
                .get(call.span.path.as_str())
                .and_then(|qualifiers| qualifiers.get(package))
                == Some(&ImportedPackage::External)
        {
            add_external_call_placeholder(context, call);
            continue;
        }

        // The receiver's declared type names the method's owner directly, so
        // it settles the choice that the label alone cannot: `b.Configure()`
        // inside `func (b *Backend)` is `Backend.Configure`. When the type is
        // qualified — `diags tfdiags.Diagnostics` — the package narrows it the
        // rest of the way: terraform declares `Diagnostics` in more than one,
        // so the owner's name alone still left a choice.
        if let Some(receiver_type) = call.receiver_type.as_deref()
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
            if !owned.is_empty() {
                basis = "receiver_type";
                targets = owned;
            }
        }

        // A qualified call (`CodeGraph::new`, `Foo.bar`) matches many bare
        // `new`/`bar` declarations; keep only methods whose owning type is the
        // one named in the call, which turns an ambiguous set into one edge.
        if let Some((owner, _)) = split_qualified_call(&call.label)
            && targets.len() > 1
        {
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
                .collect::<Vec<_>>();
            if type_targets.len() == 1 {
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
                || objc_platform_receiver(&call.language, call.receiver.as_deref());
            if type_targets.len() > 1 && !is_builtin {
                targets = type_targets;
                ambiguous_candidates_are_types = true;
            } else {
                let key = (call.language.clone(), call.label.clone());
                let resolution = if is_builtin { "builtin" } else { "unresolved" };
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

        let overloads = targets.len() > 1 && one_methods_overloads(&context.graph, &targets);
        if overloads && basis == "name" {
            basis = "overload";
        }

        // A syntactic label such as `build`, `read`, or `close` is often
        // shared by hundreds of methods. Connecting the caller to every
        // matching declaration invents dependencies and makes E grow toward
        // O(call-sites * duplicate-labels). Preserve the uncertainty as one
        // bounded node instead; semantic enrichment can replace it later.
        if targets.len() > 1 && !overloads {
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

pub(crate) fn resolve_pending_type_references(context: &mut IndexContext) {
    let pending = std::mem::take(&mut context.pending_type_references);
    let mut seen = BTreeSet::new();

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
                    .is_some_and(|language| language == &reference.language);
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

        // A Terraform module is a directory, and a name written inside one
        // means what that directory declares: terraform's fixtures declare
        // `var.input` in 40 directories, and only the one next door is the
        // variable this expression reads.
        let targets = if reference.language == "hcl" && targets.len() > 1 {
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
                if (path == *module || path.starts_with(&format!("{module}/")))
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

pub(crate) fn resolve_pending_route_handlers(context: &mut IndexContext) {
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
                        .is_some_and(|declared| declared == language)
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
            let owner_tail = owner.rsplit("::").next().unwrap_or(owner);
            let owner_is = |target: &NodeId, match_kind: OwnerMatch| {
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
                    .filter(|target| owner_is(target, match_kind))
                    .collect();
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

pub(crate) fn resolve_function_targets(
    symbols: &BTreeMap<String, Vec<NodeId>>,
    label: &str,
) -> Vec<NodeId> {
    let mut targets = Vec::new();
    // The keys are slices of the label the caller already holds: building
    // them as owned strings cost terraform's 100000 calls two allocations
    // each before a single map lookup happened.
    let (compact, simple) = symbol_key_parts(label);
    for key in [compact, simple] {
        if key == simple && !std::ptr::eq(key, compact) && compact == simple {
            continue;
        }
        if let Some(ids) = symbols.get(key) {
            for id in ids {
                if !targets.contains(id) {
                    targets.push(*id);
                }
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
