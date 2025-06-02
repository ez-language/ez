# Regex

The `regex` module provides support for regular expressions for searching, validation, and string manipulation.

## Import

```ez
import regex
```

## Functions

### match

Checks if the string fully matches the regex pattern.

```ez
regex.match(pattern: string, input: string) -> bool
```

**Example:**

```ez
regex.match('^[a-z]+$', 'hello') // true
regex.match('^[a-z]+$', '123') // false
```

---

### search

Finds the first substring that matches the regex pattern.

```ez
regex.search(pattern: string, input: string) -> Option<Match>
```

Returns Some(Match) if found, or None if no match is found.

**Example:**

```ez
result = regex.search('[0-9]+', 'Order number: 12345')

if result is Some {
	print(result.value.value) // 12345
}
```

---

### findAll

Finds all matches in the string.

```ez
regex.findAll(pattern: string, input: string) -> array<Match>
```

**Example:**

```ez
results = regex.findAll('[a-z]+', 'abc 123 def')

for word in results {
	print(word.value)
}
// abc
// def
```

---

### replace

Replaces all matches of the pattern in the string with a replacement string.

```ez
regex.replace(pattern: string, replacement: string, input: string) -> string
```

**Example:**

```ez
result = regex.replace('[0-9]+', '#', 'abc123def456')

print(result) // abc#def#
```

---

## Type: Match

Represents a found match.

```ez
Match {
	value: string
	start: int
	end: int
}
```

**Example:**

```ez
result = regex.search('abc', '123abc456')

if result is Some {
	match = result.value

	print(match.value) // abc
	print(match.start) // 3
	print(match.end) // 6
}
```

---

## Notes

-   The regex syntax follows the PCRE (Perl Compatible Regular Expressions) standard.
-   To learn more about regex syntax, visit: [https://regex101.com/](https://regex101.com/)
