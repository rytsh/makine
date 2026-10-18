# Nested function calls: save ra before another call overwrites it.
# Keep sp aligned to 16 bytes. The result of add_four(7) is 11.
    li sp, 0x10000
    li a0, 7
    call add_four
    ebreak
add_four:
    addi sp, sp, -16
    sw ra, 12(sp)
    call add_two
    call add_two
    lw ra, 12(sp)
    addi sp, sp, 16
    ret
add_two:
    addi a0, a0, 2
    ret
