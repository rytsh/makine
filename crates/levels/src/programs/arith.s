# Arithmetic and logic. Every result is stored at the next word of the
# signature area (s0), so a wrong register value shows up as a wrong store.
        li s0, 0x2000
        li a0, 1234
        li a1, -567
        li a2, 0x7FFFFFFF
        li a3, 0x80000000
        li a4, 0xDEADBEEF

        add t0, a0, a1
        sw t0, 0(s0)
        sub t0, a0, a1
        sw t0, 4(s0)
        add t0, a2, a0          # overflow wraps
        sw t0, 8(s0)
        sub t0, a3, a0
        sw t0, 12(s0)
        and t0, a4, a0
        sw t0, 16(s0)
        or t0, a4, a1
        sw t0, 20(s0)
        xor t0, a4, a2
        sw t0, 24(s0)
        slt t0, a1, a0
        sw t0, 28(s0)
        slt t0, a0, a1
        sw t0, 32(s0)
        sltu t0, a1, a0         # -567 is huge unsigned
        sw t0, 36(s0)
        sltu t0, a0, a1
        sw t0, 40(s0)
        li t1, 5
        sll t0, a4, t1
        sw t0, 44(s0)
        srl t0, a4, t1
        sw t0, 48(s0)
        sra t0, a4, t1
        sw t0, 52(s0)
        li t1, 36               # only the low 5 bits count: shift by 4
        sll t0, a0, t1
        sw t0, 56(s0)

        addi t0, a0, -2048
        sw t0, 60(s0)
        addi t0, a0, 2047
        sw t0, 64(s0)
        slti t0, a1, -566
        sw t0, 68(s0)
        sltiu t0, a0, -1        # -1 is the largest unsigned number
        sw t0, 72(s0)
        xori t0, a4, -1         # not
        sw t0, 76(s0)
        ori t0, a0, 0x0F0
        sw t0, 80(s0)
        andi t0, a4, 0x7FF
        sw t0, 84(s0)
        slli t0, a4, 31
        sw t0, 88(s0)
        srli t0, a3, 31
        sw t0, 92(s0)
        srai t0, a3, 31
        sw t0, 96(s0)
        srai t0, a4, 0
        sw t0, 100(s0)

        lui t0, 0xABCDE
        sw t0, 104(s0)
        auipc t0, 1             # pc of this instruction + 0x1000
        sw t0, 108(s0)
        auipc t0, 0
        sw t0, 112(s0)

        # Writes to x0 are ignored.
        li zero, 99
        addi zero, a0, 1
        sw zero, 116(s0)

        # Every register can hold a value.
        li x1, 1
        li x3, 3
        li x4, 4
        li x5, 5
        li x6, 6
        li x7, 7
        li x9, 9
        li x10, 10
        li x11, 11
        li x12, 12
        li x13, 13
        li x14, 14
        li x15, 15
        li x16, 16
        li x17, 17
        li x18, 18
        li x19, 19
        li x20, 20
        li x21, 21
        li x22, 22
        li x23, 23
        li x24, 24
        li x25, 25
        li x26, 26
        li x27, 27
        li x28, 28
        li x29, 29
        li x30, 30
        li x31, 31
        li x2, 2
        sw x1, 120(s0)
        sw x2, 124(s0)
        sw x3, 128(s0)
        sw x4, 132(s0)
        sw x5, 136(s0)
        sw x6, 140(s0)
        sw x7, 144(s0)
        sw x9, 148(s0)
        sw x10, 152(s0)
        sw x11, 156(s0)
        sw x12, 160(s0)
        sw x13, 164(s0)
        sw x14, 168(s0)
        sw x15, 172(s0)
        sw x16, 176(s0)
        sw x17, 180(s0)
        sw x18, 184(s0)
        sw x19, 188(s0)
        sw x20, 192(s0)
        sw x21, 196(s0)
        sw x22, 200(s0)
        sw x23, 204(s0)
        sw x24, 208(s0)
        sw x25, 212(s0)
        sw x26, 216(s0)
        sw x27, 220(s0)
        sw x28, 224(s0)
        sw x29, 228(s0)
        sw x30, 232(s0)
        sw x31, 236(s0)
        sw s0, 240(s0)
        ebreak
