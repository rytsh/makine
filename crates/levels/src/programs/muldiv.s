# The M extension, including the edge cases the spec defines.
        li s0, 0x2000
        li a0, 123456
        li a1, -789
        li a2, 0x80000000
        li a3, -1
        li a4, 0xFFFF1234
        mul t0, a0, a1
        sw t0, 0(s0)
        mulh t0, a0, a1
        sw t0, 4(s0)
        mulhu t0, a0, a1
        sw t0, 8(s0)
        mulhsu t0, a1, a4
        sw t0, 12(s0)
        mulh t0, a2, a2
        sw t0, 16(s0)
        mulhu t0, a3, a3
        sw t0, 20(s0)
        div t0, a0, a1
        sw t0, 24(s0)
        divu t0, a0, a1
        sw t0, 28(s0)
        rem t0, a0, a1
        sw t0, 32(s0)
        remu t0, a1, a0
        sw t0, 36(s0)
        div t0, a1, zero        # divide by zero: -1
        sw t0, 40(s0)
        rem t0, a1, zero        # remainder: the dividend
        sw t0, 44(s0)
        divu t0, a0, zero
        sw t0, 48(s0)
        remu t0, a0, zero
        sw t0, 52(s0)
        div t0, a2, a3          # overflow: -2^31
        sw t0, 56(s0)
        rem t0, a2, a3          # remainder 0
        sw t0, 60(s0)
        li t1, 7
        li t2, -2
        div t0, t1, t2          # rounds toward zero: -3
        sw t0, 64(s0)
        rem t0, t1, t2          # 1
        sw t0, 68(s0)
        ebreak
