# Ok tuşlarıyla bir kareyi hareket ettir. Klavye 0x10000008: okununca sıradaki
# tuş olayını verir (0 = yok). Bit 8 basıldı mı, alt 8 bit tuş kodu.
# Ok tuşları: sol 0x80, sağ 0x81, yukarı 0x82, aşağı 0x83.
        li sp, 0x100000
        li s0, 160              # x
        li s1, 100              # y
        li s4, 0                # basılı tuşlar: bit0 sol, bit1 sağ, bit2 yukarı, bit3 aşağı
dongu:  li t0, 0x10000008
tus:    lw t1, 0(t0)
        beqz t1, hareket
        andi t2, t1, 255
        srli t3, t1, 8          # 1 = basıldı
        addi t2, t2, -0x80      # 0..3 ok tuşları
        li t4, 4
        bgeu t2, t4, tus
        li t4, 1
        sll t4, t4, t2
        beqz t3, birak
        or s4, s4, t4
        j tus
birak:  not t4, t4
        and s4, s4, t4
        j tus
hareket:
        andi t1, s4, 1
        beqz t1, h1
        addi s0, s0, -2
h1:     andi t1, s4, 2
        beqz t1, h2
        addi s0, s0, 2
h2:     andi t1, s4, 4
        beqz t1, h3
        addi s1, s1, -2
h3:     andi t1, s4, 8
        beqz t1, h4
        addi s1, s1, 2
h4:     # sınırlar
        bgez s0, s5
        li s0, 0
s5:     li t1, 304
        ble s0, t1, s6
        mv s0, t1
s6:     bgez s1, s7
        li s1, 0
s7:     li t1, 184
        ble s1, t1, ciz
        mv s1, t1
ciz:    # arka planı temizle
        li a0, 0x11000000
        li a1, 64000
        li t1, 0x00202838
temiz:  sw t1, 0(a0)
        addi a0, a0, 4
        addi a1, a1, -1
        bnez a1, temiz
        # 16x16 kare
        li t2, 0
kare_y: li t3, 0
kare_x: add t4, s1, t2          # y
        li t5, 320
        mul t4, t4, t5
        add t4, t4, s0
        add t4, t4, t3
        slli t4, t4, 2
        li t5, 0x11000000
        add t4, t4, t5
        li t5, 0x0050DC78
        sw t5, 0(t4)
        addi t3, t3, 1
        li t5, 16
        blt t3, t5, kare_x
        addi t2, t2, 1
        blt t2, t5, kare_y
        li t5, 0x1000000C
        sw zero, 0(t5)
        j dongu
