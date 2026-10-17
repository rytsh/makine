<!--
seviyeler: Register32, Counter32, Ram8, RegFile
-->
# Register ve RAM

## 32-bit register

Bir **register**, aynı saatle çalışan 32 tane 1-bit kayıttır. Oyunda
**Bit ×32**, yani tek parça. İşlemci hesap yaparken sayıları register'larda
tutar.

## Program sayacı

İşlemci "sıradaki komut nerede?" diye sürekli sorar. Cevabı **program
sayacı** (`pc`) tutar: bir register ve önünde biraz mantık. Her saat
vuruşunda:

| koşul | pc |
|---|---|
| `reset` | 0 (en baştan başla) |
| `load` | d (bir yere **atla**) |
| `inc` | pc + 4 (sıradaki komut) |
| hiçbiri | aynı kal |

Neden +4? RISC-V'de her komut 32 bit, yani **4 bayt**. Bellekte adresler
bayt bayt sayılır.

> [!TIP]
> Öncelik sırasını MUX'larla kur: önce `inc` için bir MUX, sonra onun
> çıkışını `load` için bir MUX'a, en son `reset` için bir MUX'a ver. En
> sondaki MUX en güçlüsüdür.

## RAM

**RAM** (rastgele erişimli bellek) birçok register ve hangisinin
okunup yazılacağını seçen bir **adres**ten oluşur:

- **Okuma:** adres → MUX ağacı → o kelime çıkışa gelir. Saat beklemez.
- **Yazma:** adres → DEMUX ağacı → sadece o register'ın `load`'ı 1 olur.
  Saat kenarında yazılır.

8 kelimelik RAM için 3 bitlik adres yeter: 2³ = 8. Bilgisayarındaki
16 MB'lık RAM aynı fikrin 4 milyon kelimeye büyütülmüş hali. Onu
NAND'lerle simüle etmek yüz milyonlarca kapı demek olurdu. Bu yüzden oyunda
o kısım Rust ile simüle ediliyor.

## Register dosyası

RISC-V işlemcinin **32 register'ı** vardır: `x0` … `x31`. Bir komut aynı anda
**iki tanesini okur** ve **birine yazar**:

```
add x5, x6, x7     →   x5 = x6 + x7
```

Bu yüzden register dosyasının:

- iki okuma girişi (`rs1`, `rs2`) ve iki çıkışı (`a`, `b`) vardır; iki ayrı
  MUX ağacı,
- bir yazma adresi (`rd`), veri (`d`) ve izin (`we`) girişi vardır; bir
  DEMUX ağacı.

**x0 her zaman 0'dır.** Ona yazmak bir şey yapmaz. Bu küçük kural çok işe
yarar: `x0` "sıfır sabiti" olur, "sonucu çöpe at" için de kullanılır. Devrede
x0 için register koymazsın; MUX'un 0. girişine **Sabit 0** bağlarsın.

Sırada: [ALU](alu.md).
