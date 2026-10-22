# Compiler Plan

This document plans the path from the current implementation to a **self-hosted compiler**: an ez compiler written in ez that produces native binaries. It records the language decisions already made and the ones still open.

> Status: the [decisions](#decisions) below are implemented in the current (interpreted) implementation, except where noted. The phases that build the compiler (2 to 5) have not started.

---

## Where we are

Today `ez run <file>` runs four stages, all written in Rust:

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

**Backend:** generate C first (decided). It needs no LLVM or Cranelift bindings, works on any platform with a C compiler, and the generated code is readable, which makes debugging much easier. A native backend (LLVM, Cranelift or direct assembly) can be added later.

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

-   Exportable: `export function`, `export var`, `export const`, `export imut` and `export type`.
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
    print(count) // 1
    count = 5 // Error! 'count' is read-only outside 'counter.ez'
    ```

-   Importing something that is not exported is an error: `'helper' is not exported by 'math.ez'`.
-   A public signature cannot use a private type (e.g. `export function f(): Helper` with a private `Helper`).
-   The standard library is unaffected: all of its members are public.
-   Later: re-exports (`export { add } from 'math.ez'`).

### Tooling: `ez run` and `ez build`

The interpreter stays: `ez run` checks and interprets a program, for fast development. The compiler will be `ez build`, producing a native binary. Both must give the same results (every test runs in both modes).

### The `?` operator

`?` propagates errors instead of writing a `match` for every fallible call:

-   on a `Result`, `x?` gives the `Ok` value, or returns the same `Err` from the function;
-   on an `Option`, `x?` gives the `Some` value, or returns `None` (only in functions returning `Option`);
-   at the top level of the main file, an `Err` stops the program with its message and exit code 1.

```ez
type ConfigError = {
    File(error: file.FileError)
    Json(error: json.JsonError)
}

function readConfig(path: string): Result[json.Json, ConfigError] {
    f = file.open(path, file.Read)? // FileError → File(...)
    text = f.read()?

    return Ok(json.parse(text)?) // JsonError → Json(...)
}
```

**Error conversion** is automatic, with a strict rule: when the error type differs from the function's error type, `?` wraps it in the variant of the function's error type that has **exactly one** field of that type. If there is no such variant, or more than one, it is a compile error, and `result.mapErr(f)` converts the error explicitly.

Implemented; see [Error Handling](error-handling.md).

### Methods

A method is a function prefixed with its type, with an explicit `self` parameter (not `this`):

```ez
type Person = {
    name: string
}

function Person.greet(self) {
    print(`Hello, I'm ${self.name}!`)
}

const person: Person = {
    name: 'John'
}

person.greet()
```

A function prefixed with its type but without `self` is an **associated function**, called on the type: `function Person.create(name: string): Person`, called as `Person.create('John')` (e.g. for constructors).

A method that changes its value declares **`mut self`**. The call does not repeat the marker (`counter.increment()`), because the method is already called on the variable; the value must be a `var` or a `const` (or a field or element of one), never an `imut`. `mut self` changes the contents, but cannot replace the value (`self = ...` is an error), which is what makes calling it on a `const` consistent.

Implemented; see [Custom Types](custom-types.md#methods).

### Memory management: ownership, value semantics and drop

Memory is managed with **ownership**, without a garbage collector. Every value has exactly one owner, and when the owner goes out of scope the value is **dropped**: its memory is released and, for resources, the resource is closed. This is deterministic, so there is no `defer`.

```ez
function readNotes(): Result[string, file.FileError] {
    f = file.open('notes.txt', file.Read)?
    content = f.read()? // on an error, '?' returns and 'f' is closed
    return Ok(content) // 'f' goes out of scope: the file is closed
}
```

**Value semantics with copy-on-write**, as in Swift, Hylo and Mojo. Assigning or passing a value gives an independent copy: there is no aliasing, and no variable becomes unusable (unlike a move). Underneath, the memory is shared until one of the copies is modified, so copies are cheap.

```ez
a = [1, 2, 3]
b = a // nothing is copied yet
b[0] = 9 // the list is copied now, because b changes
print(a) // [1, 2, 3]
print(b) // [9, 2, 3]
```

Consequences:

-   **`is` only tests variants** (`result is Some`). There is no reference identity; `==` compares values.
-   **Three mutability levels**: `var` (everything can change), `const` (the variable cannot be reassigned, its contents can change, as in TypeScript) and `imut` (neither the variable nor its contents can change). `imut` is checked at compile time. With value semantics, `const` only prevents replacing the value as a whole. A separate keyword for compile-time constants is not needed: the compiler can recognize an `imut` with a literal value by itself. Until the interpreter uses copy-on-write, an `imut` list passed to a function can still be changed there, because it is shared by reference.
-   **`defer` is not needed**: dropping releases resources. It may come back later as an optional feature, for cleanup that does not belong to any value.
-   **Resource types are move-only.** A `File` cannot be copied (two copies would close the same file): `g = f` moves it, and using `f` afterwards is a compile error. `f.close()` consumes `f`, so using a closed file is a compile error instead of a panic.

The current interpreter still uses shared references (and `is` still compares references between objects): it will be changed to these semantics in phase 1, so that `ez run` and `ez build` behave the same.

### Value semantics details

-   **Closures capture a read-only copy** of the variables they use: assigning to a captured variable (or changing its contents) is a compile error, and the closure sees the value from the moment it was created. Module variables are not captured: functions read and change them directly. State that changes is kept another way (e.g. a type with methods). Implemented.
-   **`mut` marks a parameter the function may modify**, both in the declaration and in the call, so the change is visible where it happens:

    ```ez
    function addItem(mut list: int[], item: int) { ... }

    addItem(mut items, 2)
    ```

    An `imut` variable can never be passed as `mut`, and a `const` neither (the function could replace the value). Parameters without `mut` (and `self`) are read-only. Implemented; see [Functions](functions.md#mut-parameters).
-   **Move-only types**, including user types: values that cannot be copied (resources). Assigning moves them, and using the previous variable is a compile error.
-   **Only move-only types can have a `drop`** method, called automatically when the value is dropped. A copyable type with `drop` would run it once per copy.
-   **Move-only is implicit**: a type is move-only when it declares a `drop` method, or when it has a field of a move-only type (a type holding a `File` cannot be copied either). There is no separate marker. Not implemented yet: it comes with value semantics in the interpreter.
-   **`for` and `match` variables are `imut`**: `for x in items` gives an imut `x`; `for mut x in items` allows changing it (a copy: the list does not change). Variables bound by `match` patterns are always imut. Implemented.
-   **Intentionally shared structures** (graphs, caches) use indices into a list ("arenas"). An explicit shared type may come later, only if arenas prove insufficient.

Move-only types and `drop` are not implemented yet (phase 1).

### Standard library: file modes and readable errors

-   `file.open(path, mode)` takes a sum type, so an invalid mode does not compile:

    ```ez
    type Mode = { Read  Write  Append }

    f = file.open('notes.txt', Read)?
    ```

-   `print(error)` (and string interpolation) shows a human-readable message for standard library errors, e.g. `file 'notes.txt' not found` instead of `NotFound('notes.txt')`. The same text is available as `error.display()`.

Both are implemented.

### Block scope

A variable belongs to the block where it is declared (the file, a function, or any `{ ... }` block) and is visible in the blocks inside it. Assigning to a name that exists in an outer block changes that variable; otherwise the assignment declares a new variable in the current block.

### Imports apply to the whole block

`import` statements are processed before the rest of their block, so types and functions declared before an `import` can use it (like ES modules).

### `MathError.Indeterminate` stays

It is kept for future functions, even though no current function returns it (`0 ** 0` is `1`).

### No `interface`

`type` covers what interfaces did: shape types describe the fields of a value. Shape types are **nominal**: two types with the same fields are different types (`Celsius` and `Fahrenheit` do not mix), and an object literal is accepted by any type whose fields it matches.

`interface` stays a reserved word, as a precaution, in case behavior contracts (e.g. constraints on generic types) are ever needed. Whether generics need constraints is still an [open decision](#open-decisions); without them, behavior is passed as functions (e.g. `sort(items, (a, b) => a.age < b.age)`).

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

// int(x) and float(x) with a string
type ParseError = {
    InvalidNumber(text: string)
    OutOfRange(text: string)
}

// math
type MathError = {
    Complex // the result exists only in the complex numbers: sqrt(-1), log(-1)
    Indeterminate // indeterminate form (no current function returns it)
    Infinite // the result is infinite (a pole): pow(0, -1), log(0)
}

// regex.compile
type RegexError = {
    InvalidPattern(pattern: string, message: string)
}
```

`time` has no error type. `input()` keeps returning `Option[string]`.

`regex.compile(pattern)` returns `Result[Regex, RegexError]`, and the methods of `Regex` (`test`, `find`, `findAll`, `replace`, `replaceAll`, `split`) cannot fail. More variants may be added. Implemented.

### No NaN, no infinity

`float` is always a **finite real number**:

-   There is no NaN. Math functions whose result is not a real number return `Err(Complex)`, `Err(Indeterminate)` or `Err(Infinite)`.
-   There is no infinity. Float overflow (`1e308 * 10`) panics, like integer overflow.
-   Float literals that are too large (`1e400`) are compile-time errors.
-   `float('1e400')` returns `Err(OutOfRange(...))`.
-   `MathError` only describes **domain** problems (the result does not exist in the reals). Overflow is a **representation** limit (the result exists but does not fit), so it does not belong to `MathError`.

### Representation limits panic, domain problems are `MathError`

One rule decides how numeric failures are reported:

| Problem | Meaning | Handling | Examples |
| --- | --- | --- | --- |
| **Representation limit** | The result exists but does not fit in an `int` or a `float` | panic | `9223372036854775807 + 1`, `1e308 * 10`, `math.pow(10, 400)`, `math.round(1e300)` |
| **Domain problem** | The result does not exist in the real numbers | `Err(MathError)` | `math.sqrt(-1)` → `Complex`, `math.pow(0, -1)` → `Infinite` |

`0 ** 0` and `math.pow(0, 0)` are `1`: that is the value of the expression, as in algebra and combinatorics. The indeterminate form 0⁰ only concerns limits.

Operators cannot return a `Result`, so they always panic, including on domain problems such as division by zero.

In `math` functions, an infinite result (a pole, such as `math.pow(0, -1)`, which is `1 / 0`) is `Err(Infinite)`. It is not an overflow: a larger type would fit an overflowing result, but no type can represent an infinite one. `Infinite` has no sign field, because the sign is always determined by the function (`pow(0, -y)` is always `+∞`, a future `log(0)` always `-∞`).

Float literals support scientific notation (`6.022e23`).

### Division

Like Go and Rust: `int / int` gives an `int` truncated toward zero, a division with a `float` gives a `float`, and `%` has the sign of the dividend. `//` is always a comment.

---

## What the language needs to support the compiler

A compiler is a large program, with many data structures and many operations that can fail. These features are needed before writing `ezc` in ez, in order of importance:

1.  **User-defined generics**: `type Map[K, V]`, `function find[T](...)`, using the same bracket syntax as `Option[T]` (constraints are an [open decision](#open-decisions)).
2.  **Collections**:
    -   lists with `push`, `pop`, `insert` and slicing;
    -   maps with any comparable key type (index signatures only allow `string` keys today);
    -   sets.
3.  **Text processing**:
    -   a `char` (or `byte`) type and conversion to and from character codes;
    -   `substring`/slicing, `startsWith`, `indexOf` and a string builder;
    -   efficient indexing (`s[i]` is O(n) today).
4.  **Error propagation**: the `?` operator (implemented).
5.  **Methods on types** (`Type.method()`, `mut self`): implemented.
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

-   **`int` to `float` coercion** is inconsistent today (an `int` stored in a `float` field of an object stays an `int`). With complete types, every conversion is explicit in the generated code.
-   **Memory management**: ownership with value semantics, copy-on-write and drop (see [Memory management](#memory-management-ownership-value-semantics-and-drop)).

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

1.  **Generics**: `function first[T](items: T[]): Option[T]` and `type Pair[A, B] = { ... }`; whether type parameters need constraints is still being considered. Left for last, as it is the most complex.
2.  **Phase 1 APIs**: collections (`push`, `pop`, slicing, maps with any key type, sets), text (a character type, slicing, a string builder), system (command-line arguments, exit code, environment variables, processes) and FFI (`extern function`). Depends on generics.
3.  **More `RegexError` variants**, if `InvalidPattern` proves too coarse.

Decided:

-   **Mutating methods**: `mut self`, with no marker at the call (see [Methods](#methods)).
-   **Functions without `self`**: associated functions, `Person.create('John')`.
-   **Move-only types**: implicit, from `drop` (see [Value semantics details](#value-semantics-details)).
-   **Display**: `display()` is readable (a `display(self): string` method of the type, when declared) and is what `print` and interpolation use; `debug()` shows the structure (see [Custom Types](custom-types.md#display)).
-   **`regex.compile`**, with `RegexError = { InvalidPattern(pattern, message) }` (see [Typed errors in the standard library](#typed-errors-in-the-standard-library)).
-   **`for` and `match` variables are imut**, with `for mut` to opt out.
