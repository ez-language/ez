#include <string.h>
#include <stdlib.h>
#include "environment.h"

void init_environment(Environment *env) {
    env->count = 0;
}

void define_variable(Environment *env, const char *name, const char *value) {
    if (env->count < 100) {
        env->variables[env->count].name = strdup(name);
        env->variables[env->count].value = strdup(value);
        env->count++;
    }
}

const char* get_variable(Environment *env, const char *name) {
    for (int i = 0; i < env->count; i++) {
        if (strcmp(env->variables[i].name, name) == 0) {
            return env->variables[i].value;
        }
    }
    return NULL;
}
