# json

The `json` module provides functions to encode and decode data in JSON format.

## Functions

-   `parse(jsonString: string) -> value`  
    Converts a JSON string into an ez data value (object, array, etc).

-   `stringify(value: any) -> string`  
    Converts an ez value to a JSON string.

## Example

```ez
import json

text: string = '{"name": "ezlang", "year": 2025}'
data = json.parse(text)

print(data.name) // ezlang

jsonText = json.stringify(data)

print(jsonText)
```
