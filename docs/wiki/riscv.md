<!--
seviyeler: Decoder, Control, BranchUnit, LoadUnit, StoreUnit, MulDiv
-->
# RISC-V komutları

İşlemcinin anladığı dile **komut seti mimarisi** (ISA) denir. Oyundaki
işlemci, 2010'da Berkeley'de geliştirilen açık bir standart olan
**RISC-V**'yi (okunuşu: "risk beş") konuşur. Aynı komut setini telefon
çiplerinden süper bilgisayarlara kadar milyarlarca cihaz kullanıyor.
Senin yaptığın işlemci gerçek bir C derleyicisinin ürettiği kodu
çalıştırabilir.

## Komut biçimleri

Her komut 32 bittir. Bitler alanlara bölünür:

```
 31        25 24   20 19   15 14  12 11    7 6      0
┌───────────┬───────┬───────┬──────┬───────┬────────┐
│  funct7   │  rs2  │  rs1  │funct3│  rd   │ opcode │   R türü
└───────────┴───────┴───────┴──────┴───────┴────────┘
```

- **opcode:** komutun türü (hesap mı, yükleme mi, atlama mı?)
- **rd:** sonucun yazılacağı register
- **rs1, rs2:** okunacak register'lar
- **funct3, funct7:** türün içindeki ayrıntı (topla mı çıkar mı?)
- **imm:** komutun içine gömülü sabit sayı (anlık değer)

Önemli bir tasarım kararı: **rd, rs1 ve rs2 her türde aynı yerdedir.**
Böylece çözücü, komutun ne olduğunu anlamadan register'ları okumaya
başlayabilir.

## Anlık değerin (imm) toplanışı

imm, türe göre farklı yerlere dağılmıştır ve 32 bite **işaretle
genişletilir**: en üst biti (her zaman komutun bit 31'i) boşluğa
kopyalanır.

| tür | komutlar | imm |
|---|---|---|
| I | `addi`, `lw`, `jalr` | `instr[31:20]` |
| S | `sw`, `sb` | `instr[31:25]` + `instr[11:7]` |
| B | `beq`, `blt` … | karışık, en alt bit hep 0 |
| U | `lui`, `auipc` | `instr[31:12]`, altına 12 sıfır |
| J | `jal` | karışık, en alt bit hep 0 |

B ve J türlerindeki "karışık" düzen ilk bakışta tuhaf görünür. Amaç, her imm
bitinin olabildiğince **hep aynı komut bitinden** gelmesidir. Böylece
çözücüdeki MUX'lar küçük kalır.

## Komut grupları

| grup | örnekler | ne yapar |
|---|---|---|
| hesap | `add`, `sub`, `and`, `sll`, `slt` | `rd = rs1 op rs2` |
| hesap-imm | `addi`, `andi`, `slli` | `rd = rs1 op imm` |
| yükleme | `lw`, `lh`, `lb`, `lbu` | `rd = bellek[rs1 + imm]` |
| saklama | `sw`, `sh`, `sb` | `bellek[rs1 + imm] = rs2` |
| dallanma | `beq`, `bne`, `blt`, `bge` | koşul doğruysa `pc = pc + imm` |
| atlama | `jal`, `jalr` | `rd = pc + 4`, sonra atla (fonksiyon çağrısı) |
| üst | `lui`, `auipc` | büyük sabitler, pc'ye göre adresler |
| çarpma | `mul`, `div`, `rem` | M eklentisi |

## Bayt, yarım kelime, kelime

Bellek her zaman **4 baytlık hizalı bir kelime** verir. `lb` (bir bayt
yükle) için **yükleme birimi** kelimeyi kaydırır ve keser. `lb`'de işaretle
genişletir, `lbu`'da sıfırla.

Saklama tersi: **saklama birimi** veriyi doğru bayta kaydırır ve bir
**bayt maskesi** üretir. Böylece belleğe "sadece bu baytı yaz" denir.

## Çarpma ve bölme

- **Çarpma** okuldaki gibidir: b'nin her biti için a'yı kaydır ve topla. 32
  bit × 32 bit = 64 bitlik sonuç. `mul` alt yarıyı, `mulh` üst yarıyı verir.
- **Bölme** uzun bölmedir: her adımda "kalandan bölen çıkıyor mu?"

RISC-V'de **sıfıra bölmek hata değildir**: bölüm hep 1'lerden oluşur, kalan
`a`'dır. Bu kural donanımı basitleştirir. Kontrol etmek isteyen program
kendisi kontrol eder.

## Assembly

Bilgisayarındaki programlar bu komutların sayı halidir. İnsanlar onları
**assembly** ile yazar:

```asm
        li   a0, 0          # a0 = 0
        li   t0, 10         # t0 = 10 (sayaç)
dongu:  add  a0, a0, t0     # a0 += t0
        addi t0, t0, -1     # t0 -= 1
        bnez t0, dongu      # t0 sıfır değilse başa dön
        ebreak              # dur: a0 = 55
```

Register'ların takma adları vardır: `a0`–`a7` fonksiyon argümanları için,
`t0`–`t6` geçici değerler için, `s0`–`s11` korunanlar için, `sp` yığın,
`ra` dönüş adresi.

Sırada: [İşlemci](islemci.md).
