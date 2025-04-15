# 📘 ez

**ez** is a strongly and statically typed programming language designed to be simple, readable, and powerful. Written in C, it features a syntax inspired by Python, TypeScript, and JavaScript — but with its own identity.

> Below is the official language specification, including its syntax, features, and future roadmap.

---

## 📖 Table of Contents

-   [✨ Why use it?](#-why-use-it)
-   [✅ Primitive Types](#-primitive-types)
-   [📎 Conventions](#-conventions)
-   [Variable Declaration](#-variable-declaration)
-   [🧠 Comments](#-comments)
-   [🔀 Control Structures](#-control-structures)
-   [🧮 Operators](#-operators)
-   [🔤 String Methods](#-string-methods)
-   [🧰 Functions](#-functions)
-   [🔍 String Interpolation](#-string-interpolation)
-   [📚 Data Structures](#-data-structures)
-   [🧪 Usage Examples](#-usage-examples)
-   [🚧 Language Roadmap](#-language-roadmap)
-   [🔮 Planned Features](#-planned-features)
-   [💭 Ideas Under Discussion](#-ideas-under-discussion)
-   [🤝 Contributions](#-contributions)

---

## ✨ Why use it?

-   **Strong and static typing**
-   Clean and familiar syntax
-   Written in C, focused on **readability and performance**
-   Great for learning about language construction
-   Growing project and **open to contributions**

## ✅ Primitive Types

-   `int`: integer numbers
-   `float`: floating-point numbers
-   `boolean`: logical values (`true`, `false`)
-   `string`: character sequences
-   `null`: absence of value
-   `undefined`: undefined value

❌ There is no `any` type.

---

## 📎 Conventions

-   The language uses `camelCase` for variable and function names.
-   Parentheses are required in conditions and function calls.

---

## Variable Declaration

```ez
name: string = 'Maria'
age: int = 23
isStudent: boolean = true
values: int[] = [1, 2, 3]
```

---

## 🧠 Comments

```ez
// Single-line comment

/*
Multi-line
comment
*/
```

---

## 🔀 Control Structures

### Conditional

```ez
if (age > 18) {
  print('Adult')
} else {
  print('Minor')
}
```

### `for` Loop

```ez
for i in range(0, 10) {
  print(i)
}

// or simply
for i in range(10) {
  print(i)
}
```

### `while` Loop

```ez
while (count < 5) {
  print(count)

  count += 1
}
```

### Switch-case

```ez
switch (option) {
  case 0:
    print('Default option')
    fallthrough
  case 1:
    print('Option 1')
  case 2:
    print('Option 2')
  default:
    print('Invalid option')
}
```

---

## 🧮 Operators

| Operator | Description                      |
| -------- | -------------------------------- |
| `+`      | Addition or concat               |
| `-`      | Subtraction                      |
| `*`      | Multiplication                   |
| `/`      | Division (floating-point)        |
| `//`     | Integer Division (floor division)|
| `**`     | Exponentiation                   |
| `%`      | Modulus (remainder)              |
| `==`     | Equality                         |
| `!=`     | Inequality                       |
| `>`      | Greater than                     |
| `<`      | Less than                        |
| `>=`     | Greater than or equal            |
| `<=`     | Less than or equal               |
| `and`    | Logical AND                      |
| `or`     | Logical OR                       |
| `not`    | Logical NOT                      |

---

## 🔤 String Methods

| Method                 | Description                           | Example                            |
| ---------------------- | ------------------------------------- | ---------------------------------- |
| `str.trim()`           | Removes whitespace from start and end | `"  hi  ".trim() → "hi"`           |
| `str.upper()`          | Converts to uppercase                 | `"hello".upper() → "HELLO"`        |
| `str.lower()`          | Converts to lowercase                 | `"HELLO".lower() → "hello"`        |
| `str.isSpace()`        | Checks if only spaces                 | `"   ".isSpace() → true`           |
| `str.isNumeric()`      | Checks if numeric only                | `"123".isNumeric() → true`         |
| `str.isAlpha()`        | Checks if alphabetic only             | `"abc".isAlpha() → true`           |
| `str.isAlphaNumeric()` | Checks if alphanumeric                | `"abc123".isAlphaNumeric() → true` |
| `str.isUpper()`        | Checks if all uppercase               | `"ABC".isUpper() → true`           |
| `str.isLower()`        | Checks if all lowercase               | `"abc".isLower() → true`           |
| `str.isTitle()`        | Checks if title case                  | `"Hello World".isTitle() → true`   |
| `str.split(sep)`       | Splits by separator                   | `"a b".split(" ") → ["a", "b"]`    |

---

## 🧰 Functions

```ez
function sum(a: int, b: int): int {
  return a + b
}
```

---

## 🔍 String Interpolation

```ez
print(`Hello, ${name}!`)
print(`${number:02d} x ${i:02d} = ${result:02d}`)
```

📌 `:02d` formats numbers with two digits, padding with zero on the left.

---

## 📚 Data Structures

```ez
// List
numbers: int[] = [1, 2, 3]

// Matrix
matrix: int[][] = [[1, 2], [3, 4]]

// Dictionary
type Person = {
  name: string
  age: int
}

person: Person = {
  name: 'John',
  age: 20
}
```

---

## 🧪 Usage Examples

### Multiplication Table

```ez
function multiplicationTable(num: int): int[] {
  let result: int[] = []

  for i in range(11) {
    result += num * i
  }

  return result
}

number: int = int(input('Enter a number: '))
results: int[] = multiplicationTable(number)

for result, i in results {
  print(`${number:02d} x ${i:02d} = ${result:02d}`)
}
```

### Formatted Name

```ez
fullName: string = input('Enter your full name: ').trim()
nameParts: string[] = fullName.split(' ')

firstName: string = nameParts[0]
lastName: string = nameParts[nameParts.length() - 1]

if (firstName != lastName) {
  shortName: string = `${firstName} ${lastName}`
} else {
  shortName: string = firstName
}

print(`Hello, ${shortName}`)
```

📌 `.split(' ')` splits by spaces and `.length()` returns the number of items in the list.

---

## 🚧 Language Roadmap

This language is still under development and new features are being planned. Here are some considerations for future versions:

### 🔮 Planned Features

-   **Anonymous functions (lambda)**: inline functions for map, filter, etc.
-   **Async/Await**: native asynchronous execution, with support for concurrency and non-blocking IO.
-   **Match Expressions**: more powerful extension of `switch`, inspired by Rust and Python 3.10+.
-   **Generic types**: allow generic and reusable functions and data structures.
-   **Packages and modules**: code organization and import structure across files.
-   **Robust error system**: with exceptions and `Result`/`Option` types like Rust.

### 💭 Ideas Under Discussion

-   **Macro system**: enable compile-time expansion with custom constructs.
-   **Customizable operators**: allow defining or overloading operators in user-defined types.
-   **Partial type inference**: reduce redundancy when the type is obvious, while keeping static typing.
-   **DOM interaction**: enable direct manipulation of the DOM like in JavaScript, with clarity and safety.
-   **Native HTTP support**: simple and robust built-in API for HTTP requests without external libraries, inspired by Go.

### 🤝 Contributions

We welcome ideas, suggestions, and contributions! If you enjoy programming languages, compilers, and fun syntax discussions, you're more than welcome. Open an issue, share your thoughts, or submit a PR.
