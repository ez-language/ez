# Composite Types

## List

An ordered list of elements of the same type. The type of a list of `T` is written `T[]`.

```ez
numbers: int[] = [1, 2, 3]

coordinates: int[][] = [[1, 2], [3, 4]]

positions: int[][][] = [[[1, 2], [3, 4]], [[5, 6], [7, 8]]]
```

Elements are accessed by their index, starting at `0`. An index out of range panics.

```ez
print(numbers[0])        // 1
print(coordinates[1][0]) // 3
print(numbers.length())  // 3
print(2 in numbers)      // true

numbers[0] = 10          // changes the first element
```

To go through a list, use a [`for` loop](control-flow.md#for-loop).

Dictionaries (keys of type `string` mapped to values) are declared as [index signatures](custom-types.md#index-signatures).
