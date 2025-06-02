# Sum Types

Sum types (also known as tagged unions or variant types) allow a value to be one of several different, but fixed, forms. They are useful for modeling cases where a value can have different shapes.

## Defining a Sum Type

Use the `type` keyword to define a sum type with multiple variants:

```ez
type Result {
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
type Shape {
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
