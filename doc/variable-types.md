# Variable Types

The `ez` language provides three variable types that control mutability:

| Keyword | Reassign the variable | Change its contents |
| ------- | --------------------- | ------------------- |
| `var`   | yes                   | yes                 |
| `const` | no                    | yes                 |
| `imut`  | no                    | no                  |

## `var`

-   **Mutability**: Mutable
-   **Scope**: Block
-   **Description**: `var` is the default: a variable declared without a keyword (`count = 0`) is a `var`. It can be changed at any time. A variable belongs to the block where it is declared: the file (global), a function, or any `{ ... }` block such as the body of an `if` or a loop. It is visible in that block and in the blocks inside it.

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

Other blocks work the same way:

```ez
if (true) {
	message = 'inside' // declared in the if block
}

print(message) // Error! 'message' is not accessible outside the block
```

Assigning to a name that already exists in an outer block changes that variable instead of declaring a new one:

```ez
count = 0

if (true) {
	count = 1 // changes the outer 'count'
}

print(count) // 1
```

## `const`

-   **Mutability**: The variable cannot be reassigned; its contents can change
-   **Scope**: Block (like `var`)
-   **Description**: A `const` variable cannot be reassigned after it is declared.

**Example**:

```ez
const pi = 3.14159
pi = 3.14 // Error! cannot modify a 'const' value ('pi')
```

If its value has contents (an object or a list), they can still be changed:

```ez
type Person = {
	name: string
	age: int
}

const person: Person = {
	name: 'Ana',
	age: 30
}

person.age = 31 // allowed: changes the contents
person = { name: 'Bia', age: 25 } // Error! cannot modify a 'const' value ('person')
```

## `imut`

-   **Mutability**: Fully immutable: neither the variable nor its contents can change
-   **Scope**: Block (like `var`)
-   **Description**: For values that never change. Reassigning the variable is an error, and so is changing a field, an element or a nested value.

**Example**:

```ez
type Person = {
	name: string
	age: int
}

imut person: Person = {
	name: 'Ana',
	age: 30
}

person.age = 31 // Error! cannot modify 'person': it is imut

imut matrix = [[1, 2], [3, 4]]

matrix[0][0] = 5 // Error! cannot modify 'matrix': it is imut
```

## Other Read-Only Variables

Some variables are read-only without a keyword:

-   function parameters, unless they are declared `mut` (see [Functions](functions.md#parameters-are-read-only));
-   the variable of a `for` loop, unless it is written `for mut` (see [Control Flow](control-flow.md#the-loop-variable));
-   the variables bound by a `match` pattern.
