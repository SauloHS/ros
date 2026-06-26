int write(int fd, const char *buf, int len) {
    int ret;
    __asm__ volatile (
        "syscall"
        : "=a" (ret)
        : "a" (1), "D" (fd), "S" (buf), "d" (len)
        : "rcx", "r11", "memory"
    );
    return ret;
}

void exit(int code) {
    __asm__ volatile (
        "syscall"
        :
        : "a" (60), "D" (code)
        : "rcx", "r11"
    );
    for (;;);
}

int puts(const char *s) {
    int len = 0;
    while (s[len]) len++;
    return write(1, s, len);
}

struct printf_buf {
    char data[256];
    int len;
};

static void pb_putc(struct printf_buf *b, char c) {
    if (b->len < 256) b->data[b->len++] = c;
}

static void pb_puts(struct printf_buf *b, const char *s) {
    while (*s) pb_putc(b, *s++);
}

static void pb_putdec(struct printf_buf *b, unsigned long n) {
    char buf[20];
    int i = 0;
    if (n == 0) { pb_putc(b, '0'); return; }
    while (n > 0) {
        buf[i++] = '0' + (n % 10);
        n /= 10;
    }
    while (i > 0) pb_putc(b, buf[--i]);
}

static void pb_puthex(struct printf_buf *b, unsigned long n) {
    const char *digits = "0123456789abcdef";
    char buf[16];
    int i = 0;
    if (n == 0) { pb_putc(b, '0'); return; }
    while (n > 0) {
        buf[i++] = digits[n & 0xf];
        n >>= 4;
    }
    while (i > 0) pb_putc(b, buf[--i]);
}

int printf(const char *fmt, ...) {
    struct printf_buf b;
    b.len = 0;

    __builtin_va_list args;
    __builtin_va_start(args, fmt);

    for (const char *p = fmt; *p; p++) {
        if (*p != '%') {
            pb_putc(&b, *p);
            continue;
        }
        switch (*++p) {
        case 's':
            pb_puts(&b, __builtin_va_arg(args, const char *));
            break;
        case 'd': {
            int v = __builtin_va_arg(args, int);
            if (v < 0) { pb_putc(&b, '-'); v = -v; }
            pb_putdec(&b, v);
            break;
        }
        case 'x':
            pb_puthex(&b, __builtin_va_arg(args, unsigned));
            break;
        case 'c':
            pb_putc(&b, __builtin_va_arg(args, int));
            break;
        case '%':
            pb_putc(&b, '%');
            break;
        default:
            pb_putc(&b, '%');
            pb_putc(&b, *p);
            break;
        }
    }

    __builtin_va_end(args);
    return write(1, b.data, b.len);
}
