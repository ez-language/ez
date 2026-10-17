# Methods

## Conversion Methods

| Method      | Description                                | Example            |
| ----------- | ------------------------------------------ | ------------------ |
| `int(x)`    | convert the value of a variable to integer | `int(3.9) → 3`     |
| `float(x)`  | convert the value of a variable to float   | `float(2) → 2.0`   |
| `string(x)` | convert the value of a variable to string  | `string(0) → '0'`  |

Converting a `string` can fail, so `int(x)` and `float(x)` return a `Result` when `x` is a string:
`Ok(value)` on success or `Err(message)` otherwise.

Examples:

-  `int(x)`:

```ez
int(3.9) // 3
int('0') // Result[int, string] → Ok(0)
int('abc') // Result[int, string] → Err('not a valid int')

match (int('42')) {
	Ok(value) => print(value + 1) // 43
	Err(error) => print(error)
}
```

## String Methods

| Method                  | Description                                                               | Example                                 |
| ----------------------- | ------------------------------------------------------------------------- | --------------------------------------- |
| `str.length()`          | Returns the number of elements (characters for strings, items for arrays) | `'abc'.length() → 3`                    |
| `str.trim()`            | Removes whitespace from start and end                                     | `'  hi  '.trim() → 'hi'`                |
| `str.upper()`           | Converts to uppercase                                                     | `'hello'.upper() → 'HELLO'`             |
| `str.lower()`           | Converts to lowercase                                                     | `'HELLO'.lower() → 'hello'`             |
| `str.isSpace()`         | Checks if only spaces                                                     | `'   '.isSpace() → true`                |
| `str.isNumeric()`       | Checks if numeric only                                                    | `'123'.isNumeric() → true`              |
| `str.isAlpha()`         | Checks if alphabetic only                                                 | `'abc'.isAlpha() → true`                |
| `str.isAlphaNumeric()`  | Checks if alphanumeric                                                    | `'abc123'.isAlphaNumeric() → true`      |
| `str.isUpper()`         | Checks if all uppercase                                                   | `'ABC'.isUpper() → true`                |
| `str.isLower()`         | Checks if all lowercase                                                   | `'abc'.isLower() → true`                |
| `str.isTitle()`         | Checks if title case                                                      | `'Hello World'.isTitle() → true`        |
| `str.split(sep)`        | Splits by separator                                                       | `'a b'.split(' ') → ['a', 'b']`         |
| `str.replace(old, new)` | Replaces occurrences of `old` with `new`                                  | `'hello'.replace('hello', 'hi') → 'hi'` |
