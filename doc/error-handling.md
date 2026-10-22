# Error Handling

ez has no exceptions and no `try` / `catch`. There are two kinds of failure:

| Kind | Examples | Handling |
| --- | --- | --- |
| **Expected** (recoverable) | a missing file, invalid JSON, a network failure, `int('abc')` | a `Result` value |
| **Bug** (unrecoverable) | index out of range, division by zero, overflow, `panic(...)` | **panic**: the program stops, it cannot be caught |

## `Result`

A function that can fail returns a `Result[T, E]`: `Ok(value)` on success, `Err(error)` on failure. The standard library uses typed errors, such as `FileError` or `ParseError`, so `match` can handle each case:

```ez
match (int('42')) {
	Ok(value) => print(value + 1)
	Err(InvalidNumber(text)) => print(`not a number: ${text}`)
	Err(OutOfRange(text)) => print(`too large: ${text}`)
}
```

Standard library errors have a readable message: printing or interpolating them shows it, and `error.display()` returns it (`error.debug()` gives the structure, `InvalidNumber('abc')`). A `Result` itself keeps its structural form:

```ez
match (int('abc')) {
	Ok(value) => print(value)
	Err(error) => print(error) // 'abc' is not a valid number
}

print(int('abc')) // Err(InvalidNumber('abc'))
```

## The `?` Operator

`?` propagates an error instead of handling it with `match`:

-   on a `Result`, `value?` gives the `Ok` value, or returns the `Err` from the function;
-   on an `Option`, `value?` gives the `Some` value, or returns `None` from the function.

```ez
function parseAge(text: string): Result[int, ParseError] {
	age = int(text)?

	return Ok(age)
}

print(parseAge('42')) // Ok(42)
print(parseAge('x'))  // Err(InvalidNumber('x'))
```

The function must declare its return type, and it must be a `Result` (for `?` on a `Result`) or an `Option` (for `?` on an `Option`).

At the top level of the main file, an `Err` or a `None` stops the program, prints the error and exits with code 1:

```ez
n = int('oops')? // error: main.ez:1: 'oops' is not a valid number
```

### Combining Errors of Different Types

A function can only return one error type. To combine errors from different sources, declare a sum type with one variant for each of them:

```ez
import file
import json

type ConfigError = {
	File(error: file.FileError)
	Json(error: json.JsonError)
}

function readConfig(path: string): Result[json.Json, ConfigError] {
	f = file.open(path, file.Read)?   // a FileError becomes File(...)
	text = f.read()?            // a FileError becomes File(...)
	f.close()

	return Ok(json.parse(text)?) // a JsonError becomes Json(...)
}
```

`?` wraps the error automatically when the function's error type has **exactly one** variant whose only field has the type of the error. Otherwise the conversion is ambiguous or impossible, and it is a compile error:

```ez
type E = {
	A(error: ParseError)
	B(error: ParseError)
}

function f(): Result[int, E] {
	return Ok(int('1')?) // Error! the variants A, B all hold a ParseError
}
```

### `mapErr`

`result.mapErr(f)` converts the error of a `Result` with a function (or a variant with one field), and keeps an `Ok` as it is:

```ez
function parse(text: string): Result[int, string] {
	n = int(text).mapErr((error: ParseError) => `bad number: ${text}`)?

	return Ok(n)
}

print(parse('x')) // Err('bad number: x')
```

## Panics

`panic(message)` stops the program. Use it for states that should be impossible, not for expected failures:

```ez
function check(age: int): int {
	if (age >= 0) { return age }

	panic('age cannot be negative')
}
```
