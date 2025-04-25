// include/ast.h
#ifndef AST_H
#define AST_H

#include <stdlib.h>
#include "lexer.h"

// Forward declarations
typedef struct Expr Expr;
typedef struct Stmt Stmt;

typedef struct {
    Stmt** statements;
    size_t count;
    size_t capacity;
} StmtList;

enum ExprType {
    EXPR_LITERAL,
    EXPR_VARIABLE,
    EXPR_PRINT,
    EXPR_BINARY,
    EXPR_ARRAY,
    EXPR_CALL_METHOD
};

enum LiteralType {
    LITERAL_NUMBER,
    LITERAL_STRING
};

// AST: chamada de método
typedef struct {
    Expr* receiver;
    const char* method_name;
    Expr** arguments;
    int arg_count;
} CallMethodExpr;

// AST: array literal
typedef struct {
    Expr** elements;
    int count;
} ArrayExpr;

// Estrutura principal de expressão
struct Expr {
    enum ExprType type;
    union {
        struct {
            const char* literal;
            enum LiteralType literal_type;
        } literal;
        struct {
            const char* name;
        } variable;
        struct {
            Expr* expression;
        } print;
        struct {
            Expr* left;
            Token operator;
            Expr* right;
        } binary;
        ArrayExpr array;
        CallMethodExpr call_method;
    } as;
};

// Statements
enum StmtType {
    STMT_VAR,
    STMT_PRINT
};

typedef struct {
    const char* name;
    Expr* value;
} VarStmt;

typedef struct {
    Expr* expression;
} PrintStmt;

struct Stmt {
    enum StmtType type;
    union {
        VarStmt var;
        PrintStmt print;
    } as;
};

#endif // AST_H
