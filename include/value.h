#ifndef VALUE_H
#define VALUE_H

#include <stdbool.h>

typedef enum {
    VAL_NUMBER,
    VAL_STRING,
    VAL_BOOL
} ValueType;

typedef struct {
    ValueType type;
    union {
        double number;
        const char *string;
        bool boolean;
    };
} Value;

#endif