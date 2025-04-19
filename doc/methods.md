# Methods

## Conversion Methods

| Method      | Description                                | Example            |
| ----------- | ------------------------------------------ | ------------------ |
| `int(x)`    | convert the value of a variable to integer | `int('0') → 0`     |
| `float(x)`  | convert the value of a variable to float   | `float('0') → 0.0` |
| `string(x)` | convert the value of a variable to string  | `string(0) → '0'`  |

## String Methods

| Method                  | Description                                                          | Example                                 |
| ----------------------- | -------------------------------------------------------------------- | --------------------------------------- |
| `str.length()`          | Returns the number of characters in a string or elements in an array | `'abc'.length() → 3`                    |
| `str.trim()`            | Removes whitespace from start and end                                | `'  hi  '.trim() → 'hi'`                |
| `str.upper()`           | Converts to uppercase                                                | `'hello'.upper() → 'HELLO'`             |
| `str.lower()`           | Converts to lowercase                                                | `'HELLO'.lower() → 'hello'`             |
| `str.isSpace()`         | Checks if only spaces                                                | `'   '.isSpace() → true`                |
| `str.isNumeric()`       | Checks if numeric only                                               | `'123'.isNumeric() → true`              |
| `str.isAlpha()`         | Checks if alphabetic only                                            | `'abc'.isAlpha() → true`                |
| `str.isAlphaNumeric()`  | Checks if alphanumeric                                               | `'abc123'.isAlphaNumeric() → true`      |
| `str.isUpper()`         | Checks if all uppercase                                              | `'ABC'.isUpper() → true`                |
| `str.isLower()`         | Checks if all lowercase                                              | `'abc'.isLower() → true`                |
| `str.isTitle()`         | Checks if title case                                                 | `'Hello World'.isTitle() → true`        |
| `str.split(sep)`        | Splits by separator                                                  | `'a b'.split(' ') → ['a', 'b']`         |
| `str.replace(old, new)` | Replaces occurrences of `old` with `new`                             | `'hello'.replace('hello', 'hi') → 'hi'` |
