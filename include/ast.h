#ifndef AST_H
#define AST_H

#include <stdlib.h>
#include "lexer.h"

typedef enum {
    EXPR_LITERAL,
    EXPR_BINARY,
    EXPR_VARIABLE
} ExprType;

typedef enum {
    LITERAL_NUMBER,
    LITERAL_STRING
} LiteralType;

typedef struct Expr {
    ExprType type;
    union {
        struct {
            const char *literal;
            LiteralType literal_type;
        };
        struct {
            struct Expr *left;
            Token operator;
            struct Expr *right;
        } binary;
        struct {
            const char *name;
        } variable;
    };
} Expr;

typedef enum {
    STMT_LET
} StmtType;

typedef struct {
    const char *name;
    Expr *value;
} LetStmt;

typedef struct Stmt {
    StmtType type;
    union {
        LetStmt let;
    };
} Stmt;

#endif
