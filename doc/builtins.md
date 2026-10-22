# Built-in Functions

These functions are available in every file, without `import`.

| Function | Description |
| --- | --- |
| `print(values...)` | Shows the values, separated by spaces, followed by a new line |
| `input(prompt)` | Reads a line typed by the user |
| `range(...)` | A list of consecutive integers |
| `int(x)`, `float(x)`, `string(x)` | Conversions |
| `panic(message)` | Stops the program |

## `print`

Shows its arguments, separated by spaces. Each value is shown as its [`display()`](custom-types.md#display) text: strings without quotes, standard library errors as a readable message.

```ez
name: string = 'John'

print(`Hello, ${name}!`)   // Hello, John!
print('total:', 3, [1, 2]) // total: 3 [1, 2]
```

## `input`

Shows the optional `prompt` and reads a line. It returns an `Option[string]`: `None` when there is nothing more to read (the end of the input).

```ez
name: Option[string] = input("What's your name? ")

match (name) {
	Some(line) => print(`Hello, ${line}!`)
	None => print('No input')
}
```

## `range`

`range(end)`, `range(start, end)` or `range(start, end, step)`: the integers from `start` (default `0`) up to, but not including, `end`. A negative `step` counts down; a `step` of `0` panics.

```ez
range(5)          // [0, 1, 2, 3, 4]
range(2, 5)       // [2, 3, 4]
range(10, 0, -3)  // [10, 7, 4, 1]
```

## Conversions

| Function    | Description             | Example           |
| ----------- | ----------------------- | ----------------- |
| `int(x)`    | Converts to an integer  | `int(3.9) → 3`    |
| `float(x)`  | Converts to a float     | `float(2) → 2.0`  |
| `string(x)` | Converts to a string    | `string(0) → '0'` |

`int` truncates toward zero. A float too large for an `int` panics (see [Primitive Types](primitive-types.md)).

Converting a `string` can fail, so `int(x)` and `float(x)` return a `Result[int, ParseError]` (or `Result[float, ParseError]`) when `x` is a string:

```ez
type ParseError = {
	InvalidNumber(text: string)   // not a number: 'abc', 'nan', 'inf'
	OutOfRange(text: string)      // too large: '99999999999999999999', '1e400'
}
```

```ez
int(3.9)   // 3
int('0')   // Ok(0)
int('abc') // Err(InvalidNumber('abc'))

match (int('42')) {
	Ok(value) => print(value + 1) // 43
	Err(InvalidNumber(text)) => print(`not a number: ${text}`)
	Err(OutOfRange(text)) => print(`too large: ${text}`)
}
```

`string(x)` gives the same text as `print(x)`.

## `panic`

Stops the program with an error message. Use it for states that should be impossible (bugs), not for expected failures, which are `Result` values.

```ez
function check(age: int): int {
	if (age >= 0) { return age }

	panic('age cannot be negative')
}
```

A panic cannot be caught: ez has no `try` / `catch`. Index out of range, division by zero and overflow also panic (see [Error Handling](error-handling.md)).
