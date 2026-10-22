# Roadmap

This document outlines the planned features and milestones for the ez programming language. The detailed plan for the compiler is in the [compiler plan](compiler-plan.md).

---

## Short-term

-   [x] Define the core syntax
-   [x] Implement the basic type system (primitive and custom types)
-   [x] Built-in functions: `print()`, `input()`
-   [x] String interpolation
-   [x] `for` loop with `range()`
-   [x] Organize and write the initial documentation

---

## Mid-term

-   [x] Type-safe standard library
-   [x] Control flow: `if`, `match`, `while`, `break`, `continue`
-   [x] Methods on custom types (`Type.method()`)
-   [x] Error handling model: `Result`, the `?` operator and panics (no `try` / `catch`)
-   [x] Module system with mandatory `export`
-   [ ] Value semantics with copy-on-write, move-only types and `drop`
-   [ ] User-defined generics
-   [ ] A compiler targeting native binaries (`ez build`, through C)
-   [ ] A self-hosted compiler, written in ez

---

## Long-term

-   [ ] Package manager
-   [ ] Built-in testing framework
-   [ ] WebAssembly (WASM) compilation
-   [ ] Editor support (syntax highlighting, LSP integration)
-   [ ] Runtime optimizations
-   [ ] Production-ready tooling

---

This roadmap is a living document and will evolve as the language and its community grow.