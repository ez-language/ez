#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include "interpreter.h"
#include "lexer.h"

Value eval_expr(Expr* expr) {
    switch (expr->type) {
        case EXPR_LITERAL:
            return (Value){ .type = VAL_NUMBER, .number = atof(expr->literal) };
        case EXPR_VARIABLE:
            printf("[ERRO] Variáveis ainda não implementadas\n");
            exit(1);
        case EXPR_BINARY: {
            Value left = eval_expr(expr->binary.left);
            Value right = eval_expr(expr->binary.right);
            switch (expr->binary.operator.type) {
                case TOKEN_PLUS:
                    return (Value){ .type = VAL_NUMBER, .number = left.number + right.number };
                case TOKEN_MINUS:
                    return (Value){ .type = VAL_NUMBER, .number = left.number - right.number };
                case TOKEN_STAR:
                    return (Value){ .type = VAL_NUMBER, .number = left.number * right.number };
                case TOKEN_SLASH:
                    return (Value){ .type = VAL_NUMBER, .number = left.number / right.number };
                default:
                    printf("[ERRO] Operador binário inválido\n");
                    exit(1);
            }
        }
    }
    printf("[ERRO] Tipo de expressão inválido\n");
    exit(1);
}
