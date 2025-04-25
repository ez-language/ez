#ifndef INTERPRETER_H
#define INTERPRETER_H

#include "parser.h"
#include "value.h"

void print_value(Value value, bool add_newline);
void interpret(StmtList *stmts);

#endif
