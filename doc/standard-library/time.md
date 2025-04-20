# Time

The `time` library provides functions related to delays and time measurement.

## Importing

```ez
import time
```

## Methods

-   `time.sleep(seconds: number)`: pauses the program for a given number of seconds.

```ez
import time

print('Waiting...')

time.sleep(2)

print('Done!')
```

-   `time.now()`: returns the current timestamp in seconds since a fixed point in time (like the Unix epoch).

```ez
import time

now = time.now()

print(`Current time: ${now}`)
```

-   `time.elapsed(start: number, end: number)`: calculates the time difference between two values.

```ez
import time

start = time.now()

time.sleep(1.5)

end = time.now()

print(time.elapsed(start, end)) // 1.5
```
