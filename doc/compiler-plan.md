# Compiler Plan

This document plans the path from the current implementation to a **self-hosted compiler**: an ez compiler written in ez that produces native binaries. It records the language decisions already made and the ones still open.

> Status: planning. Nothing in this document is implemented yet unless stated otherwise.

---

## Where we are

Today `ez <file>` runs four stages, all written in Rust:

1. **Lexer** (`src/lexer/`): source text → tokens.
2. **Parser** (`src/parser/`): tokens → AST.
3. **Type checker** (`src/semantic/`): checks the whole program, including imported modules, before anything runs.
4. **Interpreter** (`src/runtime/`): walks the AST and executes it. There is no bytecode and no machine code.

`src/ir/` exists but is an empty stub.

---

## Bootstrap strategy

A self-hosted compiler needs an existing compiler to be built the first time. The Rust implementation plays that role.

```
stage 0: ez in Rust (current implementation + a code generator)
    │ compiles
    ▼
stage 1: ezc written in ez, compiled by stage 0
    │ compiles its own source
    ▼
stage 2: ezc compiled by ezc
    │ compiles its own source again
    ▼
stage 3: must be identical to stage 2 (fixed point)
```

When stage 2 and stage 3 produce the same output, the compiler sustains itself.

**Backend:** generate C first (pending confirmation, see [Open decisions](#open-decisions)). It needs no LLVM or Cranelift bindings, works on any platform with a C compiler, and the generated code is readable, which makes debugging much easier. A native backend (LLVM, Cranelift or direct assembly) can be added later.

---

## Decisions

### No `any` type

`any` is removed from the language. Dynamic data is represented by sum types. As a consequence, no value needs to carry its type at runtime.

JSON becomes a sum type in the `json` module:

```ez
type Json = {
    Null
    Bool(value: boolean)
    Int(value: int)
    Float(value: float)
    String(value: string)
    Array(items: Json[])
    Object(fields: JsonObject)
}

type JsonObject = {
    [key: string]: Json
}
```

-   `json.parse(text: string) -> Result[Json, JsonError]`
-   `json.stringify(value: Json) -> string`
-   `Int` and `Float` are separate variants:
    -   A number without `.`, `e` or `E` that fits in 64 bits is an `Int`.
    -   A number with `.`, `e` or `E` is a `Float`.
    -   An integer too large for 64 bits is `Err(NumberOutOfRange(...))`.
-   `json.stringify(Float(3.0))` writes `3.0`, so that parsing it back gives a `Float` again.
-   `Int(1) == Float(1.0)` is `false`: they are different variants.
-   Helpers such as `json.get(value, key) -> Option[Json]` and `json.asFloat(value) -> Option[float]` (accepting both `Int` and `Float`) reduce the verbosity of matching.
-   Later, with user-defined generics: `json.decode[T](text) -> Result[T, JsonError]`.

### Mandatory `export`

Everything in a module is private by default. Only declarations marked with `export` can be imported.

-   Exportable: `export function`, `export const`, `export var`, `export type` and `export interface`.
-   Exporting a sum type also exports its variants.
-   `export` is only allowed at the top level of a file.
-   Implicit declarations (`x = 1`) cannot be exported.
-   **`export var`** is read-only outside its module:
    -   Importers always see the **current** value, not a copy taken at import time.
    -   Only the **binding** is read-only: like `const`, the contents of a list or object can still be changed.

    ```ez
    // file: counter.ez
    export var count = 0

    export function increment() {
        count += 1
    }
    ```

    ```ez
    import { count, increment } from 'counter.ez'

    increment()
    print(count)    // 1
    count = 5       // Error! 'count' is read-only outside 'counter.ez'
    ```

-   Importing something that is not exported is an error: `'helper' is not exported by 'math.ez'`.
-   A public signature cannot use a private type (e.g. `export function f(): Helper` with a private `Helper`).
-   The standard library is unaffected: all of its members are public.
-   Later: re-exports (`export { add } from 'math.ez'`).

### No `try` / `catch`

Errors are values, not exceptions:

| Kind                     | Examples                                                       | Handling                                      |
| ------------------------ | -------------------------------------------------------------- | --------------------------------------------- |
| Expected (recoverable)   | missing file, invalid JSON, network failure, `int('abc')`      | `Result`                                      |
| Bug (unrecoverable)      | index out of range, division by zero, integer or float overflow | **panic**: the program stops, it cannot be caught |

A built-in `panic(message)` lets code report impossible states.

### Typed errors in the standard library

Every fallible standard library function returns a `Result` with an error type specific to its module:

```ez
// file
type FileError = {
    NotFound(path: string)
    PermissionDenied(path: string)
    IsDirectory(path: string)
    InvalidUtf8(path: string)
    Io(path: string, message: string)
}

// http (4xx and 5xx statuses are still Ok(Response): they are not transport failures)
type HttpError = {
    InvalidUrl(url: string)
    ConnectionFailed(url: string, message: string)
    Timeout(url: string)
    Tls(url: string, message: string)
    InvalidResponse(url: string, message: string)
}

// json
type JsonError = {
    Syntax(message: string, line: int, column: int)
    NumberOutOfRange(text: string, line: int, column: int)
}

// datetime.parse
type DateError = {
    PatternMismatch(text: string, pattern: string)
    InvalidDate(text: string)
}

// regex
type RegexError = {
    InvalidPattern(pattern: string, message: string)
}

// int(x) and float(x) with a string
type ParseError = {
    InvalidNumber(text: string)
    OutOfRange(text: string)
}

// math
type MathError = {
    Complex          // the result exists only in the complex numbers: sqrt(-1), log(-1)
    Indeterminate    // indeterminate form
}
```

`time` has no error type. `input()` keeps returning `Option[string]`.

### No NaN, no infinity

`float` is always a **finite real number**:

-   There is no NaN. Math functions whose result is not a real number return `Err(Complex)` or `Err(Indeterminate)`.
-   There is no infinity. Float overflow (`1e308 * 10`) panics, like integer overflow.
-   Float literals that are too large (`1e400`) are compile-time errors.
-   `float('1e400')` returns `Err(OutOfRange(...))`.
-   `MathError` only describes **domain** problems (the result does not exist in the reals). Overflow is a **representation** limit (the result exists but does not fit), so it does not belong to `MathError`.

### Representation limits panic, domain problems are `MathError`

One rule decides how numeric failures are reported:

| Problem | Meaning | Handling | Examples |
| --- | --- | --- | --- |
| **Representation limit** | The result exists but does not fit in an `int` or a `float` | panic | `9223372036854775807 + 1`, `1e308 * 10`, `math.pow(10, 400)`, `math.round(1e300)` |
| **Domain problem** | The result does not exist in the real numbers | `Err(MathError)` | `math.sqrt(-1)` → `Complex`, `math.pow(0, 0)` → `Indeterminate` |

Operators cannot return a `Result`, so they always panic, including on domain problems such as division by zero.

### Division (already implemented)

Like Go and Rust: `int / int` gives an `int` truncated toward zero, a division with a `float` gives a `float`, and `%` has the sign of the dividend. `//` is always a comment.

---

## What the language needs to support the compiler

A compiler is a large program, with many data structures and many operations that can fail. These features are needed before writing `ezc` in ez, in order of importance:

1.  **User-defined generics**: `type Map[K, V]`, `function find[T](...)`, using the same bracket syntax as `Option[T]`.
2.  **Collections**:
    -   lists with `push`, `pop`, `insert` and slicing;
    -   maps with any comparable key type (index signatures only allow `string` keys today);
    -   sets.
3.  **Text processing**:
    -   a `char` (or `byte`) type and conversion to and from character codes;
    -   `substring`/slicing, `startsWith`, `indexOf` and a string builder;
    -   efficient indexing (`s[i]` is O(n) today).
4.  **Error propagation**: the `?` operator (see [Open decisions](#open-decisions)).
5.  **Methods on types** (`Type.method()`, already on the roadmap).
6.  **Complete type inference**: today the checker tolerates unresolved types (`x = None`, `[]`). Code generation needs every type resolved, which requires real unification.
7.  **System access**:
    -   command-line arguments;
    -   the exit code (`exit`);
    -   environment variables;
    -   running processes (to call the C compiler);
    -   path manipulation;
    -   `panic`.
8.  **FFI** (`extern function`), so the compiled standard library can call C functions.
9.  **Module visibility**: covered by the mandatory `export` decision.
10. **Testing framework** (on the roadmap): not required, but it speeds up compiler development a lot.

### Semantics to settle before compiling

-   **Closures** capture variables by reference. Compiled, captured variables live in heap cells (closure conversion).
-   **`int` to `float` coercion** is inconsistent today (an `int` stored in a `float` field of an object stays an `int`). With complete types, every conversion is explicit in the generated code.
-   **Memory management**: see [Open decisions](#open-decisions).

---

## Phases

| Phase | Delivers | Validation |
| --- | --- | --- |
| **0. Decisions** | The open decisions below, and the documentation updated | Documentation reviewed |
| **1. Language** | The decisions above and the features in [What the language needs](#what-the-language-needs-to-support-the-compiler), implemented in the Rust checker and interpreter | Tests like the current ones |
| **2. Compiler in Rust** | A typed IR in `src/ir/` (monomorphization, closure conversion, `match` compiled to decision trees), a C code generator, a C runtime (strings, lists, maps, memory management) and an `ez build` command | Every test runs both interpreted and compiled, with identical output |
| **3. `ezc` in ez** | Lexer → parser → checker → IR → C generator, in this order | At each step, compare with the Rust implementation: same tokens, same AST, same errors, same generated C |
| **4. Bootstrap** | Stages 1 → 2 → 3 | Stage 2 output identical to stage 3 |
| **5. Maintenance** | A bootstrap policy: freeze the Rust implementation as stage 0, or commit the generated C of `ezc` so that only a C compiler is needed to build everything | CI rebuilds every stage |

Notes:

-   **Compiled standard library**: `json` and `regex` can be rewritten in ez. `http` with TLS is heavy: it can go through FFI (e.g. libcurl) or stay outside the core.
-   **Rule for `ezc`**: it may only use features supported by the previous compiler version, otherwise the bootstrap breaks.

---

## Open decisions

1.  **Memory management**: garbage collection (recommended to start, e.g. Boehm through the C backend), reference counting (leaks cycles) or another approach.
2.  **Initial backend**: C (recommended) or LLVM.
3.  **The interpreter**: keep it as `ez run` for fast development, or retire it once the compiler exists.
4.  **The `?` operator**, with these rules:
    -   on a `Result`, `x?` gives the `Ok` value or returns the same `Err` from the function;
    -   on an `Option`, `x?` gives the `Some` value or returns `None` (only in functions returning `Option`);
    -   at the top level of the main file, an `Err` stops the program with its message and exit code 1.
5.  **Error conversion with `?`** when a function combines errors from different modules:

    ```ez
    type ConfigError = {
        File(error: file.FileError)
        Json(error: json.JsonError)
    }
    ```

    -   (a) explicit only: `file.open(path, Read).mapErr(File)?`;
    -   (b) automatic with a strict rule (recommended): `?` wraps the error when the function's error type has **exactly one** variant whose only field has the received error type. Otherwise it is a compile error and `mapErr` is required. `mapErr` is always available.
6.  **`defer`** to release resources (`defer f.close()`), since there is no `try` / `finally`.
7.  **Standard library API changes**:
    -   `file.open(path, mode)` with `type Mode = { Read  Write  Append }` instead of `'r'`, `'w'`, `'a'`. Reading a file opened for writing, or using a closed file, panics.
    -   `regex.compile(pattern) -> Result[Regex, RegexError]`, with infallible methods on `Regex`.
    -   A `.message()` method on every standard library error type, returning a human-readable text.
8.  **`math.pow(0, y)` with a negative `y`**: the result is not a real number (like `1 / 0`), but it is neither `Complex` nor `Indeterminate`. Options: panic, as division by zero; or a new `MathError` variant such as `Undefined`.
