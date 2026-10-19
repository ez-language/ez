# Functions

Functions allow you to encapsulate logic, making your code reusable and easier to maintain.

## Basic Syntax

```ez
function sum(a: int, b: int): int {
	return a + b
}

print(sum(2, 2)) // 4

function factorial(x: int): int {
    if (x <= 1) { return 1 }

    return x * factorial(x - 1)
}

print(factorial(4)) // 24
```

## Anonymous Function

```ez
const double = (x: int): int => {
	return x * 2
}

// or simply

const double = (x: int) => x * 2

print(double(10)) // 20
```

## Capturing Variables

A function created inside another function can read the variables around it. It captures a **read-only copy**: it sees the values from the moment it was created, and cannot change them.

```ez
function makeAdder(amount: int): (int) => int {
	return (x: int) => x + amount
}

const addFive = makeAdder(5)

print(addFive(1)) // 6

function count(): int {
	total = 0
	add = () => { total += 1 } // Error! cannot modify 'total' here: functions capture a read-only copy of the variables around them
	return total
}
```

Variables at the top level of a module are not captured: functions can read and change them.

```ez
total = 0

function add(n: int) {
	total += n
}

add(2)
print(total) // 2
```
