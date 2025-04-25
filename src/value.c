#include <stdlib.h>
#include <string.h>
#include <stdio.h>

#include "value.h"

Value* alloc_number(double num) {
    Value* val = malloc(sizeof(Value));
    if (!val) exit(1);
    val->type = VAL_NUMBER;
    val->number = num;
    return val;
}

Value* alloc_string(const char* chars) {
    if (!chars) return NULL;
    size_t len = strlen(chars);
    if (len < 2 || chars[0] != '"' || chars[len - 1] != '"') {
        String* raw = malloc(sizeof(String));
        if (!raw) exit(1);
        raw->length = len;
        raw->chars = strdup(chars);

        Value* v = malloc(sizeof(Value));
        if (!v) exit(1);
        v->type = VAL_STRING;
        v->string = raw;
        return v;
    }

    const char* src = chars + 1;
    const char* end = chars + len - 1;
    char* buf = malloc(len);
    if (!buf) exit(1);

    size_t di = 0;
    for (const char* p = src; p < end; ++p) {
        if (*p == '\\' && (p + 1) < end) {
            ++p;
            switch (*p) {
                case 'n': buf[di++] = '\n'; break;
                case 't': buf[di++] = '\t'; break;
                case '\\': buf[di++] = '\\'; break;
                case '"': buf[di++] = '"'; break;
                default: buf[di++] = *p; break;
            }
        } else {
            buf[di++] = *p;
        }
    }
    buf[di] = '\0';

    String* str = malloc(sizeof(String));
    if (!str) exit(1);
    str->length = di;
    str->chars = buf;

    Value* v = malloc(sizeof(Value));
    if (!v) exit(1);
    v->type = VAL_STRING;
    v->string = str;
    return v;
}

Value* alloc_array(size_t capacity) {
    Value* val = malloc(sizeof(Value));
    if (!val) exit(1);
    val->type = VAL_ARRAY;

    Array* arr = malloc(sizeof(Array));
    if (!arr) exit(1);
    arr->length = 0;
    arr->capacity = capacity;
    arr->items = malloc(sizeof(Value*) * capacity);
    if (!arr->items) exit(1);

    val->array = arr;
    return val;
}

Value* native_length(Value* receiver, int arg_count, Value** args) {
    (void)args;
    if (arg_count != 0) return NULL;
    switch (receiver->type) {
        case VAL_STRING:
            return alloc_number((double)receiver->string->length);
        case VAL_ARRAY:
            return alloc_number((double)receiver->array->length);
        default:
            return NULL;
    }
}
