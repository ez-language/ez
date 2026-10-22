# Regex

The `regex` module provides regular expressions for searching, validation, and string manipulation.

## Import

```ez
import regex
```

## Compiling a Pattern

```ez
regex.compile(pattern: string) -> Result[Regex, RegexError]
```

A pattern is compiled once, and the resulting `Regex` is used as many times as needed. Only compiling can fail; the methods of `Regex` cannot.

```ez
type RegexError = {
    InvalidPattern(pattern: string, message: string)
}
```

```ez
match (regex.compile('(abc')) {
    Ok(_) => print('valid')
    Err(error) => print(error) // invalid regular expression '(abc': unclosed group
}

// In a function that returns a Result, `?` is shorter:
function countWords(text: string): Result[int, regex.RegexError] {
    const word = regex.compile('[a-z]+')?
    return Ok(word.findAll(text).length())
}
```

## Methods of `Regex`

| Method | Signature | Description |
| --- | --- | --- |
| `test` | `test(input: string) -> boolean` | Whether the pattern matches anywhere in `input` (use `^` and `$` to match the whole string) |
| `find` | `find(input: string) -> Option[Match]` | The first match |
| `findAll` | `findAll(input: string) -> Match[]` | Every match |
| `replace` | `replace(input: string, repl: string) -> string` | Replaces the first match with `repl` |
| `replaceAll` | `replaceAll(input: string, repl: string) -> string` | Replaces every match with `repl` |
| `split` | `split(input: string) -> string[]` | Splits `input` at every match |

In `repl`, `$1` (or `${1}`) is the text of the first group, `${name}` the text of a named group `(?P<name>...)`, and `$$` a literal `$`.

**Example:**

```ez
import regex

const digits = regex.compile('[0-9]+')?

print(digits.test('abc123')) // true
print(digits.test('abc')) // false

match (digits.find('Order number: 12345')) {
    Some(found) => print(found.value) // 12345
    None => print('no number')
}

for found in digits.findAll('a1b22c333') {
    print(found.value)
}
// 1
// 22
// 333

print(digits.replace('a1b2', '#')) // a#b2
print(digits.replaceAll('a1b2', '#')) // a#b#
print(digits.split('a1b22c')) // ['a', 'b', 'c']

const date = regex.compile('([0-9]{4})-([0-9]{2})')?
print(date.replaceAll('2025-10', '$2/$1')) // 10/2025
```

## Type: Match

Represents a found match.

```ez
Match {
	value: string
	start: int
	end: int
}
```

-   `start` and `end` are character positions (`end` is exclusive).

## Notes

-   The regex syntax is Perl-style (character classes, groups, quantifiers, anchors). Look-around and backreferences are not supported.
-   To learn more about regex syntax, visit: [https://regex101.com/](https://regex101.com/)
