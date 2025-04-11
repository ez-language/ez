#include <string.h>
#include <stdio.h>
#include <stdlib.h>

#include "environment.h"

void env_define(Environment *env, const char *name, Value value) {
    if (env->count >= MAX_VARIABLES) {
        printf("[ERRO] Limite de variáveis atingido\n");
        exit(1);
    }
    env->names[env->count] = strdup(name);
    env->values[env->count] = value;
    env->count++;
}

Value env_get(Environment *env, const char *name) {
    for (int i = 0; i < env->count; i++) {
        if (strcmp(env->names[i], name) == 0) {
            return env->values[i];
        }
    }
    printf("[ERRO] Variável não definida: %s\n", name);
    exit(1);
}
