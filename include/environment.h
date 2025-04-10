#ifndef ENVIRONMENT_H
#define ENVIRONMENT_H

#include "interpreter.h"

#define MAX_VARIABLES 256

typedef struct {
    char *names[MAX_VARIABLES];
    Value values[MAX_VARIABLES];
    int count;
} Environment;

void env_define(Environment *env, const char *name, Value value);
Value env_get(Environment *env, const char *name);

#endif
