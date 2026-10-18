# Bubble sort: inspect values in memory before and after.
    la s0, values
    li s1, 5
outer:
    mv t0, s0
    mv t1, s1
inner:
    lw t2, 0(t0)
    lw t3, 4(t0)
    bge t3, t2, ordered
    sw t3, 0(t0)
    sw t2, 4(t0)
ordered:
    addi t0, t0, 4
    addi t1, t1, -1
    bnez t1, inner
    addi s1, s1, -1
    bnez s1, outer
    ebreak
.align 2
values: .word 9, -3, 7, 0, 2, -1
