#ifndef INTERPRETER_H
#define INTERPRETER_H

#include "parser.h"

typedef enum {
    VAL_NUMBER,
    VAL_STRING
} ValueType;

typedef struct {
    ValueType type;
    union {
        double number;
        const char *string;
    };
} Value;

void interpret(StmtList *stmts);

#endif
