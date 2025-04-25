#include <stdlib.h>
#include <stdio.h>

#include "parser.h"

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

    printf("Consumed token: %s\n", token.lexeme);

    printf("[ERROR] %s\n", message);
    exit(1);
}

static Expr* parse_primary(Parser* parser) {
    Expr* expr = malloc(sizeof(Expr));

    switch (parser->current.type) {
        case TOKEN_NUMBER:
            expr->type = EXPR_LITERAL;
            expr->literal = parser->current.lexeme;
            expr->literal_type = LITERAL_NUMBER;
            break;

        case TOKEN_STRING:
            expr->type = EXPR_LITERAL;
            expr->literal = parser->current.lexeme;
            expr->literal_type = LITERAL_STRING;
            break;

        case TOKEN_IDENTIFIER:
            expr->type = EXPR_VARIABLE;
            expr->variable.name = parser->current.lexeme;
            break;

        default:
            printf("[ERROR] Invalid primary expression\n");
            exit(1);
    }

    advance(parser);

    if (match(parser, TOKEN_DOT)) {
        Token method_name = consume(parser, TOKEN_IDENTIFIER, "Expected method name after '.'");

        consume(parser, TOKEN_LPAREN, "Expected '(' after method name");

        Expr** arguments = NULL;
        int arg_count = 0;

        consume(parser, TOKEN_RPAREN, "Expected ')' after method call");

        Expr* call_expr = malloc(sizeof(Expr));
        call_expr->type = EXPR_CALL_METHOD;
        call_expr->call_method.receiver = expr;
        call_expr->call_method.method_name = method_name.lexeme;
        call_expr->call_method.arguments = arguments;
        call_expr->call_method.arg_count = arg_count;

        return call_expr;
    }

    return expr;
}

static Expr* parse_expression(Parser* parser) {
    return parse_primary(parser);
}

static Stmt* parse_print_stmt(Parser* parser) {
    advance(parser);

    consume(parser, TOKEN_LPAREN, "Expected '(' after 'print'");
    Expr* expression = parse_expression(parser);
    consume(parser, TOKEN_RPAREN, "Expected ')' after 'print' expression");

    Stmt* stmt = malloc(sizeof(Stmt));
    stmt->type = STMT_PRINT;
    stmt->print.expression = expression;
    
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
    
            match(&parser, TOKEN_COLON);
            advance(&parser);
    
            match(&parser, TOKEN_ASSIGN);
            Expr *value = parse_expression(&parser);
    
            Stmt *stmt = malloc(sizeof(Stmt));
            stmt->type = STMT_VAR;
            stmt->var.name = name.lexeme;
            stmt->var.value = value;
    
            if (list.count >= list.capacity) {
                list.capacity = list.capacity < 8 ? 8 : list.capacity * 2;
                list.statements = realloc(list.statements, list.capacity * sizeof(Stmt *));
            }
            list.statements[list.count++] = stmt;
        } else if (parser.current.type == TOKEN_PRINT) {
            Stmt* stmt = parse_print_stmt(&parser);
    
            if (list.count >= list.capacity) {
                list.capacity = list.capacity < 8 ? 8 : list.capacity * 2;
                list.statements = realloc(list.statements, list.capacity * sizeof(Stmt *));
            }

            list.statements[list.count++] = stmt;
        } else {
            printf("[ERROR] Expected 'var' or 'print'\n");
            exit(1);
        }
    }    

    return list;
}
