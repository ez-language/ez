#include <stdio.h>
#include <string.h>
#include <ctype.h>
#include <stdlib.h>

#include "lexer.h"
#include "token.h"
#include "parser.h"
#include "interpreter.h"

char* read_file(const char* path);

int main(int argc, char **argv) {
    if (argc < 2) {
        printf("Usage: %s <source file>\n", argv[0]);
        return 1;
    }

    char* buffer = read_file(argv[1]);
    if (!buffer) {
        printf("Error: failed to read source file\n");
        return 1;
    }

    Lexer lexer = lexer_init(buffer);
    StmtList stmts = parse(&lexer);
    interpret(&stmts);

    free(buffer);
    return 0;
}
