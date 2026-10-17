        .section .rodata.wad
        .balign 4
        .globl wad_data, wad_size
wad_data:
        .incbin WAD_PATH
wad_end:
        .balign 4
wad_size:
        .word wad_end - wad_data
