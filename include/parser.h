#ifndef PARSER_H
#define PARSER_H

#include "lexer.h"
#include "ast.h"

typedef struct {
    Token current;
    Token previous;
    Lexer lexer;
} Parser;

Stmt* parse(const char *source);

#endif
