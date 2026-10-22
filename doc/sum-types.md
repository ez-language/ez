# Sum Types

Sum types (also known as tagged unions or variant types) allow a value to be one of several different, but fixed, forms. They are useful for modeling cases where a value can have different shapes.

## Defining a Sum Type

Use the `type` keyword to define a sum type with multiple variants:

```ez
type Result = {
	Ok(value: string)
	Err(message: string)
}
```

## Using a Sum Type

You can use pattern matching with `match` to handle each variant of a sum type.

```ez
function handleResult(result: Result) {
	match (result) {
		Ok(value) => print(`Success: ${value}`)
		Err(message) => print(`Error: ${message}`)
	}
}
```

## Built-in Sum Types

`Option[T]` and `Result[T, E]` are provided by the language and behave as if they were declared like this (user-defined types cannot take type parameters):

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

Use `is` to check a variant; inside the `if`, its fields can be accessed directly:

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

Sum types are **exhaustive**: all possible variants must be handled unless a wildcard is used.

They are ideal for representing safe and expressive data flows without relying on `null` or `undefined`.
