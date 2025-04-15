#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include <stdio.h>
#include <stdbool.h>

#include "lexer.h"

Lexer lexer_init(const char *source) {
    Lexer lexer;
    lexer.source = source;
    lexer.position = 0;
    return lexer;
}

char current_char(Lexer *lexer) {
    return lexer->source[lexer->position];
}

void advance(Lexer *lexer) {
    if (current_char(lexer) != '\0') {
        lexer->position++;
    }
}

void skip_whitespace(Lexer *lexer) {
    while (isspace(current_char(lexer))) {
        advance(lexer);
    }
}

int is_identifier_start(char c) {
    return isalpha(c) || c == '_';
}

int is_identifier_char(char c) {
    return isalnum(c) || c == '_';
}

char* substring(const char* src, int start, int end) {
    int length = end - start;
    char* str = (char*)malloc(length + 1);
    strncpy(str, src + start, length);
    str[length] = '\0';
    return str;
}

Token make_token(TokenType type, const char *value) {
    Token token;
    token.type = type;
    token.lexeme = value ? strdup(value) : NULL;
    return token;
}

TokenType check_keyword(const char* text) {
    if (strcmp(text, "switch") == 0 || strcmp(text, "case") == 0 || strcmp(text, "default") == 0)
        return TOKEN_KEYWORD;
    if (strcmp(text, "true") == 0 || strcmp(text, "false") == 0)
        return TOKEN_BOOL_LITERAL;
    if (strcmp(text, "null") == 0)
        return TOKEN_NULL;
    if (strcmp(text, "int") == 0)
        return TOKEN_TYPE_INT;
    if (strcmp(text, "float") == 0)
        return TOKEN_TYPE_FLOAT;
    if (strcmp(text, "string") == 0)
        return TOKEN_TYPE_STRING;
    if (strcmp(text, "bool") == 0)
        return TOKEN_TYPE_BOOL;
    if (strcmp(text, "any") == 0)
        return TOKEN_TYPE_ANY;
    if (strcmp(text, "and") == 0) return TOKEN_AND;
    if (strcmp(text, "or") == 0) return TOKEN_OR;
    if (strcmp(text, "not") == 0) return TOKEN_NOT;
    if (strcmp(text, "var") == 0) return TOKEN_VAR;
    if (strcmp(text, "if") == 0) return TOKEN_IF;
    if (strcmp(text, "else") == 0) return TOKEN_ELSE;
    if (strcmp(text, "for") == 0) return TOKEN_FOR;
    if (strcmp(text, "while") == 0) return TOKEN_WHILE;
    if (strcmp(text, "function") == 0) return TOKEN_FUNCTION;
    if (strcmp(text, "print") == 0) return TOKEN_PRINT;
    if (strcmp(text, "return") == 0) return TOKEN_RETURN;
    return TOKEN_IDENTIFIER;
}

void skip_comment(Lexer* lexer) {
    if (lexer->source[lexer->position] == '/' && lexer->source[lexer->position + 1] == '/') {
        lexer->position += 2;
        while (lexer->source[lexer->position] != '\0' && lexer->source[lexer->position] != '\n') {
            lexer->position++;
        }
    }
}

void skip_whitespace_and_comments(Lexer* lexer) {
    while (1) {
        while (isspace(lexer->source[lexer->position])) lexer->position++;
        if (lexer->source[lexer->position] == '/' && lexer->source[lexer->position + 1] == '/') {
            skip_comment(lexer);
        } else {
            break;
        }
    }
}

Token lexer_next_token(Lexer *lexer) {
    skip_whitespace_and_comments(lexer);

    char c = current_char(lexer);

    if (c == '\0') return make_token(TOKEN_EOF, NULL);

    if (is_identifier_start(c)) {
        int start = lexer->position;
        while (is_identifier_char(current_char(lexer))) {
            advance(lexer);
        }
        char *text = substring(lexer->source, start, lexer->position);
        TokenType type = check_keyword(text);
        if (type != TOKEN_IDENTIFIER) return make_token(type, text);
        return make_token(TOKEN_IDENTIFIER, text);
    }

    if (isdigit(c)) {
        size_t start = lexer->position;
        bool has_dot = false;
    
        while (isdigit(lexer->source[lexer->position]) || (!has_dot && lexer->source[lexer->position] == '.')) {
            if (lexer->source[lexer->position] == '.') {
                has_dot = true;
            }
            lexer->position++;
        }
    
        size_t len = lexer->position - start;
        char* number = (char*)malloc(len + 1);
        strncpy(number, lexer->source + start, len);
        number[len] = '\0';
    
        return make_token(TOKEN_NUMBER, number);
    }    

    if (c == '"') {
        if (lexer->source[lexer->position + 1] == '"' &&
            lexer->source[lexer->position + 2] == '"') {
            // Multiline string
            lexer->position += 3; // Skip opening triple quotes
            int start = lexer->position;
    
            while (!(current_char(lexer) == '"' &&
                     lexer->source[lexer->position + 1] == '"' &&
                     lexer->source[lexer->position + 2] == '"') &&
                   current_char(lexer) != '\0') {
                advance(lexer);
            }
    
            int end = lexer->position;
            char *text = substring(lexer->source, start, end);
    
            if (current_char(lexer) != '\0') {
                lexer->position += 3; // Skip closing triple quotes
            }
    
            return make_token(TOKEN_STRING, text);
        } else {
            // Single-line string (já implementado)
            int start = lexer->position + 1;
            advance(lexer); // skip opening quote
    
            while (current_char(lexer) != '"' && current_char(lexer) != '\0') {
                if (current_char(lexer) == '\\') {
                    advance(lexer); // skip backslash
                    if (current_char(lexer) != '\0') {
                        advance(lexer); // skip escaped char
                    }
                } else {
                    advance(lexer);
                }
            }
    
            int end = lexer->position;
            char *text = substring(lexer->source, start, end);
    
            if (current_char(lexer) == '"') advance(lexer); // skip closing quote
    
            return make_token(TOKEN_STRING, text);
        }
    }
    
    if (c == '=' && lexer->source[lexer->position + 1] == '=') {
        advance(lexer); advance(lexer);
        return make_token(TOKEN_EQ, "==");
    }
    if (c == '!' && lexer->source[lexer->position + 1] == '=') {
        advance(lexer); advance(lexer);
        return make_token(TOKEN_NEQ, "!=");
    }
    if (c == '<' && lexer->source[lexer->position + 1] == '=') {
        advance(lexer); advance(lexer);
        return make_token(TOKEN_LTE, "<=");
    }
    if (c == '>' && lexer->source[lexer->position + 1] == '=') {
        advance(lexer); advance(lexer);
        return make_token(TOKEN_GTE, ">=");
    }
    if (c == '<') {
        advance(lexer);
        return make_token(TOKEN_LT, "<");
    }
    if (c == '>') {
        advance(lexer);
        return make_token(TOKEN_GT, ">");
    }

    switch (c) {
        case '=': advance(lexer); return make_token(TOKEN_ASSIGN, "=");
        case ':': advance(lexer); return make_token(TOKEN_COLON, ":");
        case ',': advance(lexer); return make_token(TOKEN_COMMA, ",");
        case ';': advance(lexer); return make_token(TOKEN_SEMICOLON, ";");
        case '.': advance(lexer); return make_token(TOKEN_DOT, ".");
        case '[': advance(lexer); return make_token(TOKEN_LBRACKET, "[");
        case ']': advance(lexer); return make_token(TOKEN_RBRACKET, "]");
        case '{': advance(lexer); return make_token(TOKEN_LBRACE, "{");
        case '}': advance(lexer); return make_token(TOKEN_RBRACE, "}");
        case '(': advance(lexer); return make_token(TOKEN_LPAREN, "(");
        case ')': advance(lexer); return make_token(TOKEN_RPAREN, ")");
        case '+': advance(lexer); return make_token(TOKEN_PLUS, "+");
        case '-': advance(lexer); return make_token(TOKEN_MINUS, "-");
        case '*': advance(lexer); return make_token(TOKEN_STAR, "*");
        case '/': advance(lexer); return make_token(TOKEN_SLASH, "/");
        case '%': advance(lexer); return make_token(TOKEN_PERCENT, "%");
    }

    char unexpected[2] = { current_char(lexer), '\0' };
    advance(lexer);
    return make_token(TOKEN_UNKNOWN, strdup(unexpected));
}

void lex(const char *source) {
    Lexer lexer = lexer_init(source);
    Token token;
    do {
        token = lexer_next_token(&lexer);
        printf("Token: %-16s | Value: %s\n", token_type_to_string(token.type), token.lexeme ? token.lexeme : "(null)");
    } while (token.type != TOKEN_EOF);
}
