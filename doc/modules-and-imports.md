# Modules and Imports

Modules allow you to organize your code in separate files, and you can import functions and variables as needed.

## Module Structure:

```ez
// file: math.ez

function sum(a: int, b: int): int {
  return a + b
}

function subtract(a: int, b: int): int {
  return a - b
}
```

## Using Modules:

```ez
// file: main.ez

import math from 'math.ez'

result: int = math.sum(5, 10)

print(result) // 15
```

In the example above, we're importing the `math` module that contains the functions `sum` and `subtract`, which we can call in the main code.
