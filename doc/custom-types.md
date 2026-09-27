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
Each variant is explicitly tagged, so the compiler can verify exhaustiveness
and the programmer always knows which variant they are handling.

```ez
type Result = string | int // Error! Untagged union is not allowed

// Tagged union — each variant is named
type Result = {
    Text(value: string)
    Num(value: int)
}
```

This ensures that "it can be A or B" always comes with a tag you can
match on. There is no "it's one of them, but I don't know which" —
the type system forbids it.

## Type Intersection

Intersection is a type alias that merges fields. It does not create subtyping.

```ez
type Animal = {
    name: string
}

type Dog = Animal & {
    breed: string
}
```

## Subtyping

`ez` does not support subtyping. A type defined with intersection (`&`)
is a flat alias — it merges fields but does not create an "is-a"
relationship.

```ez
function greet(animal: Animal) {
    print(animal.name)
}

const dog: Dog = {
    name: 'Rex',
    breed: 'Pug'
}

greet({ name: dog.name }) // Rex (extract the fields explicitly)
greet(dog) // Error! Dog is not a subtype of Animal
```
