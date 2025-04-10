#ifndef ENVIRONMENT_H
#define ENVIRONMENT_H

typedef struct {
    char *name;
    char *value;
} Variable;

typedef struct {
    Variable variables[100];
    int count;
} Environment;

void init_environment(Environment *env);
void define_variable(Environment *env, const char *name, const char *value);
const char* get_variable(Environment *env, const char *name);

#endif