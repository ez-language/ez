#ifndef PARSER_H
#define PARSER_H

#include "lexer.h"
#include "ast.h"

typedef struct {
    Lexer lexer;
    Token current;
    Token previous;
} Parser;

Token consume(Parser* parser, TokenType type, const char* message);
StmtList parse(Lexer *lexer);

#endif
