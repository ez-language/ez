# Sum Types

Sum types (also known as tagged unions or variant types) allow a value to be one of several different, but fixed, forms. They are useful for modeling cases where a value can have different shapes.

## Defining a Sum Type

Use the `type` keyword to define a sum type with multiple variants. A variant can have fields (`Success(user: string)`) or none (`Guest`):

```ez
type Login = {
	Success(user: string)
	Failure(message: string)
	Guest
}
```

## Using a Sum Type

You can use pattern matching with `match` to handle each variant of a sum type.

```ez
function describe(login: Login) {
	match (login) {
		Success(user) => print(`Welcome, ${user}`)
		Failure(message) => print(`Error: ${message}`)
		Guest => print('Hello, guest')
	}
}

describe(Success('Ana')) // Welcome, Ana
```

## Built-in Sum Types

`Option[T]` and `Result[T, E]` are provided by the language and behave as if they were declared like this (user-defined types cannot take type parameters yet):

```ez
type Option[T] = {
	Some(value: T)
	None
}

type Result[T, E] = {
	Ok(value: T)
	Err(error: E)
}
```

Use `is` to check a variant. Inside the `if`, its fields can be accessed directly:

```ez
name: Option[string] = Some('Ana')

if (name is Some) {
	print(name.value) // Ana
}
```

## Wildcard Matching

Use `_` as a wildcard to match any value not already covered:

```ez
match (option) {
	Some(value) => print(value)
	_ => print('No value')
}
```

## Nesting and Destructuring

Sum types support nested patterns and destructuring:

```ez
type Shape = {
	Circle(radius: float)
	Rectangle(width: float, height: float)
}

function area(shape: Shape): float {
	match (shape) {
		Circle(radius) => 3.14 * radius * radius
		Rectangle(width, height) => width * height
	}
}
```

A `match` on a sum type is **exhaustive**: every variant must be handled, unless a wildcard is used. Adding a variant to a type makes every `match` that does not handle it a compile error, so no case is forgotten.

Sum types represent "one of these cases" safely, without `null` or `undefined`. Sum types can also have methods (see [Custom Types](custom-types.md#methods)).
