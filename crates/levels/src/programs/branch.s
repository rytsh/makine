# Branches and jumps. t6 counts how many checks passed; a wrong decision
# jumps to `bad`, which stores the failing check number.
        li s0, 0x2000
        li t6, 0
        li a0, 5
        li a1, -3
        li a2, 5

        beq a0, a2, ok1
        j bad
ok1:  addi t6, t6, 1
        beq a0, a1, bad
        addi t6, t6, 1
        bne a0, a1, ok2
        j bad
ok2:  addi t6, t6, 1
        bne a0, a2, bad
        addi t6, t6, 1
        blt a1, a0, ok3       # -3 < 5 signed
        j bad
ok3:  addi t6, t6, 1
        blt a0, a1, bad
        addi t6, t6, 1
        blt a0, a2, bad         # equal is not less
        addi t6, t6, 1
        bge a0, a1, ok4
        j bad
ok4:  addi t6, t6, 1
        bge a0, a2, ok5       # equal counts
        j bad
ok5:  addi t6, t6, 1
        bge a1, a0, bad
        addi t6, t6, 1
        bltu a0, a1, ok6      # 5 < 0xFFFFFFFD unsigned
        j bad
ok6:  addi t6, t6, 1
        bltu a1, a0, bad
        addi t6, t6, 1
        bgeu a1, a0, ok7
        j bad
ok7:  addi t6, t6, 1
        bgeu a0, a1, bad
        addi t6, t6, 1

        # A counting loop with a backward branch.
        li t0, 0
        li t1, 0
loop:   add t1, t1, t0
        addi t0, t0, 1
        li t2, 10
        blt t0, t2, loop
        sw t1, 4(s0)            # 0+1+...+9 = 45

        # jal saves the return address; jalr returns through it.
        jal ra, sub1
        sw a3, 8(s0)
        sw ra, 12(s0)
        # jalr with an offset, and its low bit cleared.
        la t0, target
        jalr t1, 1(t0)
        j bad
target: sw t1, 16(s0)
        # jal forward and backward with large offsets.
        j far
back:   addi t6, t6, 100
        sw t6, 0(s0)
        ebreak

sub1:   li a3, 77
        ret

bad:    sw t6, 20(s0)
        ebreak

        .space 1500
far:    j back
