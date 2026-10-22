# file

The `file` module provides functionality for working with files.

## Functions

-   `open(path: string, mode: Mode) -> Result[File, FileError]`  
    Opens the file at `path` in the specified `mode`:

    ```ez
    type Mode = {
        Read // read
        Write // write (overwrites existing content)
        Append // write at the end
    }
    ```

    With `import file`, the modes are `file.Read`, `file.Write` and `file.Append`. With `import { Mode } from 'file'`, they can be written `Read`, `Write` and `Append`. Since the mode is a sum type, an invalid mode does not compile.

## Methods of `File`

-   `read() -> Result[string, FileError]`  
    Reads the entire contents of the file. The file must have been opened with `Read`.

-   `write(text: string) -> Result[void, FileError]`  
    Writes `text` to the file. The file must have been opened with `Write` or `Append`.

-   `close() -> void`  
    Closes the file.

Reading a file opened for writing, writing to a file opened for reading, or using a closed file are bugs: they panic.

## Errors

```ez
type FileError = {
    NotFound(path: string)
    PermissionDenied(path: string)
    IsDirectory(path: string)
    InvalidUtf8(path: string) // the content is not valid text
    Io(path: string, message: string) // any other failure of the operating system
}
```

Printing an error shows a readable message, also available as `error.display()`:

```ez
match (file.open('missing.txt', file.Read)) {
    Ok(f) => print(f.read())
    Err(error) => print(error) // file 'missing.txt' not found
}
```

## Example

```ez
import file

match (file.open('notes.txt', file.Write)) {
    Ok(f) => {
        f.write('Hello world!')
        f.close()
    }
    Err(error) => print(error)
}

match (file.open('notes.txt', file.Read)) {
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

With the `?` operator (see [Error Handling](../error-handling.md)), inside a function:

```ez
import file

function readNotes(): Result[string, file.FileError] {
    f = file.open('notes.txt', file.Read)?
    content = f.read()?
    f.close()

    return Ok(content)
}
```
