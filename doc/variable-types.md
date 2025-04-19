# Variable Types

The `ez` language provides two main variable types that control mutability and scope:

## `var`

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

## `const`

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
  name: 'Ana',
  age: 30
}

person.age = 31 // Change the value of person age
person = {} // Error! Cannot modify a 'const' value
```
