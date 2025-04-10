#include <stdlib.h>
#include <stdio.h>
#include "parser.h"
#include "token.h"
#include "ast.h"

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

static Expr* parse_primary(Parser* parser) {
    Expr* expr = malloc(sizeof(Expr));

    switch (parser->current.type) {
        case TOKEN_NUMBER:
            expr->type = EXPR_LITERAL;
            expr->literal = parser->current.lexeme;  // lexeme, não literal
            expr->literal_type = LITERAL_NUMBER;
            break;

        case TOKEN_STRING:
            expr->type = EXPR_LITERAL;
            expr->literal = parser->current.lexeme;  // lexeme, não literal
            expr->literal_type = LITERAL_STRING;
            break;

        case TOKEN_IDENTIFIER:
            expr->type = EXPR_VARIABLE;
            expr->variable.name = parser->current.lexeme;
            break;

        default:
            printf("[ERRO] Expressão primária inválida\n");
            exit(1);
    }

    advance(parser);
    return expr;
}

static Expr* parse_expression(Parser *parser) {
    return parse_primary(parser);
}

StmtList parse(Lexer *lexer) {
    Parser parser;
    parser.lexer = *lexer;
    advance(&parser);

    StmtList list = {0};

    while (parser.current.type != TOKEN_EOF) {
        if (match(&parser, TOKEN_LET)) {
            Token name = parser.current;
            advance(&parser); // nome da variável

            match(&parser, TOKEN_COLON);
            advance(&parser); // tipo

            match(&parser, TOKEN_ASSIGN);
            Expr *value = parse_expression(&parser);

            Stmt *stmt = malloc(sizeof(Stmt));
            stmt->type = STMT_LET;
            stmt->let.name = name.lexeme;
            stmt->let.value = value;

            if (list.count >= list.capacity) {
                list.capacity = list.capacity < 8 ? 8 : list.capacity * 2;
                list.statements = realloc(list.statements, list.capacity * sizeof(Stmt *));
            }
            list.statements[list.count++] = stmt;
        } else {
            printf("[ERRO] Esperado 'let'\n");
            exit(1);
        }
    }

    return list;
}
