#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <stdbool.h>

#include "interpreter.h"
#include "environment.h"
#include "native_method.h"

HashTable global_env;

Value* eval_expr(Expr* expr) {
    switch (expr->type) {
        case EXPR_LITERAL: {
            if (strcmp(expr->as.literal.literal, "true") == 0) {
                Value* v = malloc(sizeof(Value));
                v->type = VAL_BOOL;
                v->boolean = true;
                return v;
            } else if (strcmp(expr->as.literal.literal, "false") == 0) {
                Value* v = malloc(sizeof(Value));
                v->type = VAL_BOOL;
                v->boolean = false;
                return v;
            } else if (expr->as.literal.literal_type == LITERAL_NUMBER) {
                return alloc_number(atof(expr->as.literal.literal));
            } else {
                return alloc_string(expr->as.literal.literal);
            }
        }
        case EXPR_VARIABLE:
            return env_get(&global_env, expr->as.variable.name);
        case EXPR_PRINT: {
            Value* res = eval_expr(expr->as.print.expression);
            print_value(*res);
            return res;
        }
        case EXPR_BINARY: {
            Value* L = eval_expr(expr->as.binary.left);
            Value* R = eval_expr(expr->as.binary.right);
            switch (expr->as.binary.operator.type) {
                case TOKEN_PLUS:
                    if (L->type == VAL_STRING && R->type == VAL_STRING) {
                        size_t ln = L->string->length + R->string->length + 1;
                        char* buf = malloc(ln);
                        strcpy(buf, L->string->chars);
                        strcat(buf, R->string->chars);
                        return alloc_string(buf);
                    }
                    return alloc_number(L->number + R->number);
                case TOKEN_MINUS: return alloc_number(L->number - R->number);
                case TOKEN_STAR:  return alloc_number(L->number * R->number);
                case TOKEN_SLASH: return alloc_number(L->number / R->number);
                default:          exit(1);
            }
        }
        case EXPR_ARRAY: {
            Value* arr = alloc_array(expr->as.array.count);
            for (int i = 0; i < expr->as.array.count; i++) {
                arr->array->items[arr->array->length++] = eval_expr(expr->as.array.elements[i]);
            }
            return arr;
        }
        case EXPR_CALL_METHOD: {
            Value* recv = eval_expr(expr->as.call_method.receiver);
            int n = expr->as.call_method.arg_count;
            Expr** args_e = expr->as.call_method.arguments;
            Value** args = malloc(sizeof(Value*) * n);
            for (int i = 0; i < n; i++) args[i] = eval_expr(args_e[i]);
            NativeMethod m = get_native_method(recv, expr->as.call_method.method_name);
            if (!m) {
                printf("[ERROR] undefined method '%s'\\n", expr->as.call_method.method_name);
                exit(1);
            }
            return m(recv, n, args);
        }
    }
    printf("[ERROR] Invalid primary expression\\n");
    exit(1);
}

void print_value(Value v) {
    switch (v.type) {
        case VAL_NULL:
            printf("null\\n");
            break;
        case VAL_NUMBER:
            if ((int)v.number == v.number) printf("%d\\n", (int)v.number);
            else                          printf("%f\\n", v.number);
            break;
        case VAL_STRING:
            printf("%s\\n", v.string->chars);
            break;
        case VAL_BOOL:
            printf(v.boolean ? "true\\n" : "false\\n");
            break;
        case VAL_ARRAY:
            printf("[array length: %zu]\\n", v.array->length);
            break;
        default:
            printf("[unknown type]\\n");
    }
}

void exec_stmt(Stmt* s) {
    switch (s->type) {
        case STMT_VAR: {
            Value* val = eval_expr(s->as.var.value);
            env_define(&global_env, s->as.var.name, *val);
            printf("[var] %s = ", s->as.var.name);
            print_value(*val);
            break;
        }
        case STMT_PRINT: {
            Value* res = eval_expr(s->as.print.expression);
            print_value(*res);
            break;
        }
    }
}

void interpret(StmtList* stmts) {
    env_init(&global_env);
    for (size_t i = 0; i < stmts->count; i++) {
        exec_stmt(stmts->statements[i]);
    }
}
