# Getting Started

This guide shows how to build ez, write a first program and run it.

## Requirements

ez is implemented in Rust. To build it, install [Rust](https://www.rust-lang.org/tools/install) (which includes `cargo`).

## Building

Clone the repository and build it in release mode:

```sh
git clone https://github.com/ez-language/ez.git
cd ez
cargo build --release
```

The executable is created at `target/release/ez`. To call it as `ez` from anywhere, add that directory to your `PATH`, or install it with:

```sh
cargo install --path .
```

## Your First Program

Create a file named `hello.ez`:

```ez
function greet(name: string): string {
    return `Hello, ${name}!`
}

print(greet('World'))
```

Run it:

```sh
ez run hello.ez
```

```
Hello, World!
```

## Checking a Program

Every program is checked before it runs: syntax, types, `match` arms that are not handled, assignments to `const` or `imut` values and so on. A program with an error does not run at all.

To only check a program, without running it:

```sh
ez check hello.ez
```

It prints nothing when the program is correct. If `hello.ez` called `greet(42)`, both `ez run` and `ez check` would report:

```
type error: hello.ez:5: argument 1 must be string, found int
```

The message gives the file and the line of the error.

## Commands

| Command | Description |
| --- | --- |
| `ez run <file.ez>` | Checks and runs a program |
| `ez check <file.ez>` | Only checks syntax and types |
| `ez help` | Shows the available commands |

Programs currently run on an interpreter. Compiling to native binaries (`ez build`) is planned; see the [compiler plan](compiler-plan.md).

## Examples

The [`examples`](../examples) folder has small programs to try:

```sh
ez run examples/functions.ez
ez run examples/shapes.ez
```

## Running the Tests

If you want to contribute, run the test suite with:

```sh
cargo test
```

See the [contributing guide](../CONTRIBUTING.md) for details.

## Next Steps

-   [Variable Types](variable-types.md): `var`, `const` and `imut`
-   [Primitive Types](primitive-types.md) and [Composite Types](composite-types.md)
-   [Functions](functions.md)
-   [Custom Types](custom-types.md) and [Sum Types](sum-types.md)
-   [Control Flow](control-flow.md)
-   [Error Handling](error-handling.md)
-   [Modules and Imports](modules-and-imports.md)
-   [Standard Library](stdlib.md)
-   The full [documentation](README.md)
