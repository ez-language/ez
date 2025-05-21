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
