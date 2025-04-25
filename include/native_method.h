#ifndef NATIVE_METHOD_H
#define NATIVE_METHOD_H

#include <string.h>

#include "value.h"

typedef Value* (*NativeMethod)(Value* receiver, int arg_count, Value** args);

typedef struct {
    const char* name;
    NativeMethod function;
} MethodEntry;

NativeMethod get_native_method(Value* receiver, const char* name);

void init_native_methods();

#endif
