# String Interpolation

Only strings between backticks support interpolation: `${expression}` is replaced by the value of the expression.

```ez
name: string = 'Emma'

print(`Hello, ${name}!`) // Hello, Emma!
print('Hello, ${name}!') // Hello, ${name}!
print("Hello, ${name}!") // Hello, ${name}!
```

Any expression can be interpolated, and its value is shown as `print` shows it (its [`display()`](custom-types.md#display) text):

```ez
items = [1, 2, 3]

print(`${items.length()} items: ${items}`) // 3 items: [1, 2, 3]
```
