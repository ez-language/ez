# Built-ins

-   `input()`: reads a line of input from the user and returns a string.

```ez
name: Option[string] = input("What's your name?")

match (name) {
	Some(line) => print(`Hello, ${line}!`)
	None => print('No input')
}
```

-   `print()`: displays values to the console.

```ez
name: string = 'John'

print(`Hello, ${name}!`)
```

-   `panic(message)`: stops the program with an error message. Use it for states that should be impossible (bugs), not for expected failures, which are `Result` values.

```ez
function check(age: int): int {
	if (age >= 0) { return age }

	panic('age cannot be negative')
}
```

A panic cannot be caught: ez has no `try` / `catch`. Index out of range, division by zero and overflow also panic. Expected failures are `Result` values instead (see [Error Handling](error-handling.md)).
