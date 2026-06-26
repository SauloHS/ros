void _start(void) {
    __asm__ volatile (
        "jmp 1f\n"
        "0: .ascii \"Hello from init (compiled with C)!\\n\"\n"
        "1:\n"
        "mov $1, %%eax\n"
        "mov $1, %%edi\n"
        "lea 0b(%%rip), %%rsi\n"
        "mov $35, %%edx\n"
        "int $0x80\n"
        "mov $60, %%eax\n"
        "xor %%edi, %%edi\n"
        "int $0x80\n"
        "jmp 1b\n"
        :
        :
        : "eax", "edi", "rsi", "rdx"
    );
}
