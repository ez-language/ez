# Modules and Imports

Modules allow you to organize your code in separate files, and you can import an entire module or specific items from it.

## Module Structure:

```ez
// file: math.ez

export function add(a: int, b: int): int {
	return a + b
}

export function subtract(a: int, b: int): int {
	return a - b
}
```

Everything in a module is private by default. Only declarations marked with `export` (functions, constants, variables and types) can be imported by other modules. Exporting a sum type also exports its variants.

```ez
// file: math.ez

function clamp(x: int): int { ... }   // private: only usable inside math.ez

export const pi: float = 3.14159
```

```ez
import { clamp } from 'math.ez' // Error! 'clamp' is not exported by 'math.ez'
```

`export` is only allowed at the top level of a file, and must be followed by `function`, `const`, `var` or `type`. A declaration without a keyword (`x = 1`) cannot be exported.

## Using Modules:

```ez
// file: main.ez

import math from 'math.ez'

result: int = math.add(5, 10)

print(result) // 15
```

Or import specific items:

```ez
// file: main.ez

import { add, subtract } from 'math.ez'

result: int = add(5, 10)

print(result) // 15
```

In the example above, we're importing the `math` module that contains the functions `add` and `subtract`, which we can call in the main code.

## Exported Variables

An exported `var` is read-only outside its module: only the module itself can assign to it. Importers always see its **current** value.

```ez
// file: counter.ez

export var count = 0

export function increment() {
	count += 1
}
```

```ez
// file: main.ez

import { count, increment } from 'counter.ez'

increment()
print(count)    // 1
count = 5       // Error! 'count' is read-only outside 'counter.ez'
```

Like `const`, only the variable is read-only: the contents of an exported list or object can still be changed.

## Private Types in Public Declarations

An exported declaration cannot use a private type, because other modules could not name it:

```ez
type Helper = {
	value: int
}

export function make(): Helper { ... } // Error! 'make' is exported, but its type uses the private type 'Helper'
```
