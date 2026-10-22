# Primitive Types

## Numeric Types

-   `int`: 64-bit integer numbers.

```ez
age: int = 23
```

-   `float`: 64-bit floating-point numbers. A `float` is always a finite real number: there is no NaN and no infinity.

```ez
const pi: float = 3.14
const avogadro: float = 6.022e23 // scientific notation
```

A float literal too large to be represented (`1e400`) is a compile-time error.

When a numeric result does not fit in an `int` or a `float` (a representation limit), the program panics. For example, `9223372036854775807 + 1` and `1e308 * 10` stop the program. Results that do not exist in the real numbers (a domain problem, like the square root of a negative number) are reported by the `math` functions as a `MathError` (see [math](standard-library/math.md)).

## Text

-   `string`: a sequence of characters.

```ez
name: string = 'Mary'
```

## Boolean

-   `boolean`: logical values (`true`, `false`).

```ez
isStudent: boolean = true
```

## Missing Values

The ez language does **not** include `null` or `undefined`.

To represent the absence of a value, use the `Option[T]` sum type:

```ez
name: Option[string] = None
```

A value that is only known later starts as `None` and is set afterwards:

```ez
x: Option[int] = None

// Later in the code
x = Some(42)
```
