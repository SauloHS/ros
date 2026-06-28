#include "libros.h"

void _start(void) {
    char buf[256];
    char c;
    int pos;

    printf("ROS Userspace Shell\n");

    for (;;) {
        printf("> ");
        pos = 0;
        while (1) {
            if (read(0, &c, 1) != 1) continue;
            if (c == '\n') {
                buf[pos] = '\0';
                printf("\nEcho: %s\n", buf);
                break;
            }
            if (c == '\b' || c == 127) {
                if (pos > 0) {
                    pos--;
                    printf("\b \b");
                }
                continue;
            }
            if (pos < 255) {
                buf[pos++] = c;
                printf("%c", c);
            }
        }
    }
}
