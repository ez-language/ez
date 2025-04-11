#include <stdio.h>
#include <stdlib.h>

char* read_file(const char* path) {
    FILE* file = fopen(path, "rb");

    if (!file) return NULL;

    fseek(file, 0, SEEK_END);

    long length = ftell(file);
    
    fseek(file, 0, SEEK_SET);

    char* buffer = (char*)malloc(length + 1);

    if (!buffer) {
        fclose(file);

        return NULL;
    }

    size_t read = fread(buffer, 1, length, file);
    buffer[read] = '\0';

    fclose(file);

    return buffer;
}
