#ifndef ENVIRONMENT_H
#define ENVIRONMENT_H

#include "interpreter.h"

#define HASH_TABLE_SIZE 256

typedef struct {
    char* name;
    Value value;
} HashEntry;

typedef struct {
    HashEntry* table[HASH_TABLE_SIZE];
} HashTable;

void env_init(HashTable* env);
void env_define(HashTable* env, const char* name, Value value);
Value* env_get(HashTable* env, const char* name);

#endif
