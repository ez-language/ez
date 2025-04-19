# Operators

## Arithmetic Operators

| Operator | Description                       |
| -------- | --------------------------------- |
| `+`      | Addition or concat                |
| `-`      | Subtraction                       |
| `*`      | Multiplication                    |
| `/`      | Division (floating-point)         |
| `//`     | Integer Division (floor division) |
| `**`     | Exponentiation                    |
| `%`      | Modulus (remainder)               |

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

## Logical Operators

| Operator | Description |
| -------- | ----------- |
| `and`    | Logical AND |
| `or`     | Logical OR  |
| `not`    | Logical NOT |

## Assignment Operators

| Operator | Description                                |
| -------- | ------------------------------------------ |
| `=`      | Assignment (assigns a value to a variable) |
| `+=`     | Add and assign                             |
| `-=`     | Subtract and assign                        |
| `*=`     | Multiply and assign                        |
| `/=`     | Divide and assign                          |
| `//=`    | Integer Divide and assign                  |
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
