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

`if` can also be used as an expression.

```ez
const label: string = if (age > 18) { 'Adult' } else { 'Minor' }

print(label)
```

`ez` does not have else if. For multiple branches, use a match
expression with guard conditions.

## Loop

### `for` Loop

The `for` loop is used to repeat a block of code over a range of numbers.

#### How `range` Works

The `range(start, end, step)` function generates integers from `start` up to (but not including) `end`.

```ez
range(0, 5) // Produces: 0, 1, 2, 3, 4
```

#### Basic Syntax

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

### `break` and `continue`

-	`break`: exits the current loop immediately.

```ez
for i in range(0, 10) {
    if (i == 5) {
        break
    }

    print(i)
}
// Prints: 0, 1, 2, 3, 4

break // Error! break can only be used inside a loop
```

-	`continue`: skips to the next iteration.

```ez
for i in range(0, 10) {
    if (i % 2 == 0) {
        continue
    }

    print(i)
}
// Prints: 1, 3, 5, 7, 9

continue // Error! continue can only be used inside a loop
```

## Match Expression

The `match` expression allows you to compare a value against multiple patterns.

```ez
match (result) {
	Ok(value) => print(`Value: ${value}`)
	Err(error) => print(`Error: ${error}`)
}
```

Each arm follows the pattern:

```ez
Pattern => Expression
```

Arms are matched in order, and the first one that matches will be executed. Pattern matching supports destructuring and variant matching for sum types.

```ez
match (user) {
	Admin(name) => print(`Admin: ${name}`)
	Guest => print('Guest user')
}
```

The `match` expression is exhaustive, and must handle all possible variants unless a wildcard (`_`) is used:

```ez
match (option) {
	Some(value) => print(value)
	_ => print('No value')
}
```

A `match` expression can also be assigned to a variable.

```ez
const label: string = match (result) {
    Ok(value) => `Value: ${value}`
    Err(error) => `Error: ${error}`
}
```

All arms must evaluate to the same type, which becomes the type of the expression.

```ez
const label: int = match (result) {
    Ok(value) => `Value: ${value}` // Error! string is not int
    Err(error) => `Error: ${error}`
}
```

`match` expressions with guard conditions replaces both `else if` chains and `switch` / `case` statements.
Guards are evaluated in order, and the first one that matches is used.

```ez
age: int = 14

const label: string = match (age) {
    x if (x > 18) => 'Adult'
    x if (x > 12) => 'Teen'
    _             => 'Minor'
}

print(label) // Teen
```
