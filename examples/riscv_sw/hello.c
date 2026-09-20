/* First real C program for examples/riscv_core.volt.
 *
 * Freestanding: no libc, no libgcc. The M extension provides MUL, DIV
 * and REM in hardware, so -march=rv32im never needs a library helper.
 */

#define UART_TX     (*(volatile unsigned char *)0x20000000)
#define UART_STATUS (*(volatile unsigned char *)0x20000004)
#define UART_BUSY   1

void putchar(char c)
{
    while (UART_STATUS & UART_BUSY) { }   /* a store while busy is dropped */
    UART_TX = c;
}

void puts(const char *s)
{
    while (*s)
        putchar(*s++);
}

int main(void)
{
    puts("Hello from Volt!\n");

    /* volatile keeps -O2 from folding the arithmetic away (and from
     * turning the division by ten into a multiply-by-reciprocal), so
     * the core really executes MUL, DIV and REM. */
    volatile int a = 7, b = 6, ten = 10;
    int c = a * b;                 /* MUL -> 42 */
    putchar('0' + c / ten);        /* DIV -> 4  */
    putchar('0' + c % ten);        /* REM -> 2  */
    putchar('\n');
    return 0;
}
