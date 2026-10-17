# Loads and stores of every size and alignment.
        li s0, 0x2000
        la s1, data
        lw t0, 0(s1)
        sw t0, 0(s0)
        lb t0, 0(s1)
        sw t0, 4(s0)
        lb t0, 1(s1)
        sw t0, 8(s0)
        lb t0, 2(s1)
        sw t0, 12(s0)
        lb t0, 3(s1)
        sw t0, 16(s0)
        lbu t0, 3(s1)
        sw t0, 20(s0)
        lh t0, 0(s1)
        sw t0, 24(s0)
        lh t0, 2(s1)
        sw t0, 28(s0)
        lhu t0, 2(s1)
        sw t0, 32(s0)
        lw t0, 4(s1)            # positive word
        sw t0, 36(s0)
        lw t0, -4(s1)           # negative offset
        sw t0, 40(s0)

        # Stores of each size into a scratch word.
        li s2, 0x3000
        li t0, -1
        sw t0, 0(s2)
        li t1, 0x11
        sb t1, 0(s2)
        li t1, 0x22
        sb t1, 1(s2)
        li t1, 0x33
        sb t1, 2(s2)
        li t1, 0x44
        sb t1, 3(s2)
        lw t0, 0(s2)
        sw t0, 44(s0)
        li t1, 0xABCD
        sh t1, 2(s2)
        lw t0, 0(s2)
        sw t0, 48(s0)
        li t1, 0x5566
        sh t1, 0(s2)
        lw t0, 0(s2)
        sw t0, 52(s0)
        li t1, 0x12345678
        sb t1, 1(s2)            # only the low byte is stored
        lw t0, 0(s2)
        sw t0, 56(s0)

        # Copy a string byte by byte.
        la a0, msg
        li a1, 0x3100
copy:   lbu t0, 0(a0)
        sb t0, 0(a1)
        addi a0, a0, 1
        addi a1, a1, 1
        bnez t0, copy
        li t0, 0x3100
        lw t1, 0(t0)
        sw t1, 60(s0)
        lw t1, 4(t0)
        sw t1, 64(s0)
        ebreak

        .align 2
before: .word 0xCAFEBABE
data:   .word 0x80FF7F01
        .word 0x12345678
msg:    .asciz "makine!"
