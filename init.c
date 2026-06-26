#include "libros.h"

void _start(void) {
    printf("Hello from init (compiled with C, using printf)!\n");
    printf("Answer: %d, hex: 0x%x\n", 42, 0xdead);
    exit(0);
}
