#include <string.h>

#include "native_method.h"

static MethodEntry string_methods[] = {
    {"length", native_length},
    {NULL, NULL}
};

static MethodEntry array_methods[] = {
    {"length", native_length},
    {NULL, NULL}
};

NativeMethod get_native_method(Value* receiver, const char* name) {
    MethodEntry* methods = NULL;

    switch (receiver->type) {
        case VAL_STRING:
            methods = string_methods;
            break;
        case VAL_ARRAY:
            methods = array_methods;
            break;
        default:
            return NULL;
    }

    for (int i = 0; methods[i].name != NULL; i++) {
        if (strcmp(methods[i].name, name) == 0) {
            return methods[i].function;
        }
    }

    return NULL;
}
