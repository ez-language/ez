# datetime

The `datetime` module provides utilities to work with current date and time, and to format or parse date strings.

## Functions

-   `now() -> string`  
    Returns the current date and time as a formatted string (e.g., `2025-04-20 14:30:00`).

    `now()` methods:

    -   `year`: full year
    -   `month`: month
    -   `day`: day
    -   `hour`: hour
    -   `minute`: minutes
    -   `second`: seconds

-   `today() -> string`  
    Returns the current date as a string (e.g., `2025-04-20`).

-   `format(timestamp: number, pattern: string) -> string`  
    Converts a timestamp to a formatted date string based on a pattern.

    Format tokens can include:

    -   `yyyy`: full year
    -   `mm`: month (01–12)
    -   `dd`: day of the month (01–31)
    -   `hh`: hour (00–23)
    -   `min`: minutes (00–59)
    -   `ss`: seconds (00–59)

-   `parse(dateString: string, pattern: string) -> number`  
    Parses a date string into a timestamp according to the given pattern.

## Example

```ez
import datetime

date = datetime.today()

print(date)

timestamp = datetime.parse('2025-04-20', 'YYYY-MM-DD')

print(datetime.format(timestamp, 'DD/MM/YYYY'))
```
