use ez::lexer::{token::Token, Lexer};
use std::fs;

fn read_example(name: &str) -> String {
    fs::read_to_string(format!("examples/{}.ez", name)).expect("failed to read example")
}

fn lex_all(input: &str) -> Vec<Token> {
    Lexer::new(input).tokenize()
}

#[test]
fn test_basic_tokens() {
    let input = read_example("hello");
    let tokens = lex_all(&input);

    assert_eq!(tokens.last(), Some(&Token::EOF));
    assert!(!tokens.iter().any(|t| matches!(t, Token::Illegal(_))));
}

#[test]
fn test_all_examples_parse() {
    for entry in fs::read_dir("examples").unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("ez") {
            continue;
        }
        let source = fs::read_to_string(&path).unwrap();
        let tokens = Lexer::new(&source).tokenize();
        let result = ez::parser::parse_program(&tokens);
        assert!(result.is_ok(), "{}: {:?}", path.display(), result.err());
    }
}