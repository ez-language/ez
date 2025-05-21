## Sum Types (Tagged Enums)

The **ez** language supports **sum types** through the `enum` keyword. These types allow representing data that can take on **different named variants**, each optionally carrying data. Sum types are ideal for modeling optional values, operation results, and more.

**ez does not have `null`**. Instead, absence of a value is expressed using sum types like `Option`.

### Definition

Sum types are declared using the `enum` keyword:

```ez
enum TypeName[Gen1, Gen2, ...] {
  Variant1(field1: Type1, field2: Type2)
  Variant2
  Variant3(value: Type)
}
```

-   Each **variant** may contain **named fields**, a **single unnamed value**, or **no data**.
-   **Generic types** are supported via square brackets `[]`.

#### Example: optional value

```ez
enum Option[T] {
  Some(value: T)
  None
}
```

#### Example: operation result

```ez
enum Result[T, E] {
  Ok(value: T)
  Err(error: E)
}
```

---

### Creating values

```ez
name: Option[string] = Some('Anna')
age: Option[int] = None

result: Result[int, string] = Ok(42)
failure: Result[int, string] = Err('Connection error')
```

---

### Pattern matching with `match`

The `match` expression allows safely deconstructing and handling each variant:

```ez
match (name) {
  Some(value) => print(`Name: ${value}`)
  None => print('No name')
}
```

```ez
match (result) {
  Ok(value) => print(`Success: ${value}`)
  Err(e) => print(`Failure: ${e}`)
}
```

All variants **must be covered**. If not, the compiler will emit an error.

---

### Expression form

`match` can return a value directly:

```ez
message = match (name) {
  Some(value) => `Welcome, ${value}`
  None => 'Anonymous user'
}
```

---

### Best practices

-   Use `Option[T]` instead of allowing `null`.
-   Use `Result[T, E]` to model success and failure.
-   Avoid using booleans with additional optional values—prefer dedicated sum types.

---

### Notes

-   Variants support named fields or a single unnamed value.
-   `match` is required to be exhaustive unless explicitly marked otherwise (future feature).
-   Sum types are a core feature for safe and expressive data modeling in ez.
