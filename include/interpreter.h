#ifndef INTERPRETER_H
#define INTERPRETER_H

#include "lexer.h"
#include "ast.h"

typedef enum {
    VAL_NUMBER
} ValueType;

typedef struct {
    ValueType type;
    double number;
} Value;

Value eval_expr(Expr* expr);

#endif