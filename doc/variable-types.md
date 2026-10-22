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
-   **Description**: The `var` type is the default in `ez`. You don't need to specify it explicitly — any variable not declared with `const` is considered a `var`. `var` variables can be changed at any time. A variable belongs to the block where it is declared: the file (global), a function, or any `{ ... }` block such as the body of an `if` or a loop. It is visible in that block and in the blocks inside it.

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

Blocks inside a function work the same way:

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
-   **Description**: The `const` type is used to create variables that cannot be reassigned after their initial assignment. Attempting to reassign a `const` variable will result in an error.

**Example**:

```ez
const pi = 3.14159
pi = 3.14 // Error! Cannot modify a 'const' value
```

The `const` keyword defines a variable that cannot be reassigned.
However, if its value has contents (an object or a list), they can still be changed:

```ez
type Person = {
	name: string
	age: int
}

const person: Person = {
	name: 'Ana',
	age: 30
}

person.age = 31 // Change the value of person age
person = { name: 'Bia', age: 25 } // Error! Cannot modify a 'const' value
```

## `imut`

-   **Mutability**: Fully immutable: neither the variable nor its contents can change
-   **Scope**: Block (like `var`)
-   **Description**: The `imut` type is used for values that never change. Changing a field, an element or a nested value is an error, as well as reassigning the variable.

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
