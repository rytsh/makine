/* Programs load at address 0 of a 16 MB RAM. The stack grows down from the
 * top; the heap grows up from the end of the program. */
OUTPUT_ARCH(riscv)
ENTRY(_start)

MEMORY { RAM (rwx) : ORIGIN = 0, LENGTH = 16M }

SECTIONS
{
    .text : { *(.text.start) *(.text .text.*) } > RAM
    .rodata : ALIGN(8) { *(.rodata .rodata.* .srodata .srodata.*) } > RAM
    .data : ALIGN(8) { *(.data .data.* .sdata .sdata.*) } > RAM
    __bss_start = .;
    .bss : ALIGN(8) { *(.bss .bss.* .sbss .sbss.* COMMON) } > RAM
    . = ALIGN(16);
    _end = .;
    __stack_top = ORIGIN(RAM) + LENGTH(RAM);
    __heap_end = __stack_top - 256K;
    /DISCARD/ : { *(.comment) *(.riscv.attributes) *(.note*) *(.eh_frame*) }
}
