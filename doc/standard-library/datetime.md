# Datetime

The `datetime` library provides utilities to get the current date and time, parse strings into dates, and format datetime objects.

## Importing

```ez
import datetime
```

## Methods

-   `datetime.now()`: returns an object representing the current date and time.

```ez
import datetime

now = datetime.now()

print(`Year: ${now.year}`)
print(`Month: ${now.month}`)
print(`Day: ${now.day}`)
print(`Hour: ${now.hour}`)
print(`Minute: ${now.minute}`)
```

-   `datetime.format(datetime: datetime, format: string)`: formats a datetime object into a string.

Format tokens can include:

-   `yyyy` — full year
-   `mm` — month (01–12)
-   `dd` — day of the month (01–31)
-   `hh` — hour (00–23)
-   `min` — minutes (00–59)
-   `ss` — seconds (00–59)

```ez
import datetime

now = datetime.now()
formatted = datetime.format(now, 'yyyy-mm-dd hh:min:ss')

print(formatted) // e.g., 2025-04-19 14:35:02
```

-   `datetime.parse(string: string)`: parses a string into a datetime object.

```ez
import datetime

date = datetime.parse('2025-04-19 14:35:02')

print(`Parsed hour: ${date.hour}`)
```
