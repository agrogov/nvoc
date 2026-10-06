CC ?= cc
CFLAGS ?= -O2 -Wall -Wextra

rmtrace.so: rmtrace.c
	$(CC) $(CFLAGS) -shared -fPIC -o $@ $< -ldl -lpthread

clean:
	rm -f rmtrace.so

.PHONY: clean
