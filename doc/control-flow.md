# Control Flow

## Conditional

### if / else

Executes a block of code conditionally.

```ez
if (age > 18) {
    print('Adult')
} else {
    print('Minor')
}
```

## Loop

### `for` Loop

The `for` loop is used to repeat a block of code over a range of numbers.

#### How `range` Works

The `range(start, end, step)` function generates integers from `start` up to (but not including) `end`.

```ez
range(0, 5) // Produces: 0, 1, 2, 3, 4
```

#### Basic Sintax

```ez
for i in range(0, 10) {
    print(i)
}

// or simply

for i in range(10) {
    print(i)
}
```

This prints numbers from 0 to 9.

#### Loop through a list

```ez
names: string[] = ['Anna', 'Claire', 'Leon']

// By index
for i in range(0, names.length()) {
    print(names[i])
}

// Direct iteration
for name in names {
    print(name)
}
```

### `while` Loop

Repeats a block of code while a condition is true.

```ez
while (count < 5) {
    print(count)

    count += 1
}
```

## Match Expression

The `match` expression in **ez** allows you to **pattern match** on values, especially useful with sum types (`enum`s). It is a powerful control flow construct that forces you to handle **all possible cases**, ensuring safer and more expressive code.

### Basic Syntax

```ez
match (expression) {
  Pattern1 => expression1
  Pattern2 => expression2
  ...
}
```

-   Each Pattern can match a variant of a sum type or a literal value.
-   All cases must be covered; otherwise, the compiler raises an error.
-   `match` is an expression, meaning it evaluates to a value.

### Example with Sum Types

Given the Option[T] enum:

```ez
enum Option[T] {
  Some(value: T)
  None
}
```

You can match on it like this:

```ez
name: Option[string] = Some('Alice')

greeting = match (name) {
  Some(value) => `Hello, ${value}!`
  None => 'Hello, guest!'
}

print(greeting)
```

### Destructuring

Patterns can destructure variants with data fields:

```ez
enum Result[T, E] {
  Ok(value: T)
  Err(error: E)
}

res: Result[int, string] = Ok(42)

match (res) {
  Ok(val) => print(`Success with value: ${val}`)
  Err(msg) => print(`Error occurred: ${msg}`)
}
```

### Exhaustiveness

Every possible variant must be matched:

```ez
enum Color {
  Red
  Green
  Blue
}

color: Color = Red

match (color) {
  Red => print('Red')
  Green => print('Green')
  Blue => print('Blue')
  // Missing any variant here causes a compile error
}
```

### Use as Expression

`match` returns the value of the executed branch:

```ez
result = match (someOption) {
  Some(v) => v
  None => 0
}
```
