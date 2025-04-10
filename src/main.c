#include <stdio.h>
#include "lexer.h"

int main(int argc, char **argv) {
    if (argc < 2) {
        printf("Usage: %s <source file>\n", argv[0]);
        return 1;
    }

    const char *source = argv[1];
    FILE *file = fopen(source, "r");
    if (!file) {
        printf("Error: failed to read source file\n");
        return 1;
    }

    fseek(file, 0, SEEK_END);
    long length = ftell(file);
    fseek(file, 0, SEEK_SET);
    char *buffer = malloc(length + 1);
    fread(buffer, 1, length, file);
    buffer[length] = '\0';
    fclose(file);

    Lexer lexer = lexer_init(buffer);
    Token token;

    do {
        token = lexer_next_token(&lexer);
        printf("Token: %s (%s)\n", token_type_to_string(token.type), token.lexeme);
    } while (token.type != TOKEN_EOF);

    free(buffer);
    return 0;
}