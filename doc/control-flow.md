# Control Flow

## Conditional

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

```ez
while (count < 5) {
    print(count)

    count += 1
}
```

## Switch-case

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
