# Function calls with a stack: recursive factorial and Fibonacci.
        li sp, 0x4000
        li s0, 0x2000
        li a0, 10
        call fact
        sw a0, 0(s0)
        li a0, 12
        call fib
        sw a0, 4(s0)
        sw sp, 8(s0)            # stack balanced again
        ebreak

# fact(n) = n * fact(n-1), with multiplication by repeated addition so this
# program does not need the M extension.
fact:   addi sp, sp, -8
        sw ra, 4(sp)
        sw a0, 0(sp)
        li t0, 1
        bgt a0, t0, fact_rec
        li a0, 1
        j fact_ret
fact_rec: addi a0, a0, -1
        call fact
        lw t1, 0(sp)            # n
        li t2, 0
fact_mul: add t2, t2, a0
        addi t1, t1, -1
        bnez t1, fact_mul
        mv a0, t2
fact_ret: lw ra, 4(sp)
        addi sp, sp, 8
        ret

fib:    addi sp, sp, -12
        sw ra, 8(sp)
        sw s1, 4(sp)
        sw a0, 0(sp)
        li t0, 2
        blt a0, t0, fib_ret
        addi a0, a0, -1
        call fib
        mv s1, a0
        lw a0, 0(sp)
        addi a0, a0, -2
        call fib
        add a0, a0, s1
fib_ret: lw s1, 4(sp)
        lw ra, 8(sp)
        addi sp, sp, 12
        ret
