# math

The `math` module provides mathematical functions and constants.

## Functions

The parameters written `float` also accept an `int`.

-   `abs(x: int) -> int`, `abs(x: float) -> float`  
    Returns the absolute value of `x`, with the same type as `x`.

-   `sqrt(x: float) -> Result[float, MathError]`  
    Returns the square root of `x`. Fails with `Complex` when `x` is negative.

-   `pow(x: float, y: float) -> Result[float, MathError]`  
    Returns `x` raised to the power of `y`. Fails with `Complex` when `x` is negative and `y` is not an integer (e.g. `pow(-8, 0.5)`), and with `Infinite` when `x` is `0` and `y` is negative (e.g. `pow(0, -1)`, which is `1 / 0`). `pow(0, 0)` is `1`: that is the value of the expression (only the limit form 0⁰ is indeterminate).

-   `floor(x: float) -> int`  
    Returns the largest integer less than or equal to `x`.

-   `ceil(x: float) -> int`  
    Returns the smallest integer greater than or equal to `x`.

-   `round(x: float) -> int`  
    Rounds `x` to the nearest integer (halfway cases away from zero: `round(2.5)` is `3`).

## Errors

The `math` module follows one rule:

-   **Domain problems return a `MathError`.** The result does not exist in the real numbers.
-   **Representation limits panic.** The result exists, but does not fit in an `int` or a `float` (e.g. `math.pow(10, 400)`, or `math.round(1e300)`, which does not fit in an `int`).

```ez
type MathError = {
    Complex // the result exists only in the complex numbers: sqrt(-1)
    Indeterminate // indeterminate form (no current function returns it)
    Infinite // the result is infinite: pow(0, -1)
}
```

`Infinite` is not an overflow. To tell them apart, ask whether a larger type would fit the result: `pow(10, 400)` is a finite number too large for a `float` (overflow, it panics), while `pow(0, -1)` is infinite, so no type could represent it (`Err(Infinite)`).

Operators cannot return a `Result`, so they panic instead: `1 / 0` and `0.0 ** -1.0` are division by zero (see [Operators](../operators.md)).

`float` has no NaN and no infinity, so these cases cannot be represented as a `float` value (see [Primitive Types](../primitive-types.md)).

## Constants

-   `pi`  
    The mathematical constant π (approximately 3.14159).

-   `e`  
    Euler's number (approximately 2.71828).

## Example

```ez
import math

match (math.pow(5, 2)) {
    Ok(squared) => print(math.pi * squared)
    Err(error) => print(error)
}

match (math.sqrt(-1)) {
    Ok(root) => print(root)
    Err(Complex) => print('no real square root')
    Err(Indeterminate) => print('indeterminate')
    Err(Infinite) => print('infinite')
}
```
