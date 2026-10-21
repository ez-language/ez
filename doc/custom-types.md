# Custom Types

## Shape Types

Types with fixed named fields, useful for representing entities with known structures.

```ez
type Person = {
    name: string
    age: int
}

person: Person = {
    name: 'John',
    age: 20
}
```

Shape types can be nested, and used for variables, function parameters and return types:

```ez
type Address = {
    street: string
    city: string
}

type User = {
    id: int
    name: string
    address: Address
}

user: User = {
    id: 42,
    name: 'Bob',
    address: {
        street: '123 Main St',
        city: 'Exampleville'
    }
}

function printUser(user: User) {
    print(`${user.name} lives in ${user.address.city}`)
}
```

All fields are required, and an object literal cannot have extra fields. Fields can be separated by commas or new lines.

Shape types are **nominal**: two types with the same fields are still different types. An object literal is accepted by any type whose fields it matches.

```ez
type Celsius = { value: float }
type Fahrenheit = { value: float }

const celsius: Celsius = { value: 20.0 }
fahrenheit: Fahrenheit = { value: 68.0 } // Ok

fahrenheit = celsius // Error! Celsius is not Fahrenheit
```

## Index Signatures

Types with dynamic keys, where keys follow a pattern and point to values ​​of the same type.

```ez
type Dictionary = {
    [key: string]: string
}

translations: Dictionary = {
    'hello': 'olá',
    'world': 'mundo'
}
```

## Type Union

`ez` does not support untagged type unions (e.g. `string | int`).

When a value can be one of several types, use a sum type (tagged union).
Each variant is explicitly tagged, so the compiler can verify exhaustiveness and the programmer always knows which variant they are handling.

```ez
type Result = string | int // Error! Untagged union is not allowed

// Tagged union — each variant is named
type Result = {
    Text(value: string)
    Num(value: int)
}
```

This ensures that "it can be A or B" always comes with a tag you can match on. There is no "it's one of them, but I don't know which" — the type system forbids it.

## Type Intersection

A type defined with intersection (`&`) is a flat alias that merges fields — it does not create an "is-a" relationship.

```ez
type Animal = {
    name: string
}

type Dog = Animal & {
    breed: string
}
```

## Subtyping

```ez
type Person = {
    name: string
    age: int
}

function Person.greet(self) {
    print(`Hello, I'm ${self.name}!`)
}

const person: Person = {
    name: 'John',
    age: 20
}

person.greet() // Hello, I'm John!
```

```ez
type Rect = {
    width: int
    height: int
}

function Rect.area(self): int {
    return self.width * self.height
}

const rect: Rect = {
    width: 3,
    height: 4
}

print(rect.area()) // 12
```

### Methods

A function declared as `function Type.name(...)` belongs to `Type`:

-   With `self` as the first parameter, it is a method: `value.name(...)`. `self` has no type annotation (it is always the type) and, like every parameter, it is read-only.
-   Without `self`, it is an associated function, called on the type: `Type.name(...)`. It is useful for constructors:

    ```ez
    function Rect.square(side: int): Rect {
        return { width: side, height: side }
    }

    print(Rect.square(3).area()) // 9
    ```

-   Sum types can have methods too:

    ```ez
    type Shape = {
        Circle(radius: float)
        Square(side: float)
    }

    function Shape.area(self): float {
        return match (self) {
            Circle(r) => 3.14 * r * r
            Square(s) => s * s
        }
    }

    print(Square(2.0).area()) // 4.0
    ```

-   Methods are declared at the top level of the file that declares the type, and take no `export`: they are available wherever the type is (`import { Point } from 'geometry.ez'` then `Point.origin()`, or `geometry.Point.origin()`).
-   A method cannot have the same name as a field of its type.
-   A method cannot change `self` yet (`mut self` is still being designed).
