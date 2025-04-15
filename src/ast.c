#include <stdio.h>

#include "ast.h"

void print_expr(Expr *expr) {
    if (expr->type == EXPR_LITERAL) {
        printf("Literal: %s\n", expr->literal);
    }
}

void print_stmt(Stmt *stmt) {
    if (stmt->type == STMT_VAR) {
        printf("VarStatement:\n");
        printf("  Name: %s\n", stmt->var.name);
        printf("  Value:\n");
        print_expr(stmt->var.value);
    }
}
