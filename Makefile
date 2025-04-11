CC = gcc
CFLAGS = -Wall -Wextra -std=c99 -Iinclude
SRC = $(wildcard src/*.c)
OUT = ez.exe

all: $(OUT)

$(OUT): $(SRC)
	$(CC) $(CFLAGS) $^ -o $@

clean:
	rm -f $(OUT)
