# 📘 ez

**ez** is a strongly and statically typed programming language designed to be simple, readable, and powerful. Written in C, it features a syntax inspired by Python, TypeScript, and JavaScript — but with its own identity.

> Below is the official language specification, including its syntax, features, and future roadmap.

---

## 🧭 Table of Contents

-   [💡 Philosophy](#-philosophy)
-   [✅ Primitive Types](#-primitive-types)
-   [📖 Variable Types](#-variable-types)
-   [🧱 Custom Types](#custom-types)
-   [📎 Conventions](#-conventions)
-   [Variable Declaration](#-variable-declaration)
-   [🧠 Comments](#-comments)
-   [🔀 Control Structures](#-control-structures)
-   [🧮 Operators](#-operators)
-   [🔤 String Methods](#-string-methods)
-   [🧰 Functions](#-functions)
-   [🔍 String Interpolation](#-string-interpolation)
-   [📚 Data Structures](#-data-structures)
-   [Modules and Imports](#-modules-and-imports)
-   [🧪 Usage Examples](#-usage-examples)
-   [🚧 Language Roadmap](#-language-roadmap)
-   [🔮 Planned Features](#-planned-features)
-   [💭 Ideas Under Discussion](#-ideas-under-discussion)
-   [🤝 Contributions](#-contributions)

---

💡 Philosophy
The ez programming language is built on the following principles:

-   **Clarity over cleverness**: Code should be easy to read and understand
-   **Strong, static typing**: For safety and predictability
-   **Productivity-focused**: Clean syntax, modern features, low friction
-   **Balance between performance and simplicity**
-   **Beginner-friendly**: Especially for those coming from Python, TypeScript, or JavaScript
-   **Built-in utilities**: String methods, modern data structures, planned async/await support, and more

The goal is to make programming in ez natural, expressive, and joyful — whether you're a beginner or an experienced developer

---

## ✅ Primitive Types

-   `int`: integer numbers
-   `float`: floating-point numbers
-   `boolean`: logical values (`true`, `false`)
-   `string`: character sequences
-   `null`: absence of value
-   `undefined`: undefined value

❌ There is no `any` type.

---

## 📖 Variable Types

The `ez` language provides two main variable types that control mutability and scope:

### `var`

-   **Mutability**: Mutable
-   **Scope**: Global (by default)
-   **Description**: The `var` type is the default in `ez`. You don't need to specify it explicitly — any variable not declared with `const` is considered a `var`. `var` variables can be changed at any time. Their scope is global by default, unless declared inside a function, in which case they become local to that function.

**Example**:

```ez
globalVar = 1 // Global variable

function test() {
    var localVar = 2 // Local variable

    print(globalVar) // 1 (accesses the global variable)
    print(localVar) // 2 (accesses the local variable within the function)
}

test()

print(globalVar) // 1 (global variable accessible outside the function)
print(localVar) // Error! 'localVar' is not accessible outside the function
```

### `const`

-   **Mutability**: Immutable
-   **Scope**: Global or Local (like var)
-   **Description**: The `const` type is used to create variables that cannot be changed after their initial assignment. Attempting to modify a `const` variable will result in an error. Its scope depends on where it's declared — global or local.

**Example**:

```ez
const pi = 3.14159
pi = 3.14 // Error! Cannot modify a 'const' value
```

The `const` keyword defines an immutable variable — it cannot be reassigned.
However, if the constant is a reference type (such as an object or array), its internal contents can be mutated:

```ez
type Person = {
  name: string
  age: int
}

const person: Person = {
  name: "Ana",
  age: 30
}

person.age = 31 // Change the value of person age
person = {} // Error! Cannot modify a 'const' value
```

---

## 🧱 Custom Types

You can define your own types using the type keyword. This makes it easier to organize and reuse complex structures:

```ez
type Person = {
  name: string
  age: int
  hobbies: string[]
}

person: Person = {
  name: "Lucas"
  age: 25
  hobbies: ["coding", "music"]
}
```

You can also use custom types in functions for type checking:

```ez
function greeting(person: Person): string {
  print(`Hello, ${person.name}!`)
}
```

---

## 📎 Conventions

-   The language uses `camelCase` for variable and function names.
-   Parentheses are required in conditions and function calls.

---

## Variable Declaration

```ez
const pi: float = 3.14
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

// Looping through a list of values
fruits: string[] = ["apple", "banana", "cherry"]

for fruit in fruits {
  print(fruit)
}

// Looping through a dictionary (Map)
type Person = {
  name: string
  age: int
}

person: Person = {
  name: 'John',
  age: 20
}

for key in person {
  print(`${key}: ${person[key]}`)
}

// or (preferred)

for key, value in person {
  print(`${key}: ${value}`)
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

### Arithmetic Operators

| Operator | Description                       |
| -------- | --------------------------------- |
| `+`      | Addition or concat                |
| `-`      | Subtraction                       |
| `*`      | Multiplication                    |
| `/`      | Division (floating-point)         |
| `//`     | Integer Division (floor division) |
| `**`     | Exponentiation                    |
| `%`      | Modulus (remainder)               |

### Comparison Operators

| Operator | Description                                                |
| -------- | ---------------------------------------------------------- |
| `==`     | Equality                                                   |
| `!=`     | Inequality                                                 |
| `>`      | Greater than                                               |
| `<`      | Less than                                                  |
| `>=`     | Greater than or equal                                      |
| `<=`     | Less than or equal                                         |
| `is`     | Identity comparison                                        |
| `in`     | Membership check (checks if an element is in a collection) |

### Logical Operators

| Operator | Description |
| -------- | ----------- |
| `and`    | Logical AND |
| `or`     | Logical OR  |
| `not`    | Logical NOT |

### Assignment Operators

| Operator | Description                                |
| -------- | ------------------------------------------ |
| `=`      | Assignment (assigns a value to a variable) |
| `+=`     | Add and assign                             |
| `-=`     | Subtract and assign                        |
| `*=`     | Multiply and assign                        |
| `/=`     | Divide and assign                          |
| `//=`    | Integer Divide and assign                  |
| `**=`    | Exponentiate and assign                    |
| `%=`     | Modulus and assign                         |

### String Operators

| Operator | Description                                  |
| -------- | -------------------------------------------- |
| `+`      | Concatenation (joins strings)                |
| `*`      | Repetition (repeats a string multiple times) |

---

## 🔤 String Methods

| Method                  | Description                              | Example                                 |
| ----------------------- | ---------------------------------------- | --------------------------------------- |
| `str.trim()`            | Removes whitespace from start and end    | `"  hi  ".trim() → "hi"`                |
| `str.upper()`           | Converts to uppercase                    | `"hello".upper() → "HELLO"`             |
| `str.lower()`           | Converts to lowercase                    | `"HELLO".lower() → "hello"`             |
| `str.isSpace()`         | Checks if only spaces                    | `"   ".isSpace() → true`                |
| `str.isNumeric()`       | Checks if numeric only                   | `"123".isNumeric() → true`              |
| `str.isAlpha()`         | Checks if alphabetic only                | `"abc".isAlpha() → true`                |
| `str.isAlphaNumeric()`  | Checks if alphanumeric                   | `"abc123".isAlphaNumeric() → true`      |
| `str.isUpper()`         | Checks if all uppercase                  | `"ABC".isUpper() → true`                |
| `str.isLower()`         | Checks if all lowercase                  | `"abc".isLower() → true`                |
| `str.isTitle()`         | Checks if title case                     | `"Hello World".isTitle() → true`        |
| `str.split(sep)`        | Splits by separator                      | `"a b".split(" ") → ["a", "b"]`         |
| `str.replace(old, new)` | Replaces occurrences of `old` with `new` | `"hello".replace("hello", "hi") → "hi"` |

---

## 🧰 Functions

```ez
function sum(a: int, b: int): int {
  return a + b
}

print(sum(2, 2)) // 4

// Anonymous function (example of inline usage)

const double = (x: int): int => {
  return x * 2
}

print(double(10)) // 20
```

---

## 🔍 String Interpolation

```ez
print(`Hello, ${name}!`)
print(`${number:02d} x ${i:02d} = ${result:02d}`)
print(`2 + 2 = ${2 + 2}`)
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

## Modules and Imports

`ez` supports code modularization. Modules allow you to organize your code in separate files, and you can import functions and variables as needed.

### Module Structure:

```ez
// file: math.ez
function add(a: int, b: int): int {
  return a + b
}

function subtract(a: int, b: int): int {
  return a - b
}
```

### Using Modules:

```ez
// file: main.ez
import math from "math.ez"

result: int = math.add(5, 10)
print(result) // 15
```

In the example above, we're importing the `math` module that contains the functions `add` and `subtract`, which we can call in the main code.

---

## 🧪 Usage Examples

### Multiplication Table

```ez
function multiplicationTable(num: int): int[] {
  result: int[] = []

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
