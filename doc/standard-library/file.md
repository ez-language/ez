# file

The `file` module provides functionality for working with files.

## Functions

-   `open(path: string, mode: string) -> Result[File, FileError]`  
    Opens the file at `path` in the specified `mode`.

    Modes:

    -   `'r'`: read
    -   `'w'`: write (overwrites existing content)
    -   `'a'`: append

    An invalid mode is a bug: it panics.

## File Object Methods

-   `read() -> Result[string, FileError]`  
    Reads the entire contents of the file. The file must have been opened with `'r'`.

-   `write(text: string) -> Result[void, FileError]`  
    Writes `text` to the file. The file must have been opened with `'w'` or `'a'`.

-   `close() -> void`  
    Closes the file.

Reading a file opened for writing, writing to a file opened for reading, or using a closed file are bugs: they panic.

## Errors

```ez
type FileError = {
    NotFound(path: string)
    PermissionDenied(path: string)
    IsDirectory(path: string)
    InvalidUtf8(path: string)              // the content is not valid text
    Io(path: string, message: string)      // any other failure of the operating system
}
```

## Example

```ez
import file

match (file.open('notes.txt', 'w')) {
    Ok(f) => {
        f.write('Hello world!')
        f.close()
    }
    Err(error) => print(error)
}

match (file.open('notes.txt', 'r')) {
    Ok(f) => {
        match (f.read()) {
            Ok(content) => print(content)
            Err(error) => print(error)
        }

        f.close()
    }
    Err(NotFound(path)) => print(`${path} does not exist`)
    Err(error) => print(error)
}
```
