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

`if` can also be used as an expression. It then needs an `else`, and both branches must have the same type:

```ez
const label: string = if (age > 18) { 'Adult' } else { 'Minor' }

print(label)
```

ez has no `else if`. For more than two branches, use a [`match` with guards](#guards).

## Loop

### `for` Loop

The `for` loop runs a block once for each item of a list, each character of a string, or each key of a dictionary.

#### Counting with `range`

`range(start, end)` gives the integers from `start` up to, but not including, `end` (see [`range`](builtins.md#range) for the other forms):

```ez
range(0, 5) // [0, 1, 2, 3, 4]
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

#### Going Through a List

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

#### The Loop Variable

The loop variable is `imut`: assigning to it, or changing its contents, is an error. With `for mut`, it can be changed, but it is a copy: the list does not change.

```ez
for name in names {
    name = name.upper() // Error! cannot modify 'name': the 'for' variable is imut (use 'for mut' to change it)
}

var rows = [[1, 2], [3, 4]]

for mut row in rows {
    row[0] = 0
    print(row) // [0, 2], then [0, 4]
}

print(rows) // [[1, 2], [3, 4]]
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

Each arm has the form `pattern => expression`. Arms are tried in order, and the first one that matches runs. Patterns can match variants, take their fields apart (`Ok(value)`), and be nested (`Err(NotFound(path))`). The variables bound by a pattern (`value`, `error`) are `imut`.

```ez
match (user) {
	Admin(name) => print(`Admin: ${name}`)
	Guest => print('Guest user')
}
```

A `match` is exhaustive: it must handle every possible variant, unless a wildcard (`_`) is used:

```ez
match (option) {
	Some(value) => print(value)
	_ => print('No value')
}
```

A `match` is also an expression, so its value can be assigned to a variable:

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

### Guards

A guard, `if (condition)` after a pattern, makes the arm match only when the condition is true. `match` with guards replaces both `else if` chains and `switch` / `case` statements:

```ez
age: int = 14

const label: string = match (age) {
    x if (x > 18) => 'Adult'
    x if (x > 12) => 'Teen'
    _             => 'Minor'
}

print(label) // Teen
```
