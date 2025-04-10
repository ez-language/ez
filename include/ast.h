#ifndef AST_H
#define AST_H

#include <stdlib.h>
#include "lexer.h"

typedef struct Stmt Stmt;

typedef struct {
    Stmt** statements;
    size_t count;
    size_t capacity;
} StmtList;

typedef enum {
    EXPR_LITERAL,
    EXPR_BINARY,
    EXPR_VARIABLE,
    EXPR_PRINT,
} ExprType;

typedef enum {
    LITERAL_NUMBER,
    LITERAL_STRING
} LiteralType;

// ast.h
typedef struct Expr Expr;  // Declaração do tipo Expr

struct Expr {
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
        struct {
            Expr* expression;
        } print;
    };
};

typedef enum {
    STMT_LET,
    STMT_PRINT
} StmtType;

typedef struct {
    const char *name;
    Expr *value;
} LetStmt;

typedef struct {
    Expr* expression;
} PrintStmt;

typedef struct Stmt {
    StmtType type;
    union {
        LetStmt let;
        PrintStmt print;
    };
} Stmt;

#endif
