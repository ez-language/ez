# math

The `math` module provides mathematical functions and constants.

## Functions

-   `abs(x: number) -> number`  
    Returns the absolute value of `x`.

-   `sqrt(x: number) -> number`  
    Returns the square root of `x`.

-   `pow(x: number, y: number) -> number`  
    Returns `x` raised to the power of `y`.

-   `floor(x: number) -> number`  
    Returns the largest integer less than or equal to `x`.

-   `ceil(x: number) -> number`  
    Returns the smallest integer greater than or equal to `x`.

-   `round(x: number) -> number`  
    Rounds `x` to the nearest integer.

## Constants

-   `pi`  
    The mathematical constant π (approximately 3.14159).

-   `e`  
    Euler's number (approximately 2.71828).

## Example

```ez
import math

area: float = math.pi * math.pow(5, 2)

print(area)
```
