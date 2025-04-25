#include <stdlib.h>
#include <string.h>

#include "value.h"

Value* alloc_number(double num) {
    Value* val = malloc(sizeof(Value));
    val->type = VAL_NUMBER;
    val->number = num;
    return val;
}

Value* alloc_string(const char* chars) {
    Value* val = malloc(sizeof(Value));
    val->type = VAL_STRING;

    String* str = malloc(sizeof(String));
    str->length = strlen(chars);
    str->chars = malloc(str->length + 1);
    memcpy((char*)str->chars, chars, str->length + 1);

    val->string = str;
    return val;
}

Value* alloc_array(size_t capacity) {
    Value* val = malloc(sizeof(Value));
    val->type = VAL_ARRAY;

    Array* arr = malloc(sizeof(Array));
    arr->length = 0;
    arr->capacity = capacity;
    arr->items = malloc(sizeof(Value*) * capacity);

    val->array = arr;
    return val;
}

Value* native_length(Value* receiver, int arg_count, Value** args) {
    (void)args;
    
    if (arg_count != 0) return NULL;

    switch (receiver->type) {
        case VAL_STRING:
            return alloc_number((int)receiver->string->length);
        case VAL_ARRAY:
            return alloc_number((int)receiver->array->length);
        default:
            return NULL;
    }
}

