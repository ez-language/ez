#ifndef PARSER_H
#define PARSER_H

#include "lexer.h"
#include "ast.h"

typedef struct {
    Lexer lexer;
    Token current;
    Token previous;
} Parser;

typedef struct {
    Stmt **statements;
    size_t count;
    size_t capacity;
} StmtList;

StmtList parse(Lexer *lexer);

#endif
