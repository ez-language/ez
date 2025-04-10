#include <stdlib.h>
#include <string.h>
#include "parser.h"

static void advance(Parser* parser) {
    parser->previous = parser->current;
    parser->current = next_token(&parser->lexer);
}

static int match(Parser* parser, TokenType type) {
    if (parser->current.type == type) {
        advance(parser);
        return 1;
    }
    return 0;
}

static Expr* parse_primary(Parser* parser) {
    if (parser->current.type == TOKEN_NUMBER) {
        Expr* expr = malloc(sizeof(Expr));
        expr->type = EXPR_LITERAL;
        expr->literal = strdup(parser->current.value);
        advance(parser);
        return expr;
    }
    if (parser->current.type == TOKEN_IDENTIFIER) {
        Expr* expr = malloc(sizeof(Expr));
        expr->type = EXPR_VARIABLE;
        expr->variable.name = strdup(parser->current.value);
        advance(parser);
        return expr;
    }
    return NULL;
}

static Expr* parse_factor(Parser* parser) {
    Expr* expr = parse_primary(parser);
    while (parser->current.type == TOKEN_STAR || parser->current.type == TOKEN_SLASH) {
        Token operator = parser->current;
        advance(parser);
        Expr* right = parse_primary(parser);
        Expr* binary = malloc(sizeof(Expr));
        binary->type = EXPR_BINARY;
        binary->binary.left = expr;
        binary->binary.operator = operator;
        binary->binary.right = right;
        expr = binary;
    }
    return expr;
}

static Expr* parse_expression(Parser* parser) {
    Expr* expr = parse_factor(parser);
    while (parser->current.type == TOKEN_PLUS || parser->current.type == TOKEN_MINUS) {
        Token operator = parser->current;
        advance(parser);
        Expr* right = parse_factor(parser);
        Expr* binary = malloc(sizeof(Expr));
        binary->type = EXPR_BINARY;
        binary->binary.left = expr;
        binary->binary.operator = operator;
        binary->binary.right = right;
        expr = binary;
    }
    return expr;
}

Stmt* parse(const char* source) {
    Parser parser;
    parser.lexer = create_lexer(source);
    advance(&parser);

    if (match(&parser, TOKEN_LET)) {
        Token name = parser.current;
        advance(&parser);
        match(&parser, TOKEN_ASSIGN);
        Expr* value = parse_expression(&parser);

        Stmt* stmt = malloc(sizeof(Stmt));
        stmt->type = STMT_LET;
        stmt->let.name = strdup(name.value);
        stmt->let.value = value;
        return stmt;
    }

    return NULL;
}