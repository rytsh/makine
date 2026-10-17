# Konsola yazı yaz. Konsol 0x10000000 adresinde: oraya yazılan her bayt
# ekranın altındaki konsolda görünür.
        li sp, 0x100000
        la a0, mesaj
        call yaz
        la a0, ikinci
        call yaz
        ebreak                  # dur

# yaz(a0 = metnin adresi): sıfır baytına kadar her baytı konsola gönder.
yaz:    li t1, 0x10000000
dongu:  lbu t0, 0(a0)
        beqz t0, bitti
        sb t0, 0(t1)
        addi a0, a0, 1
        j dongu
bitti:  ret

mesaj:  .asciz "Merhaba! Bu yazı senin yaptığın işlemcide çalışıyor.\n"
ikinci: .asciz "NAND kapılarından bir bilgisayar :)\n"
