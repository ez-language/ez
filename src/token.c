#include "token.h"

const char* token_type_to_string(TokenType type) {
    switch (type) {
        case TOKEN_EOF: return "EOF";
        case TOKEN_IDENTIFIER: return "IDENTIFIER";
        case TOKEN_UNKNOWN: return "UNKNOWN";
        default: return "UNKNOWN_TOKEN";
    }
}