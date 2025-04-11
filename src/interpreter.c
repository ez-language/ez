#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <stdbool.h>

#include "interpreter.h"
#include "environment.h"

Environment global_env = {0};

Value eval_expr(Expr* expr) {
    switch (expr->type) {
        case EXPR_LITERAL:
            if (strcmp(expr->literal, "true") == 0) {
                return (Value){ .type = VAL_BOOL, .boolean = true };
            } else if (strcmp(expr->literal, "false") == 0) {
                return (Value){ .type = VAL_BOOL, .boolean = false };
            } else if (expr->literal_type == LITERAL_NUMBER) {
                return (Value){ .type = VAL_NUMBER, .number = atof(expr->literal) };
            } else if (expr->literal_type == LITERAL_STRING) {
                return (Value){ .type = VAL_STRING, .string = expr->literal };
            } else {
                printf("[ERRO] Tipo de literal desconhecido\n");
                exit(1);
            }
        case EXPR_VARIABLE:
            return env_get(&global_env, expr->variable.name);
        case EXPR_PRINT:
            Value result = eval_expr(expr->print.expression);
            print_value(result);  // Imprime o valor
            return result;
        case EXPR_BINARY: {
            Value left = eval_expr(expr->binary.left);
            Value right = eval_expr(expr->binary.right);
            switch (expr->binary.operator.type) {
                case TOKEN_PLUS:
                    if (left.type == VAL_STRING && right.type == VAL_STRING) {
                        size_t len = strlen(left.string) + strlen(right.string) + 1;
                        char *concat = malloc(len);
                        strcpy(concat, left.string);
                        strcat(concat, right.string);
                        return (Value){ .type = VAL_STRING, .string = concat };
                    } else if (left.type == VAL_NUMBER && right.type == VAL_NUMBER) {
                        return (Value){ .type = VAL_NUMBER, .number = left.number + right.number };
                    }
                    break;
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

void print_value(Value value) {
    switch (value.type) {
        case VAL_NUMBER:
            printf("%f\n", value.number);
            break;
        case VAL_STRING:
            printf("%s\n", value.string);
            break;
        case VAL_BOOL:
            printf(value.boolean ? "true\n" : "false\n");
            break;
        default:
            printf("[tipo desconhecido]\n");
    }
}

void exec_stmt(Stmt* stmt) {
    switch (stmt->type) {
        case STMT_LET: {
            Value value = eval_expr(stmt->let.value);
            env_define(&global_env, stmt->let.name, value);

            printf("[let] %s = ", stmt->let.name);
            print_value(value);

            break;
        }
        case STMT_PRINT: {
            Value result = eval_expr(stmt->print.expression);
            print_value(result);
            break;
        }
    }
}

void interpret(StmtList *stmts) {
    for (size_t i = 0; i < stmts->count; i++) {
        exec_stmt(stmts->statements[i]);
    }
}
