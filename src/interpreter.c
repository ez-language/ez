
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <stdbool.h>

#include "interpreter.h"
#include "environment.h"
#include "native_method.h"

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
                String* new_string = malloc(sizeof(String));
                new_string->chars = strdup(expr->literal);
                new_string->length = strlen(new_string->chars);
                return (Value){ .type = VAL_STRING, .string = new_string };
            } else {
                printf("[ERROR] Unknown literal type\n");
                exit(1);
            }
        case EXPR_VARIABLE:
            return env_get(&global_env, expr->variable.name);
        case EXPR_PRINT:
            Value result = eval_expr(expr->print.expression);
            print_value(result);
            return result;
        case EXPR_BINARY: {
            Value left = eval_expr(expr->binary.left);
            Value right = eval_expr(expr->binary.right);
            switch (expr->binary.operator.type) {
                case TOKEN_PLUS:
                    if (left.type == VAL_STRING && right.type == VAL_STRING) {
                        size_t len = left.string->length + right.string->length + 1;
                        char *concat = malloc(len);
                        strcpy(concat, left.string->chars);
                        strcat(concat, right.string->chars);

                        String *result_string = malloc(sizeof(String));
                        result_string->chars = concat;
                        result_string->length = len - 1;

                        return (Value){ .type = VAL_STRING, .string = result_string };
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
                    printf("[ERROR] Invalid binary operator\n");
                    exit(1);
            }
            exit(1);
        }
        case EXPR_CALL_METHOD: {
            Value receiver = eval_expr(expr->call_method.receiver);
            const char* method_name = expr->call_method.method_name;
        
            Value* args = NULL;
            int arg_count = expr->call_method.arg_count;
        
            NativeMethod method = get_native_method(&receiver, method_name);

            if (method != NULL) {
                return *method(&receiver, arg_count, &args);
            } else {
                printf("[ERROR] Method '%s' not defined for this type.\n", method_name);
                exit(1);
            }
        }        
    }

    printf("[ERROR] Invalid primary expression\n");
    exit(1);
}

void print_value(Value value) {
    switch (value.type) {
        case VAL_NUMBER:
            printf("%f", value.number);
            break;
        case VAL_STRING:
            printf("%s", value.string->chars);
            break;
        case VAL_BOOL:
            printf(value.boolean ? "true" : "false");
            break;
        default:
            printf("[unknown type]");
    }
}

void exec_stmt(Stmt* stmt) {
    switch (stmt->type) {
        case STMT_VAR: {
            Value value = eval_expr(stmt->var.value);
            env_define(&global_env, stmt->var.name, value);

            printf("[var] %s = ", stmt->var.name);
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
