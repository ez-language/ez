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

```ez
type result = string | int
```

## Type Intersection

```ez
type animal = {
    name: string
}

type Dog = Animal & {
    breed: string
}
```
