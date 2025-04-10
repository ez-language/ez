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
    Token token;

    do {
        token = lexer_next_token(&lexer);
        printf("Token: %s (%s)\n", token_type_to_string(token.type), token.lexeme);
    } while (token.type != TOKEN_EOF);

    printf("Final do source: [%c] (%d)\n", buffer[strlen(buffer) - 1], buffer[strlen(buffer) - 1]);

    for (int i = 0; buffer[i] != '\0'; i++) {
        printf("%03d (%c)\n", buffer[i], isprint(buffer[i]) ? buffer[i] : '.');
    }

    free(buffer);
    return 0;
}
