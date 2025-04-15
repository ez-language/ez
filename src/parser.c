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

    printf("Token consumido: %s\n", token.lexeme);

    printf("[ERRO] %s\n", message);
    exit(1);
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

        case TOKEN_PRINT:
            expr->type = EXPR_PRINT;
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

static Stmt* parse_print_stmt(Parser* parser) {
    advance(parser);

    consume(parser, TOKEN_LPAREN, "Esperado '(' após 'print'.");
    Expr* expression = parse_expression(parser);
    consume(parser, TOKEN_RPAREN, "Esperado ')' após expressão do 'print'.");

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
            advance(&parser); // nome da variável
    
            match(&parser, TOKEN_COLON);
            advance(&parser); // tipo (ignorado por enquanto)
    
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
            printf("[ERRO] Esperado 'var' ou 'print'\n");
            exit(1);
        }
    }    

    return list;
}
