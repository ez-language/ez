# File

The `file` library provides functions to read, write, and manage files.

## Importing

```ez
import file
```

## Methods

-   `file.open(path: string)`: opens a file or resource.

```ez
import file

content = file.open('data.txt')
```

-   `file.read(path: string)`: reads the entire content of a file and returns it as a string.

```ez
import file

content = file.read('data.txt')

print(content)
```

-   `file.readLines(path: string)`: reads a file and returns a list of strings, one for each line.

```ez
import file

lines = file.readLines('data.txt')

for line in lines {
    print(line)
}
```

-   `file.write(path: string, content: string)`: writes the content to the file, replacing anything that was already there.

```ez
import file

file.write('log.txt', 'Starting program...\n')
```

-   `file.append(path: string, content: string)`: appends the content to the end of the file.

```ez
import file

file.append('log.txt', 'Another line.\n')
```

-   `file.exists(path: string)`: returns true if the file exists, false otherwise.

```ez
import file

if file.exists('config.json') {
    print('Config found.')
}
```

-   `file.delete(path: string)`: deletes the specified file.

```ez
import file

file.delete('temp.txt')
```
