# file

The `file` module provides functionality for working with files.

## Functions

-   `open(path: string, mode: string) -> File`  
    Opens the file at `path` in the specified `mode`. Returns a `File` object.

    Modes:

    -   `"r"`: read
    -   `"w"`: write (overwrites existing content)
    -   `"a"`: append

## File Object Methods

-   `read() -> string`  
    Reads the entire contents of the file.

-   `write(text: string) -> void`  
    Writes `text` to the file.

-   `close() -> void`  
    Closes the file.

## Example

```ez
import file

f = file.open('notes.txt', 'w')

f.write('Hello world!')
f.close()

f = file.open('notes.txt', 'r')

content = f.read()

print(content)

f.close()
```
