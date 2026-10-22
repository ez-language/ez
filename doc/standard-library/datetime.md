# datetime

The `datetime` module provides utilities to work with current date and time, and to format or parse date strings.

## Functions

-   `now() -> DateTime`  
    Returns the current date and time. When printed or interpolated, it is shown as a formatted string (e.g. `2025-04-20 14:30:00`).

    A `DateTime` has these fields (all `int`):

    -   `year`: full year
    -   `month`: month (1–12)
    -   `day`: day of the month (1–31)
    -   `hour`: hour (0–23)
    -   `minute`: minutes
    -   `second`: seconds

-   `today() -> string`  
    Returns the current date as a string (e.g. `2025-04-20`).

-   `format(timestamp: int, pattern: string) -> string`  
    Converts a timestamp (seconds since the epoch) to a date string that follows `pattern`.

    Format tokens (case-insensitive, e.g. `YYYY-MM-DD` or `yyyy-mm-dd`) can include:

    -   `yyyy`: full year
    -   `mm`: month (01–12)
    -   `dd`: day of the month (01–31)
    -   `hh`: hour (00–23)
    -   `min`: minutes (00–59)
    -   `ss`: seconds (00–59)

-   `parse(text: string, pattern: string) -> Result[int, DateError]`  
    Converts a date string that follows `pattern` into a timestamp (seconds since the epoch). The pattern uses the same tokens as `format`.

    ```ez
    type DateError = {
        PatternMismatch(text: string, pattern: string)   // the text does not follow the pattern
        InvalidDate(text: string)                        // e.g. 2025-02-30
    }
    ```

Dates and times are in UTC.

## Example

```ez
import datetime

date = datetime.today()

print(date)

match (datetime.parse('2025-04-20', 'YYYY-MM-DD')) {
    Ok(timestamp) => print(datetime.format(timestamp, 'DD/MM/YYYY')) // 20/04/2025
    Err(error) => print(error)
}

now = datetime.now()

print(now.year)
```
