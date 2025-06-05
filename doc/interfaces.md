## Interfaces

Interfaces define the shape of an object. They describe the expected properties and their types, ensuring that any object that implements an interface matches its structure.

### Basic Syntax

```ez
interface InterfaceName {
    propertyName: Type
    anotherProperty: Type
}
```

### Example

```ez
interface User {
    id: int
    name: string
    email: string
}

user: User = {
    id: 1
    name: 'Alice'
    email: 'alice@example.com'
}
```

### Rules

-   All properties are required by default.
-   Interfaces are structural: any object that has the same shape is considered valid.
-   Interfaces can be used as types for variables, function parameters, and return types.
-   Nested interfaces are supported.

### Nested Interface Example

```ez
interface Address {
    street: string
    city: string
}

interface User {
    id: int
    name: string
    address: Address
}

user: User = {
    id: 42
    name: 'Bob'
    address: {
        street: '123 Main St'
        city: 'Exampleville'
    }
}
```

### Usage in Functions

```ez
interface Product {
    id: int
    name: string
    price: float
}

function printProduct(product: Product) {
    print('Product: ${product.name} (${product.id}) - ${product.price}')
}
```
