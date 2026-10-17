# Ekrana renkli desen çiz ve hareket ettir. Ekran 320x200 piksel; her piksel
# 0x11000000'dan başlayan bir kelime (0x00RRGGBB). 0x1000000C'ye yazmak
# kareyi gösterir.
        li s0, 0x11000000       # ekran
        li s1, 320              # genişlik
        li s2, 200              # yükseklik
        li s3, 0                # zaman
kare:   li t1, 0                # y
        mv a0, s0
satir:  li t0, 0                # x
piksel: add t2, t0, s3          # kırmızı = x + zaman
        andi t2, t2, 255
        slli t2, t2, 16
        add t3, t1, s3          # yeşil = y + zaman
        andi t3, t3, 255
        slli t3, t3, 8
        xor t4, t0, t1          # mavi = x xor y
        andi t4, t4, 255
        or t2, t2, t3
        or t2, t2, t4
        sw t2, 0(a0)
        addi a0, a0, 4
        addi t0, t0, 1
        blt t0, s1, piksel
        addi t1, t1, 1
        blt t1, s2, satir
        li t5, 0x1000000C
        sw zero, 0(t5)          # kareyi göster
        addi s3, s3, 3
        j kare
