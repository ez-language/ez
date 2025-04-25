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
    EXPR_CALL_METHOD
} ExprType;

// Declarar o tipo Expr antes de usá-lo
typedef struct Expr Expr;

typedef struct {
    Expr* receiver;
    const char* method_name;
    int arg_count;
    Expr** arguments;
} CallMethodExpr;

typedef enum {
    LITERAL_NUMBER,
    LITERAL_STRING
} LiteralType;

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
        CallMethodExpr call_method;
    };
};

typedef enum {
    STMT_VAR,
    STMT_PRINT
} StmtType;

typedef struct {
    const char *name;
    Expr *value;
} VarStmt;

typedef struct {
    Expr* expression;
} PrintStmt;

typedef struct Stmt {
    StmtType type;
    union {
        VarStmt var;
        PrintStmt print;
    };
} Stmt;

#endif
