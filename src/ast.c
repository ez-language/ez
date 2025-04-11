#include <stdio.h>

#include "ast.h"

void print_expr(Expr *expr) {
    if (expr->type == EXPR_LITERAL) {
        printf("Literal: %s\n", expr->literal);
    }
}

void print_stmt(Stmt *stmt) {
    if (stmt->type == STMT_LET) {
        printf("LetStatement:\n");
        printf("  Name: %s\n", stmt->let.name);
        printf("  Value:\n");
        print_expr(stmt->let.value);
    }
}
