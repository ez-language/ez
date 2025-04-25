#ifndef VALUE_H
#define VALUE_H

#include <stdbool.h>
#include <stddef.h>

typedef enum {
    VAL_NULL,
    VAL_BOOL,
    VAL_NUMBER,
    VAL_STRING,
    VAL_ARRAY
} ValueType;

typedef struct {
    const char* chars;
    size_t length;
} String;

typedef struct Value Value;

typedef struct {
    Value** items;
    size_t length;
    size_t capacity;
} Array;

struct Value {
    ValueType type;
    union {
        bool boolean;
        double number;
        String* string;
        Array* array;
    };
};

Value* alloc_number(double num);
Value* alloc_string(const char* chars);
Value* alloc_array(size_t capacity);

Value* native_length(Value* receiver, int arg_count, Value** args);

#endif