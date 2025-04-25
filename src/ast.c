#include <stdio.h>

#include "ast.h"

void print_expr(Expr* expr) {
    switch (expr->type) {
        case EXPR_LITERAL:
            printf("Literal: %s\n", expr->literal);
            break;
        case EXPR_VARIABLE:
            printf("Variable: %s\n", expr->variable.name);
            break;
        case EXPR_CALL_METHOD:
            printf("Call Method: %s on ", expr->call_method.method_name);
            print_expr(expr->call_method.receiver);
            break;
        default:
            printf("Unknown Expression Type\n");
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
