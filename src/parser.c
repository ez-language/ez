#include <stdlib.h>
#include <string.h>
#include <stdio.h>

#include "parser.h"

// Forward declaration
static Expr* parse_expression(Parser* parser);

static void advance(Parser *parser) {
    parser->previous = parser->current;
    parser->current = lexer_next_token(&parser->lexer);
}

static int match(Parser *parser, TokenType type) {
    if (parser->current.type == type) {
        advance(parser);
        return 1;
    }
    return 0;
}

Token consume(Parser* parser, TokenType type, const char* message) {
    Token token = parser->current;

    if (parser->current.type == type) {
        advance(parser);
        return token;
    }

    printf("[ERROR] %s\n", message);
    exit(1);
}

static Expr* parse_primary(Parser* parser) {
    Expr* expr = malloc(sizeof(Expr));
    switch (parser->current.type) {
        case TOKEN_NUMBER:
            expr->type = EXPR_LITERAL;
            expr->as.literal.literal = parser->current.lexeme;
            expr->as.literal.literal_type = LITERAL_NUMBER;
            break;
        case TOKEN_STRING:
            expr->type = EXPR_LITERAL;
            expr->as.literal.literal = parser->current.lexeme;
            expr->as.literal.literal_type = LITERAL_STRING;
            break;
        case TOKEN_IDENTIFIER:
            expr->type = EXPR_VARIABLE;
            expr->as.variable.name = parser->current.lexeme;
            break;
        case TOKEN_LBRACKET: {
            advance(parser); // consume '['
            Expr** elements = NULL;
            int count = 0, capacity = 0;
            while (parser->current.type != TOKEN_RBRACKET) {
                if (count >= capacity) {
                    capacity = capacity < 8 ? 8 : capacity * 2;
                    elements = realloc(elements, sizeof(Expr*) * capacity);
                }
                elements[count++] = parse_expression(parser);
                if (!match(parser, TOKEN_COMMA)) break;
            }
            consume(parser, TOKEN_RBRACKET, "Expected ']' after array literal");
            expr->type = EXPR_ARRAY;
            expr->as.array.elements = elements;
            expr->as.array.count = count;
            return expr;
        }
        default:
            printf("[ERROR] Invalid primary expression\n");
            exit(1);
    }
    advance(parser);
    // Method call
    if (match(parser, TOKEN_DOT)) {
        Token method_name = consume(parser, TOKEN_IDENTIFIER, "Expected method name after '.'");
        consume(parser, TOKEN_LPAREN, "Expected '(' after method name");
        Expr** arguments = NULL;
        int arg_count = 0, arg_cap = 0;
        while (!match(parser, TOKEN_RPAREN)) {
            if (arg_count >= arg_cap) {
                arg_cap = arg_cap < 4 ? 4 : arg_cap * 2;
                arguments = realloc(arguments, sizeof(Expr*) * arg_cap);
            }
            arguments[arg_count++] = parse_expression(parser);
            if (!match(parser, TOKEN_COMMA)) break;
        }
        Expr* call_expr = malloc(sizeof(Expr));
        call_expr->type = EXPR_CALL_METHOD;
        call_expr->as.call_method.receiver = expr;
        call_expr->as.call_method.method_name = method_name.lexeme;
        call_expr->as.call_method.arguments = arguments;
        call_expr->as.call_method.arg_count = arg_count;
        return call_expr;
    }
    return expr;
}

static Expr* parse_expression(Parser* parser) {
    return parse_primary(parser);
}

static Stmt* parse_print_stmt(Parser* parser) {
    advance(parser); // consume 'print'
    consume(parser, TOKEN_LPAREN, "Expected '(' after 'print'");
    Expr* expression = parse_expression(parser);
    consume(parser, TOKEN_RPAREN, "Expected ')' after print expression");
    Stmt* stmt = malloc(sizeof(Stmt));
    stmt->type = STMT_PRINT;
    stmt->as.print.expression = expression;
    return stmt;
}

StmtList parse(Lexer *lexer) {
    Parser parser;
    parser.lexer = *lexer;
    advance(&parser);
    StmtList list = {0};
    while (parser.current.type != TOKEN_EOF) {
        if (match(&parser, TOKEN_VAR)) {
            Token name = parser.current;
            advance(&parser);
            consume(&parser, TOKEN_COLON, "Expected ':' after var name");
            advance(&parser); // ignore type annotation
            consume(&parser, TOKEN_ASSIGN, "Expected '=' after variable declaration");
            Expr* value = parse_expression(&parser);
            Stmt* stmt = malloc(sizeof(Stmt));
            stmt->type = STMT_VAR;
            stmt->as.var.name = name.lexeme;
            stmt->as.var.value = value;
            if (list.count >= list.capacity) {
                list.capacity = list.capacity < 8 ? 8 : list.capacity * 2;
                list.statements = realloc(list.statements, sizeof(Stmt*) * list.capacity);
            }
            list.statements[list.count++] = stmt;
        } else if (parser.current.type == TOKEN_PRINT) {
            Stmt* stmt = parse_print_stmt(&parser);
            if (list.count >= list.capacity) {
                list.capacity = list.capacity < 8 ? 8 : list.capacity * 2;
                list.statements = realloc(list.statements, sizeof(Stmt*) * list.capacity);
            }
            list.statements[list.count++] = stmt;
        } else {
            printf("[ERROR] Expected 'var' or 'print'\n");
            exit(1);
        }
    }
    return list;
}