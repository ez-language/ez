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
