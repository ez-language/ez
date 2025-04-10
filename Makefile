CC = gcc
CFLAGS = -Wall -Iinclude

SRC = src/main.c src/lexer.c src/token.c src/ast.c src/environment.c src/interpreter.c src/parser.c
OUT = ez

all: $(OUT)

$(OUT): $(SRC)
	$(CC) $(CFLAGS) -o $(OUT) $(SRC)

clean:
	rm -f $(OUT)