# json

The `json` module provides functions to encode and decode data in JSON format.

## The `Json` Type

JSON data is represented by a sum type, so every kind of value is explicit and must be handled with `match`:

```ez
type Json = {
    Null
    Bool(value: boolean)
    Int(value: int)
    Float(value: float)
    String(value: string)
    Array(items: Json[])
    Object(fields: JsonObject)
}

type JsonObject = {
    [key: string]: Json
}
```

-   A number without `.`, `e` or `E` is an `Int` (`2025`); any other number is a `Float` (`2025.0`, `1e3`).
-   JSON `null` is the `Null` variant: ez itself has no `null`.
-   `Int(1) == Float(1.0)` is `false`: they are different variants.

## Functions

-   `parse(text: string) -> Result[Json, JsonError]`  
    Converts a JSON string into a `Json` value.

-   `stringify(value: Json) -> string`  
    Converts a `Json` value to a JSON string. A `Float` always keeps its decimal point (`Float(3.0)` is written `3.0`), so that parsing the text back gives a `Float` again.

-   `get(value: Json, key: string) -> Option[Json]`  
    The field `key` of an `Object`, or `None` if `value` is not an object or has no such field.

-   `asString(value: Json) -> Option[string]`, `asInt(value: Json) -> Option[int]`, `asBool(value: Json) -> Option[boolean]`  
    The payload of a `String`, `Int` or `Bool`, or `None` for any other variant.

-   `asFloat(value: Json) -> Option[float]`  
    The payload of a `Float`, or of an `Int` converted to `float`. Many serializers write the float `3.0` as `3`, so this accepts both.

## Errors

```ez
type JsonError = {
    Syntax(message: string, line: int, column: int)
    NumberOutOfRange(text: string, line: int, column: int) // an integer too large for int
}
```

## Example

```ez
import json

text: string = '{"name": "ezlang", "year": 2025}'

match (json.parse(text)) {
    Ok(Object(fields)) => match (fields['name']) {
        String(name) => print(name) // ezlang
        _ => print('name is not a string')
    }
    Ok(_) => print('expected an object')
    Err(error) => print(`invalid JSON: ${error}`)
}
```

With the helper functions:

```ez
import json

match (json.parse('{"name": "ezlang", "year": 2025}')) {
    Ok(data) => {
        match (json.get(data, 'year')) {
            Some(year) => print(json.asInt(year)) // Some(2025)
            None => print('no year')
        }

        print(json.stringify(data)) // {"name":"ezlang","year":2025}
    }
    Err(error) => print(error)
}
```

Building a value to convert:

```ez
import json

print(json.stringify(json.Array([json.Int(1), json.Float(2.0), json.Null]))) // [1,2.0,null]
```
