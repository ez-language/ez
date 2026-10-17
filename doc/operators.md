# Operators

## Arithmetic Operators

| Operator | Description                       |
| -------- | --------------------------------- |
| `+`      | Addition or concat                |
| `-`      | Subtraction                       |
| `*`      | Multiplication                    |
| `/`      | Division                          |
| `**`     | Exponentiation                    |
| `%`      | Modulus (remainder)               |

Division follows the type of the operands, as in Go and Rust: dividing two `int` values gives an `int` (truncated toward zero), and dividing `float` values gives a `float`. If one operand is a `float`, the result is a `float`. The remainder `%` has the sign of the dividend.

```ez
7 / 2      // 3
-7 / 2     // -3
7.0 / 2.0  // 3.5
7 / 2.0    // 3.5
-7 % 2     // -1
```

`//` is always a comment (see [Comments](comments.md)).

Operators cannot return a `Result`, so an invalid arithmetic operation panics:

-   division by zero (`1 / 0`, `1.5 % 0.0`, `0.0 ** -1.0`);
-   results that do not fit in an `int` or a `float` (overflow);
-   `**` with a result that is not a real number (`(-8.0) ** 0.5`) or indeterminate (`0 ** 0`).

To handle these cases instead of panicking, use `math.pow`, which returns a `MathError` (see [math](standard-library/math.md)).

## Comparison Operators

| Operator | Description                                                |
| -------- | ---------------------------------------------------------- |
| `==`     | Equality                                                   |
| `!=`     | Inequality                                                 |
| `>`      | Greater than                                               |
| `<`      | Less than                                                  |
| `>=`     | Greater than or equal                                      |
| `<=`     | Less than or equal                                         |
| `is`     | Identity comparison                                        |
| `in`     | Membership check (checks if an element is in a collection) |

Examples:

-  `is`:

```ez
const a = { name: 'Ana', age: 30 }
const b = { name: 'Ana', age: 30 }

a == b  // true  — fields are equal
a is b  // false — different objects in memory
```

-  `in`:

```ez
'a' in 'banana'  // true
2 in [1, 2, 3]   // true
```

`is` also checks the variant of a sum type value:

```ez
result = Some(1)

if (result is Some) {
	print(result.value) // 1
}
```

## Logical Operators

| Operator | Description |
| -------- | ----------- |
| `and`    | Logical AND |
| `or`     | Logical OR  |
| `not`    | Logical NOT |

Examples:

-  `not`:

```ez
if not (age > 18) { ... }
```

## Assignment Operators

| Operator | Description                                |
| -------- | ------------------------------------------ |
| `=`      | Assignment (assigns a value to a variable) |
| `+=`     | Add and assign                             |
| `-=`     | Subtract and assign                        |
| `*=`     | Multiply and assign                        |
| `/=`     | Divide and assign                          |
| `**=`    | Exponentiate and assign                    |
| `%=`     | Modulus and assign                         |

## String Operators

| Operator | Description                                  |
| -------- | -------------------------------------------- |
| `+`      | Concatenation (joins strings)                |
| `*`      | Repetition (repeats a string multiple times) |

## Bitwise Operators

Bitwise operators operate on the binary representations of integers.

| Operator | Description | Example (a = 5, b = 3) |
| -------- | ----------- | ---------------------- |
| `&`      | AND         | `a & b → 1`            |
| `\|`     | OR          | `a \| b → 7`           |
| `^`      | XOR         | `a ^ b → 6`            |
| `~`      | NOT         | `~a → -6`              |
| `<<`     | Shift left  | `a << 1 → 10`          |
| `>>`     | Shift right | `a >> 1 → 2`           |

Explanation:

-   `a = 5` → `0101` (binary), `b = 3` → `0011` (binary)
-   `a & b` → `0101 & 0011` = `0001` → `1`
-   `a | b` → `0101 | 0011` = `0111` → `7`
-   `a ^ b` → `0101 ^ 0011` = `0110` → `6`
-   `~a` → bitwise NOT of `0101` = `1010` (in two's complement, this is `-6`)
-   `a << 1` → shift `0101` one bit to the left = `1010` → `10`
-   `a >> 1` → shift `0101` one bit to the right = `0010` → `2`
