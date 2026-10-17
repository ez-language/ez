//! Runs the examples from the documentation and checks their output.

use std::cell::RefCell;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[derive(Clone, Default)]
struct Output(Rc<RefCell<Vec<u8>>>);

impl Write for Output {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn execute(source: &str, input: &str, path: &Path) -> (String, Result<(), ez::error::EzError>) {
    let output = Output::default();
    let result = ez::run_source(
        source,
        path,
        Box::new(output.clone()),
        Box::new(std::io::Cursor::new(input.to_string().into_bytes())),
    );
    let text = String::from_utf8(output.0.borrow().clone()).unwrap();
    (text, result)
}

fn run_with_input(source: &str, input: &str) -> String {
    let (output, result) = execute(source, input, Path::new("test.ez"));
    if let Err(error) = result {
        panic!("program failed: {}\noutput so far:\n{}", error, output);
    }
    output
}

fn run(source: &str) -> String {
    run_with_input(source, "")
}

/// Runs a program that must fail and returns the error message.
fn run_err(source: &str) -> String {
    let (output, result) = execute(source, "", Path::new("test.ez"));
    match result {
        Ok(()) => panic!("program should have failed, output:\n{}", output),
        Err(error) => error.to_string(),
    }
}

fn assert_error(source: &str, fragment: &str) {
    let error = run_err(source);
    assert!(
        error.contains(fragment),
        "expected an error containing {:?}, found {:?}",
        fragment,
        error
    );
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ez-test-{}-{}", name, std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

// ---------------------------------------------------------------------------
// Primitive types, variables and comments
// ---------------------------------------------------------------------------

#[test]
fn primitive_types() {
    let source = r#"
        age: int = 23
        const pi: float = 3.14
        name: string = 'Mary'
        isStudent: boolean = true
        print(age, pi, name, isStudent)
    "#;
    assert_eq!(run(source), "23 3.14 Mary true\n");
}

#[test]
fn option_instead_of_null() {
    let source = r#"
        name: Option[string] = None
        x: Option[int] = None

        // Later in the code
        x = Some(42)
        print(name, x)
    "#;
    assert_eq!(run(source), "None Some(42)\n");
}

#[test]
fn global_and_local_variables() {
    let source = r#"
        globalVar = 1 // Global variable

        function test() {
            var localVar = 2 // Local variable

            print(globalVar) // 1 (accesses the global variable)
            print(localVar) // 2 (accesses the local variable within the function)
        }

        test()

        print(globalVar) // 1 (global variable accessible outside the function)
    "#;
    assert_eq!(run(source), "1\n2\n1\n");
    assert_error(
        "function test() {\n var localVar = 2\n}\nprint(localVar)",
        "undefined variable 'localVar'",
    );
}

#[test]
fn functions_can_update_globals() {
    let source = r#"
        count = 0
        function increment() {
            count += 1
        }
        increment()
        increment()
        print(count)
    "#;
    assert_eq!(run(source), "2\n");
}

#[test]
fn const_cannot_be_reassigned() {
    assert_error("const pi = 3.14159\npi = 3.14", "cannot modify a 'const' value");
}

#[test]
fn const_reference_contents_can_change() {
    let source = r#"
        type Person = {
            name: string
            age: int
        }

        const person: Person = {
            name: 'Ana',
            age: 30
        }

        person.age = 31 // Change the value of person age
        print(person.age)
    "#;
    assert_eq!(run(source), "31\n");
    assert_error(
        "type Person = { name: string }\nconst person: Person = { name: 'Ana' }\nperson = { name: 'Bia' }",
        "cannot modify a 'const' value",
    );
}

#[test]
fn comments() {
    let source = "// Single-line comment\n/*\nMulti-line\ncomment\n*/\nprint('ok') // trailing";
    assert_eq!(run(source), "ok\n");
}

#[test]
fn lists() {
    let source = r#"
        numbers: int[] = [1, 2, 3]
        coordinates: int[][] = [[1, 2], [3, 4]]
        positions: int[][][] = [[[1, 2], [3, 4]], [[5, 6], [7, 8]]]
        print(numbers, coordinates[1][0], positions[1][1][1], numbers.length())
    "#;
    assert_eq!(run(source), "[1, 2, 3] 3 8 3\n");
    assert_error("numbers: int[] = [1, 'a']", "same type");
}

// ---------------------------------------------------------------------------
// Operators
// ---------------------------------------------------------------------------

#[test]
fn arithmetic_operators() {
    assert_eq!(
        run("print(1 + 2, 5 - 7, 3 * 4, 7 / 2, 7 //2, 2 ** 10, 7 % 3, -7 //2, 2.5 * 2)"),
        "3 -2 12 3.5 3 1024 1 -4 5.0\n"
    );
}

#[test]
fn integer_division_is_not_a_comment() {
    assert_eq!(run("x = 17\ny = x//5\nx //= 2\nprint(y, x)"), "3 8\n");
}

#[test]
fn comparison_operators() {
    assert_eq!(
        run("print(1 == 1, 1 != 2, 3 > 2, 2 < 3, 3 >= 3, 2 <= 1, 'a' < 'b')"),
        "true true true true true false true\n"
    );
}

#[test]
fn identity_and_membership() {
    let source = r#"
        const a = { name: 'Ana', age: 30 }
        const b = { name: 'Ana', age: 30 }

        print(a == b)  // true  — fields are equal
        print(a is b)  // false — different objects in memory
        print(a is a)
        print('a' in 'banana')
        print(2 in [1, 2, 3], 'x' in ['a'])
    "#;
    assert_eq!(run(source), "true\nfalse\ntrue\ntrue\ntrue false\n");
}

#[test]
fn logical_operators() {
    let source = r#"
        age = 20
        if not (age > 18) { print('minor') } else { print('adult') }
        print(true and false, true or false, not true)
    "#;
    assert_eq!(run(source), "adult\nfalse true false\n");
}

#[test]
fn assignment_operators() {
    let source = r#"
        e = 10
        e += 1
        e -= 2
        e *= 3
        e //= 2
        e **= 2
        e %= 7
        f = 1.0
        f /= 4
        print(e, f)
    "#;
    assert_eq!(run(source), "1 0.25\n");
}

#[test]
fn string_operators() {
    assert_eq!(run("print('ab' + 'cd', 'ab' * 3)"), "abcd ababab\n");
}

#[test]
fn bitwise_operators() {
    let source = "a = 5\nb = 3\nprint(a & b, a | b, a ^ b, ~a, a << 1, a >> 1)";
    assert_eq!(run(source), "1 7 6 -6 10 2\n");
}

#[test]
fn operator_type_errors() {
    assert_error("print(1 + 'a')", "cannot be applied to int and string");
    assert_error("print(true + 1)", "cannot be applied");
}

// ---------------------------------------------------------------------------
// Strings and conversions
// ---------------------------------------------------------------------------

#[test]
fn string_interpolation() {
    let source = r#"
        name: string = 'Emma'

        print(`Hello, ${name}!`) // Hello, Emma!
        print('Hello, ${name}!') // Hello, ${name}!
        print("Hello, ${name}!") // Hello, ${name}!
        print(`1 + 1 = ${1 + 1}`)
    "#;
    assert_eq!(
        run(source),
        "Hello, Emma!\nHello, ${name}!\nHello, ${name}!\n1 + 1 = 2\n"
    );
}

#[test]
fn conversion_methods() {
    assert_eq!(
        run("print(int('0'), int('abc'), float('0'), string(0), int(3.9), float(2))"),
        "Ok(0) Err('not a valid int') Ok(0.0) 0 3 2.0\n"
    );
    let source = r#"
        match (int('42')) {
            Ok(value) => print(value + 1)
            Err(error) => print(error)
        }
    "#;
    assert_eq!(run(source), "43\n");
}

#[test]
fn string_methods() {
    let source = r#"
        print('abc'.length(), '  hi  '.trim(), 'hello'.upper(), 'HELLO'.lower())
        print('   '.isSpace(), '123'.isNumeric(), 'abc'.isAlpha(), 'abc123'.isAlphaNumeric())
        print('ABC'.isUpper(), 'abc'.isLower(), 'Hello World'.isTitle(), 'Hello world'.isTitle())
        print('a b'.split(' '), 'hello'.replace('hello', 'hi'))
    "#;
    assert_eq!(
        run(source),
        "3 hi HELLO hello\ntrue true true true\ntrue true true false\n['a', 'b'] hi\n"
    );
}

// ---------------------------------------------------------------------------
// Built-ins
// ---------------------------------------------------------------------------

#[test]
fn input_returns_an_option() {
    let source = r#"
        name: Option[string] = input("What's your name?")

        match (name) {
            Some(line) => print(`Hello, ${line}!`)
            None => print('No input')
        }
    "#;
    assert_eq!(run_with_input(source, "Ana\n"), "What's your name?Hello, Ana!\n");
    assert_eq!(run_with_input(source, ""), "What's your name?No input\n");
}

// ---------------------------------------------------------------------------
// Control flow
// ---------------------------------------------------------------------------

#[test]
fn if_else_statement_and_expression() {
    let source = r#"
        age = 20
        if (age > 18) {
            print('Adult')
        } else {
            print('Minor')
        }

        const label: string = if (age > 18) { 'Adult' } else { 'Minor' }

        print(label)
    "#;
    assert_eq!(run(source), "Adult\nAdult\n");
    assert_error("x = if (true) { 1 }", "no value");
    assert_error("if (1) { print(1) }", "must be boolean");
    assert_error("if (true) { } else if (false) { }", "does not have 'else if'");
}

#[test]
fn for_loops() {
    let source = r#"
        print(range(0, 5))
        for i in range(0, 3) { print(i) }
        for i in range(2) { print(i) }
        for i in range(10, 0, -4) { print(i) }

        names: string[] = ['Anna', 'Claire', 'Leon']

        // By index
        for i in range(0, names.length()) {
            print(names[i])
        }

        // Direct iteration
        for name in names {
            print(name)
        }
    "#;
    assert_eq!(
        run(source),
        "[0, 1, 2, 3, 4]\n0\n1\n2\n0\n1\n10\n6\n2\nAnna\nClaire\nLeon\nAnna\nClaire\nLeon\n"
    );
}

#[test]
fn while_break_and_continue() {
    let source = r#"
        count = 0
        while (count < 3) {
            print(count)

            count += 1
        }

        for i in range(0, 10) {
            if (i == 5) {
                break
            }

            print(i)
        }

        for i in range(0, 10) {
            if (i % 2 == 0) {
                continue
            }

            print(i)
        }
    "#;
    assert_eq!(run(source), "0\n1\n2\n0\n1\n2\n3\n4\n1\n3\n5\n7\n9\n");
    assert_error("break", "can only be used inside a loop");
    assert_error("continue", "can only be used inside a loop");
}

#[test]
fn match_with_guards() {
    let source = r#"
        function classify(age: int): string {
            match (age) {
                x if (x > 18) => 'Adult'
                x if (x > 12) => 'Teen'
                _             => 'Minor'
            }
        }
        age: int = 14

        const label: string = match (age) {
            x if (x > 18) => 'Adult'
            x if (x > 12) => 'Teen'
            _             => 'Minor'
        }

        print(label) // Teen
        print(classify(30), classify(5))
    "#;
    assert_eq!(run(source), "Teen\nAdult Minor\n");
}

#[test]
fn match_arms_must_agree_with_the_declared_type() {
    let source = r#"
        result = Ok(1)
        const label: int = match (result) {
            Ok(value) => `Value: ${value}` // Error! string is not int
            Err(error) => `Error: ${error}`
        }
    "#;
    assert_error(source, "has type string, expected int");
}

#[test]
fn match_must_be_exhaustive() {
    assert_error(
        "option = Some(1)\nmatch (option) {\n Some(value) => print(value)\n}",
        "None is not handled",
    );
    assert_error("match (1) {\n 1 => print(1)\n}", "wildcard");
    let source = r#"
        option = Some(1)
        match (option) {
            Some(value) => print(value)
            _ => print('No value')
        }
    "#;
    assert_eq!(run(source), "1\n");
}

#[test]
fn match_arms_on_separate_lines() {
    let source = r#"
        function sign(n: int): string {
            match (n) {
                0 => 'zero'
                -1 => 'minus one'
                x if (x > 0) => 'positive'
                _ => 'negative'
            }
        }
        print(sign(0), sign(-1), sign(5), sign(-9))
    "#;
    assert_eq!(run(source), "zero minus one positive negative\n");
}

#[test]
fn match_literals_and_nested_patterns() {
    let source = r#"
        type Shape = {
            Circle(radius: float)
            Rectangle(width: float, height: float)
        }
        shapes: Option[Shape][] = [Some(Circle(2.0)), None, Some(Rectangle(1.0, 2.0))]
        for s in shapes {
            match (s) {
                Some(Circle(r)) => print(`circle ${r}`)
                Some(Rectangle(w, h)) => print(`rect ${w}x${h}`)
                None => print('nothing')
            }
        }
        match ('b') {
            'a' => print('A')
            'b' => print('B')
            _ => print('?')
        }
    "#;
    assert_eq!(run(source), "circle 2.0\nnothing\nrect 1.0x2.0\nB\n");
}

// ---------------------------------------------------------------------------
// Sum types and custom types
// ---------------------------------------------------------------------------

#[test]
fn sum_types() {
    let source = r#"
        type Result = {
            Ok(value: string)
            Err(message: string)
        }

        function handleResult(result: Result) {
            match (result) {
                Ok(value) => print(`Success: ${value}`)
                Err(message) => print(`Error: ${message}`)
            }
        }

        handleResult(Ok('saved'))
        handleResult(Err('disk full'))
    "#;
    assert_eq!(run(source), "Success: saved\nError: disk full\n");
}

#[test]
fn sum_type_destructuring() {
    let source = r#"
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

        print(area(Circle(1.0)), area(Rectangle(2.0, 3.0)))
    "#;
    assert_eq!(run(source), "3.14 6.0\n");
}

#[test]
fn unit_variants() {
    let source = r#"
        type User = {
            Admin(name: string)
            Guest
        }
        users: User[] = [Admin('Ana'), Guest]
        for user in users {
            match (user) {
                Admin(name) => print(`Admin: ${name}`)
                Guest => print('Guest user')
            }
        }
    "#;
    assert_eq!(run(source), "Admin: Ana\nGuest user\n");
}

#[test]
fn recursive_sum_types() {
    let source = r#"
        type List = {
            Cons(head: int, tail: List)
            Nil
        }
        function total(list: List): int {
            match (list) {
                Cons(head, tail) => head + total(tail)
                Nil => 0
            }
        }
        print(total(Cons(1, Cons(2, Cons(3, Nil)))))
    "#;
    assert_eq!(run(source), "6\n");
}

#[test]
fn shape_types() {
    let source = r#"
        type Person = {
            name: string
            age: int
        }

        person: Person = {
            name: 'John',
            age: 20
        }
        print(person.name, person.age, person)
    "#;
    assert_eq!(run(source), "John 20 { name: 'John', age: 20 }\n");
    assert_error("type P = { a: int }\np: P = { a: 1, b: 2 }", "has no field 'b'");
    assert_error("type P = { a: int  b: int }\np: P = { a: 1 }", "missing field(s) b");
    assert_error("type P = { a: int }\np: P = { a: 'x' }", "must be int");
}

#[test]
fn index_signatures() {
    let source = r#"
        type Dictionary = {
            [key: string]: string
        }

        translations: Dictionary = {
            'hello': 'olá',
            'world': 'mundo'
        }
        translations['bye'] = 'tchau'
        print(translations['hello'], 'world' in translations, translations.length())
        for key in translations { print(key) }
    "#;
    assert_eq!(run(source), "olá true 3\nhello\nworld\nbye\n");
    assert_error(
        "type D = { [key: string]: string }\nd: D = { 'a': 1 }",
        "must be string",
    );
}

#[test]
fn untagged_unions_are_rejected() {
    assert_error("type Result = string | int", "untagged union");
}

#[test]
fn type_intersection_and_no_subtyping() {
    let types = r#"
        type Animal = {
            name: string
        }

        type Dog = Animal & {
            breed: string
        }

        function greet(animal: Animal) {
            print(animal.name)
        }

        const dog: Dog = {
            name: 'Rex',
            breed: 'Pug'
        }
    "#;
    let ok = format!("{}\ngreet({{ name: dog.name }})\nprint(dog.breed)", types);
    assert_eq!(run(&ok), "Rex\nPug\n");
    let error = format!("{}\ngreet(dog)", types);
    assert_error(&error, "Dog is not a subtype of Animal");
}

// ---------------------------------------------------------------------------
// Interfaces
// ---------------------------------------------------------------------------

#[test]
fn interfaces() {
    let source = r#"
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

        interface Product {
            id: int
            name: string
            price: float
        }

        function printProduct(product: Product) {
            print(`Product: ${product.name} (${product.id}) - ${product.price}`)
        }

        type Item = {
            id: int
            name: string
            price: float
        }
        const item: Item = { id: 2, name: 'Cup', price: 3.5 }

        print(user.address.city)
        printProduct({ id: 1, name: 'Pen', price: 2.5 })
        printProduct(item) // structural: Item has the same shape as Product
    "#;
    assert_eq!(
        run(source),
        "Exampleville\nProduct: Pen (1) - 2.5\nProduct: Cup (2) - 3.5\n"
    );
    assert_error(
        "interface U { id: int }\nu: U = { id: 1, extra: true }",
        "has no field 'extra'",
    );
}

// ---------------------------------------------------------------------------
// Functions
// ---------------------------------------------------------------------------

#[test]
fn functions() {
    let source = r#"
        function sum(a: int, b: int): int {
            return a + b
        }

        print(sum(2, 2)) // 4

        function factorial(x: int): int {
            if (x <= 1) { return 1 }

            return x * factorial(x - 1)
        }

        print(factorial(4)) // 24
    "#;
    assert_eq!(run(source), "4\n24\n");
    assert_error("function f(a: int): int { return a }\nf('x')", "must be int");
    assert_error("function f(): int {\n print(1)\n}", "must return a value of type int");
    assert_error("function f() { return 1 }", "cannot return a value");
}

#[test]
fn anonymous_functions_and_closures() {
    let source = r#"
        const double = (x: int): int => {
            return x * 2
        }

        // or simply

        const triple = (x: int) => x * 3

        print(double(10), triple(10)) // 20 30

        function makeCounter(): () => int {
            count = 0
            return () => {
                count += 1
                return count
            }
        }
        counter = makeCounter()
        counter()
        print(counter())

        apply = (f: (int) => int, x: int) => f(x)
        print(apply(double, 21))
    "#;
    assert_eq!(run(source), "20 30\n2\n42\n");
}

#[test]
fn functions_are_hoisted() {
    assert_eq!(run("print(later(1))\nfunction later(x: int): int { return x + 1 }"), "2\n");
}

// ---------------------------------------------------------------------------
// Modules
// ---------------------------------------------------------------------------

#[test]
fn modules_and_imports() {
    let dir = temp_dir("modules");
    std::fs::write(
        dir.join("math.ez"),
        r#"
        // file: math.ez

        function add(a: int, b: int): int {
            return a + b
        }

        function subtract(a: int, b: int): int {
            return a - b
        }

        type Shape = {
            Circle(radius: float)
            Square(side: float)
        }
        "#,
    )
    .unwrap();
    let source = r#"
        // file: main.ez

        import math from 'math.ez'
        import { add, subtract, Shape } from 'math.ez'

        result: int = math.add(5, 10)

        print(result) // 15
        print(add(5, 10), subtract(5, 10))
        s: Shape = Circle(1.0)
        t: math.Shape = math.Square(2.0)
        print(s, t)
    "#;
    let (output, result) = execute(source, "", &dir.join("main.ez"));
    result.unwrap();
    assert_eq!(output, "15\n15 -5\nCircle(1.0) Square(2.0)\n");

    let (_, result) = execute("import { nope } from 'math.ez'", "", &dir.join("main.ez"));
    assert!(result.unwrap_err().to_string().contains("has no member 'nope'"));
}

// ---------------------------------------------------------------------------
// Standard library
// ---------------------------------------------------------------------------

#[test]
fn stdlib_time() {
    let source = r#"
        import time

        print('Waiting...')

        time.sleep(0.01)

        print('Done!')

        start = time.monotonic()
        // Do something
        end = time.monotonic()

        print(end - start >= 0.0, time.now() > 1700000000.0)
    "#;
    assert_eq!(run(source), "Waiting...\nDone!\ntrue true\n");
}

#[test]
fn stdlib_file() {
    let dir = temp_dir("file");
    let path = dir.join("notes.txt").display().to_string().replace('\\', "/");
    let source = format!(
        r#"
        import file

        f = file.open('{path}', 'w')

        f.write('Hello world!')
        f.close()

        f = file.open('{path}', 'a')
        f.write(' Bye!')
        f.close()

        f = file.open('{path}', 'r')

        content = f.read()

        print(content)

        f.close()
        "#
    );
    assert_eq!(run(&source), "Hello world! Bye!\n");
    assert_error("import file\nf = file.open('/nonexistent/x.txt', 'r')", "cannot open");
}

#[test]
fn stdlib_datetime() {
    let source = r#"
        import datetime

        date = datetime.today()

        print(date.length())

        timestamp = datetime.parse('2025-04-20', 'YYYY-MM-DD')

        print(datetime.format(timestamp, 'DD/MM/YYYY'))
        print(datetime.format(timestamp + 3661, 'yyyy-mm-dd hh:min:ss'))

        now = datetime.now()
        print(now.year >= 2025, now.month >= 1 and now.month <= 12)
    "#;
    assert_eq!(run(source), "10\n20/04/2025\n2025-04-20 01:01:01\ntrue true\n");
}

#[test]
fn stdlib_math() {
    let source = r#"
        import math

        area: float = math.pi * math.pow(5, 2)

        print(area)
        print(math.abs(-3), math.abs(-2.5), math.sqrt(16), math.floor(2.7), math.ceil(2.1), math.round(2.5))
        print(math.e > 2.71)
    "#;
    assert_eq!(run(source), "78.53981633974483\n3 2.5 4.0 2 3 3\ntrue\n");
}

#[test]
fn stdlib_json() {
    let source = r#"
        import json

        text: string = '{"name": "ezlang", "year": 2025}'
        data = json.parse(text)

        print(data.name) // ezlang
        print(data.year + 1)

        jsonText = json.stringify(data)

        print(jsonText)
        print(json.stringify([1.5, 2.0]), json.stringify({ ok: true, tags: ['a'] }), json.stringify(None))
    "#;
    assert_eq!(
        run(source),
        "ezlang\n2026\n{\"name\":\"ezlang\",\"year\":2025}\n[1.5,2] {\"ok\":true,\"tags\":[\"a\"]} null\n"
    );
}

#[test]
fn stdlib_regex() {
    let source = r#"
        import regex

        print(regex.match('^[a-z]+$', 'hello')) // true
        print(regex.match('^[a-z]+$', '123')) // false

        result = regex.search('[0-9]+', 'Order number: 12345')

        if (result is Some) {
            print(result.value.value) // 12345
        }

        results = regex.findAll('[a-z]+', 'abc 123 def')

        for word in results {
            print(word.value)
        }

        print(regex.replace('[0-9]+', '#', 'abc123def456')) // abc#def#

        found = regex.search('abc', '123abc456')

        if (found is Some) {
            m = found.value

            print(m.value) // abc
            print(m.start) // 3
            print(m.end) // 6
        }
        print(regex.search('x', 'abc'))
    "#;
    assert_eq!(
        run(source),
        "true\nfalse\n12345\nabc\ndef\nabc#def#\nabc\n3\n6\nNone\n"
    );
    assert_error(
        "import regex\nresult = regex.search('a', 'a')\nprint(result.value)",
        "check the variant with 'is'",
    );
}

#[test]
fn stdlib_http() {
    use std::io::{Read, Write as _};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        for stream in listener.incoming().take(2) {
            let mut stream = stream.unwrap();
            let mut data = Vec::new();
            let mut buffer = [0u8; 4096];
            // Read the headers, then as many body bytes as Content-Length says.
            let request = loop {
                let read = stream.read(&mut buffer).unwrap();
                data.extend_from_slice(&buffer[..read]);
                let text = String::from_utf8_lossy(&data).to_string();
                if let Some((headers, body)) = text.split_once("\r\n\r\n") {
                    let length = headers
                        .lines()
                        .find_map(|l| l.to_lowercase().strip_prefix("content-length:").map(|v| v.trim().to_string()))
                        .and_then(|v| v.parse::<usize>().ok())
                        .unwrap_or(0);
                    if body.len() >= length || read == 0 {
                        break text;
                    }
                }
                if read == 0 {
                    break text;
                }
            };
            let first_line = request.lines().next().unwrap_or("").to_string();
            let body = request.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
            let reply = format!("{} {}", first_line, body);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                reply.len(),
                reply
            );
            stream.write_all(response.as_bytes()).unwrap();
        }
    });

    let source = format!(
        r#"
        import http

        res = http.get('http://127.0.0.1:{port}/api')

        if (res.status == 200) {{
            print(res.body)
        }}
        print(http.post('http://127.0.0.1:{port}/items', 'data').body)
        "#
    );
    assert_eq!(
        run(&source),
        "GET /api HTTP/1.1 \nPOST /items HTTP/1.1 data\n"
    );
    server.join().unwrap();
}

#[test]
fn all_examples_type_check() {
    for entry in std::fs::read_dir("examples").unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("ez") {
            continue;
        }
        if let Err(error) = ez::check_file(&path) {
            panic!("{}: {}", path.display(), error);
        }
    }
}
