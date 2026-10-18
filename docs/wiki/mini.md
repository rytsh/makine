<!--
seviyeler: AddSub8, MiniDecode, MiniZero8, MiniPc4, MiniAcc8, MiniControl, MiniCpu
-->
# Mini işlemci: ilk bilgisayarın

Hafıza bölümünü bitirdin: kayıtların, sayacın ve RAM'in var. RISC-V
işlemciye geçmeden önce, elindeki parçalarla **küçük ama gerçek** bir
işlemci kurabilirsin. Bu bölüm isteğe bağlıdır; ALU bölümü onu beklemez.

## Akümülatörlü makine

Mini işlemcinin tek bir hesap kaydı var: **acc** (akümülatör, 8 bit).
Her işlem acc ile yapılır, sonuç yine acc'ye yazılır. 1970'lerin ilk
mikroişlemcileri (Intel 4004, 8008, MOS 6502) böyle çalışırdı.

Bir de **pc** (program sayacı, 4 bit) var: sıradaki komutun adresi.
Program 16 baytlık bir ROM'da, veriler 16 baytlık bir RAM'de durur; ikisi
de çipin dışındadır.

## Komutlar

Her komut bir bayt: üst 4 bit **op** (ne yapılacak), alt 4 bit **arg**
(bir sayı ya da bir RAM adresi).

| op | komut | ne yapar |
|---|---|---|
| 1 | `LDI n` | acc = n |
| 2 | `LD a` | acc = RAM[a] |
| 3 | `ST a` | RAM[a] = acc |
| 4 | `ADD a` | acc = acc + RAM[a] |
| 5 | `SUB a` | acc = acc − RAM[a] |
| 6 | `JMP a` | pc = a |
| 7 | `JZ a` | acc = 0 ise pc = a |
| 15 | `HLT` | dur (pc yerinde kalır) |

Diğer op'lar hiçbir şey yapmaz (`NOP`). Atlama yoksa pc bir artar.

## Bir program

RAM[0] ile RAM[1]'i çarpıp sonucu RAM[2]'ye yazan program, toplamayı
tekrarlayarak çarpar (RAM[3] = 1):

```
0: LD 2     acc = sonuç
1: ADD 0    acc += a
2: ST 2     sonuç = acc
3: LD 1     acc = b
4: SUB 3    acc -= 1
5: ST 1     b = acc
6: JZ 8     b = 0 ise bitti
7: JMP 0    değilse tekrar
8: LD 2
9: HLT
```

Bu makinede çarpma komutu yok, ama toplama ve koşullu döngüyle bu program
çarpma yapar. Yine de 16 baytlık ROM/RAM sınırlıdır: her programı
sığdıramaz, her hesabı temsil edemez. Genel hesaplanabilirlikte Turing'in
1936'daki modeli gerekirse büyüyebilen bellek varsayar; bu küçük devreyle
onu aynı şey sanmamalısın.

## Parçalar

1. **Topla / Çıkar:** tek bir toplayıcı hem toplar hem çıkarır:
   a − b = a + (DEĞİL b) + 1. [Çıkarma](cikarma.md) sayfasına bak.
2. **Komutu Alanlarına Ayır (MiniDecode):** üst dört bit opcode, alt dört bit
   argüman. Bitleri taşımak işlem yapmak değildir.
3. **Sıfır Bayrağı (MiniZero8):** sekiz bitin hepsini sınar; JZ'nin koşulunu üretir.
4. **Komut Adresi ve Atlama (MiniPc4):** reset, durma, atlama ve +1 önceliği;
   sonuç yalnızca yükselen kenarda saklanır.
5. **Akümülatör (MiniAcc8):** reset ve yazma izni olan 8-bit kayıt.
6. **Mini Kontrol:** komutun op'una bakıp her parçaya ne yapacağını
   söyleyen devre. Yalnızca VE, VEYA ve DEĞİL'den oluşur.
7. **Mini İşlemci:** öğrendiğin altı parçayı seçicilerle birleştir.

Her seviyede **Ayrıntılı anlatım ve deney** altında somut örnek, sık hata ve
bir deney vardır. Kendi tahminini not olarak kaydedebilirsin. Saat yüksek
kalırken veriyi değiştirmek yeni kenar sayılmaz; PC ve acc yeniden yazılmamalı.
Yazmaç ve kontrolü bir anda kurmak yerine bu ara seviyeleri sırayla tamamla.

Kendi opcode tablonu ve bu devrenin bağlantısını öğrenmek için
[Sıfırdan bir komut seti tasarla](komutseti.md) yoluna geç. Programlama'daki
on tasarım dersi, sadece metni okuduğunu değil kodlamayı ve devreni de sınar.

Test, işlemcinde üç program çalıştırır ve her adımda acc'yi, pc'yi ve
RAM'e yazılanları bir referansla karşılaştırır. Bir fark olursa hangi
komutta olduğunu söyler.

Sonra [ALU](alu.md) bölümünde aynı fikirleri 32 bite ve RISC-V'nin
40'tan fazla komutuna büyüteceksin.
