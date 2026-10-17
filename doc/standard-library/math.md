# math

The `math` module provides mathematical functions and constants.

## Functions

-   `abs(x: number) -> number`  
    Returns the absolute value of `x`.

-   `sqrt(x: number) -> Result[float, MathError]`  
    Returns the square root of `x`. Fails with `Complex` when `x` is negative.

-   `pow(x: number, y: number) -> Result[float, MathError]`  
    Returns `x` raised to the power of `y`. Fails with `Complex` when `x` is negative and `y` is not an integer (e.g. `pow(-8, 0.5)`), and with `Indeterminate` for `pow(0, 0)`.

-   `floor(x: number) -> number`  
    Returns the largest integer less than or equal to `x`.

-   `ceil(x: number) -> number`  
    Returns the smallest integer greater than or equal to `x`.

-   `round(x: number) -> number`  
    Rounds `x` to the nearest integer.

## Errors

The `math` module follows one rule:

-   **Domain problems return a `MathError`.** The result does not exist in the real numbers.
-   **Representation limits panic.** The result exists, but does not fit in an `int` or a `float` (e.g. `math.pow(10, 400)`, or `math.round(1e300)` as an `int`).

```ez
type MathError = {
    Complex          // the result exists only in the complex numbers: sqrt(-1)
    Indeterminate    // indeterminate form: pow(0, 0)
}
```

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
}
```
