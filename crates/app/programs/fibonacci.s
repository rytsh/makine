# Fibonacci: store the first 12 values at results, then stop.
    la t0, results
    li t1, 12
    li a0, 0
    li a1, 1
loop:
    sw a0, 0(t0)
    add t2, a0, a1
    mv a0, a1
    mv a1, t2
    addi t0, t0, 4
    addi t1, t1, -1
    bnez t1, loop
    ebreak
.align 2
results: .space 48
