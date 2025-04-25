#include <stdio.h>
#include "ast.h"

void print_expr(Expr* expr) {
    switch (expr->type) {
        case EXPR_LITERAL:
            printf("Literal: %s\n", expr->as.literal.literal);
            break;
        case EXPR_VARIABLE:
            printf("Variable: %s\n", expr->as.variable.name);
            break;
        case EXPR_BINARY:
            printf("Binary Expression: (operator %s)\n", expr->as.binary.operator.lexeme);
            print_expr(expr->as.binary.left);
            print_expr(expr->as.binary.right);
            break;
        case EXPR_PRINT:
            printf("Print Expression:\n");
            print_expr(expr->as.print.expression);
            break;
        case EXPR_ARRAY:
            printf("Array Literal with %d elements:\n", expr->as.array.count);
            for (int i = 0; i < expr->as.array.count; i++) {
                print_expr(expr->as.array.elements[i]);
            }
            break;
        case EXPR_CALL_METHOD:
            printf("Call Method: %s on\n", expr->as.call_method.method_name);
            print_expr(expr->as.call_method.receiver);
            if (expr->as.call_method.arg_count > 0) {
                printf(" With %d args:\n", expr->as.call_method.arg_count);
                for (int i = 0; i < expr->as.call_method.arg_count; i++) {
                    print_expr(expr->as.call_method.arguments[i]);
                }
            }
            break;
        default:
            printf("Unknown ExprType %d\n", expr->type);
    }
}

void print_stmt(Stmt* stmt) {
    switch (stmt->type) {
        case STMT_VAR:
            printf("Var Statement:\n");
            printf("  Name: %s\n", stmt->as.var.name);
            printf("  Value:\n");
            print_expr(stmt->as.var.value);
            break;
        case STMT_PRINT:
            printf("Print Statement:\n");
            print_expr(stmt->as.print.expression);
            break;
        default:
            printf("Unknown StmtType %d\n", stmt->type);
    }
}