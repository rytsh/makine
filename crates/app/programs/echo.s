# Echo each console key. Enter (ASCII 10) ends the program.
# Empty input is -1, not 0. This address is a device, not ordinary RAM.
    li t0, 0x10000000
    li t2, -1
    li t3, 10
wait:
    lw t1, 0(t0)
    beq t1, t2, wait
    sb t1, 0(t0)
    bne t1, t3, wait
    ebreak
