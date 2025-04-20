# time

The `time` module provides functions to delay execution and measure elapsed time.

## Functions

-   `sleep(seconds: number) -> void`  
    Pauses the execution for the specified number of seconds.

-   `now() -> number`  
    Returns the current time in seconds since the epoch. This value reflects the system clock and may change if the system time is adjusted.

-   `monotonic() -> number`  
    Returns a monotonically increasing time value, useful for measuring durations. This value is not affected by changes to the system clock.

## Choosing Between `now()` and `monotonic()`

Use `now()` when you need the current timestamp, such as logging the time of an event.  
Use `monotonic()` when you need to measure elapsed time precisely, regardless of changes to the system clock.

## Example

```ez
import time

print('Waiting...')

time.sleep(2)

print('Done!')

start = time.monotonic()
// Do something
end = time.monotonic()

print(`Elapsed: ${end - start} seconds`)
```
