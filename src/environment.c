#include <string.h>
#include <stdio.h>
#include <stdlib.h>

#include "environment.h"

unsigned int hash(const char* str) {
    unsigned int hash = 5381;
    int c;

    while ((c = *str++)) {
        hash = ((hash << 5) + hash) + c;
    }

    return hash % HASH_TABLE_SIZE;
}

void env_init(HashTable* env) {
    for (int i = 0; i < HASH_TABLE_SIZE; i++) {
        env->table[i] = NULL;
    }
}

void env_define(HashTable* env, const char* name, Value value) {
    unsigned int index = hash(name);

    HashEntry* entry = malloc(sizeof(HashEntry));
    entry->name = strdup(name);
    entry->value = value;

    env->table[index] = entry;
}

Value* env_get(HashTable* env, const char* name) {
    unsigned int index = hash(name);
    HashEntry* entry = env->table[index];

    if (entry == NULL) {
        printf("[ERROR] Undefined variable '%s'\n", name);
        exit(1);
    }

    return &entry->value;
}