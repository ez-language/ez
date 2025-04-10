#ifndef INTERPRETER_H
#define INTERPRETER_H

#include "parser.h"
#include "value.h"

void print_value(Value value);
void interpret(StmtList *stmts);

#endif
