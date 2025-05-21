# Primitive Types

## Numeric Types

-   `int`: integer numbers.

```ez
age: int = 23
```

-   `float`: floating-point numbers.

```ez
const pi: float = 3.14
```

## Text Types

-   `string`: character sequences.

```ez
name: string = 'Mary'
```

## Boolean Types

-   `boolean`: logical values (`true`, `false`).

```ez
isStudent: boolean = true
```

## Missing Values

The ez language does **not** include `null` or `undefined`.

To represent the absence of a value, use the `Option[T]` sum type:

```ez
name: Option[string] = None
```

To indicate an uninitialized or computed-later value, define an optional and update it later:

```ez
x: Option[int] = None

// Later in the code
x = Some(42)
```
