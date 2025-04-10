#ifndef LEXER_H
#define LEXER_H

#include "token.h"

typedef struct {
    const char *source;
    int position;
} Lexer;

Lexer lexer_init(const char *source);
Token lexer_next_token(Lexer *lexer);
void lex(const char *source);

#endif