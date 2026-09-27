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
