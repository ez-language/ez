use std::fs;

fn read_example(name: &str) -> String {
    fs::read_to_string(format!("examples/{}.ez", name)).expect("failed to read example")
}

#[test]
fn test_basic_tokens() {
    let input = read_example("basic");
    let tokens = lex_all(&input);
}
