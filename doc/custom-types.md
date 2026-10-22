# Custom Types

Custom types are declared with `type`. This page covers shape types (named fields), dictionaries (index signatures), intersections, and the methods of a type. Types with variants are covered in [Sum Types](sum-types.md).

## Shape Types

Types with fixed named fields, for values with a known structure.

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

Types with dynamic keys: every key is a `string`, and every value has the same type. They work as dictionaries.

```ez
type Dictionary = {
    [key: string]: string
}

translations: Dictionary = {
    'hello': 'olá',
    'world': 'mundo'
}

print(translations['hello']) // olá
translations['thanks'] = 'obrigado'
```

Reading a key that does not exist panics. To go through the keys, use `for key in translations`.

## Type Union

ez does not support untagged type unions (e.g. `string | int`).

When a value can be one of several types, use a sum type (tagged union). Each variant is explicitly tagged, so the compiler can verify exhaustiveness and the programmer always knows which variant they are handling.

```ez
type Value = string | int // Error! Untagged union is not allowed

// Tagged union: each variant is named
type Value = {
    Text(value: string)
    Num(value: int)
}
```

This ensures that "it can be A or B" always comes with a tag you can match on. There is no "it's one of them, but I don't know which": the type system forbids it.

## Type Intersection

A type defined with intersection (`&`) merges the fields of its parts into a new type:

```ez
type Animal = {
    name: string
}

type Dog = Animal & {
    breed: string
}

const rex: Dog = { name: 'Rex', breed: 'Beagle' }
```

## No Subtyping

ez has no subtyping. A type is not accepted where another type is expected, even when it has all of its fields: `Dog` above has the fields of `Animal`, but it is not an `Animal`.

```ez
function greet(animal: Animal) {
    print(animal.name)
}

greet(rex) // Error! argument 1 must be Animal, found Dog
greet({ name: rex.name }) // Ok
```

## Methods

A function declared as `function Type.name(...)` belongs to `Type`. With `self` as the first parameter, it is a **method**, called on a value:

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

`self` has no type annotation (it is always the type) and, like every parameter, it is read-only.

### Associated Functions

Without `self`, the function is an **associated function**, called on the type: `Type.name(...)`. It is useful for constructors:

```ez
function Rect.square(side: int): Rect {
    return { width: side, height: side }
}

print(Rect.square(3).area()) // 9
```

### Methods of Sum Types

Sum types can have methods too:

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

### Methods That Change the Value

A method that changes its value declares `mut self`. The call needs no marker, but the value must be a `var` or a `const` (or a field or element of one), not an `imut`:

```ez
type Counter = {
    count: int
}

function Counter.increment(mut self) {
    self.count += 1
}

var counter: Counter = { count: 0 }
counter.increment()
print(counter.count) // 1
```

`mut self` changes the contents of the value, not the value as a whole: `self = ...` is an error. So, as with `const`, the variable keeps the same value, only with different contents.

### Rules

-   Methods are declared at the top level of the file that declares the type, and take no `export`: they are available wherever the type is (`import { Point } from 'geometry.ez'` then `Point.origin()`, or `geometry.Point.origin()`).
-   A method cannot have the same name as a field of its type.
-   A method with `self` is called on a value (`person.greet()`), and an associated function on the type (`Person.create()`); the other way around is an error.

## Display

`print` and string interpolation show a value with its type's `display` method, when there is one. It is declared as `function Type.display(self): string`:

```ez
type Person = {
    name: string
    age: int
}

function Person.display(self): string {
    return `${self.name} (${self.age})`
}

const ana: Person = { name: 'Ana', age: 30 }

print(ana) // Ana (30)
print(`Hi, ${ana}!`) // Hi, Ana (30)!
print(ana.debug()) // { name: 'Ana', age: 30 }
```

-   Every value has `display()` (the text `print` shows) and `debug()` (the structure of the value, as written in code). Without a `display` method, both show the structure, except for strings and standard library errors, whose `display()` is readable (`'abc' is not a valid number`).
-   Inside a list or another value, the structure is shown: `print([ana])` gives `[{ name: 'Ana', age: 30 }]`.
-   `debug` cannot be declared: it always shows the structure.
