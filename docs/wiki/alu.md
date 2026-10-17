<!--
seviyeler: Zero32, Compare32, ShiftLeft32, ShiftRight32, Alu
-->
# ALU: aritmetik ve mantık birimi

**ALU**, işlemcinin hesap makinesidir. İki sayı (`a`, `b`) ve bir işlem
kodu (`op`) alır, sonucu verir.

## Sıfır mı?

32 biti VEYA'la: herhangi biri 1 ise sayı sıfır değildir. Sonucun tersi
"sıfır mı?" sorusunun cevabıdır.

Sırayla `b0 VEYA b1 VEYA b2…` yapmak 31 kapılık bir zincir olur. **Ağaç**
şeklinde yapmak daha iyidir: 32 → 16 → 8 → 4 → 2 → 1, sadece 5 kat.
Oyunda `Böl 16,16` ve `Or ×16` ile her katı tek parça yapabilirsin.

## Karşılaştırma

`a - b`'yi [hesapla](cikarma.md), sonra:

| soru | nasıl |
|---|---|
| `a = b`? | fark sıfır mı |
| `a < b` (işaretsiz)? | çıkarmanın **eldesi 0** mı |
| `a < b` (işaretli)? | işaretler farklıysa: negatif olan küçük. Aynıysa: farkın işareti |

İşaretsiz kısım neden elde? `a + DEĞİL(b) + 1` toplamında elde, sonucun
32 bite sığmadığını gösterir. Bu da tam olarak `a ≥ b` durumudur.

## Kaydırıcı (barrel shifter)

`a << n`: a'yı n basamak sola kaydır (2ⁿ ile çarpmak gibi).

Sabit miktarda kaydırmak sadece [tel işi](mux.md). Değişken miktar için n'nin
her bitini bir **aşama** yap:

```
aşama 0: n'nin bit 0'ı 1 ise 1 kaydır,  değilse olduğu gibi
aşama 1: n'nin bit 1'i 1 ise 2 kaydır
aşama 2: … 4
aşama 3: … 8
aşama 4: … 16
```

Her aşama bir **Mux32**. 0 ile 31 arasındaki her kaydırma, 5 aşamanın bir
bileşimidir: 13 = 8 + 4 + 1.

**Sağa kaydırmada** iki çeşit var:

- **mantıksal** (`srl`): soldan 0 gelir
- **aritmetik** (`sra`): soldan **işaret biti** gelir, böylece `-8 >> 1 = -4`
  olur

## ALU'nun tamamı

| op | işlem | | op | işlem |
|---|---|---|---|---|
| 0 | add `a + b` | | 8 | sub `a - b` |
| 1 | sll `a << b` | | 5 | srl `a >> b` |
| 2 | slt `a < b ? 1 : 0` (işaretli) | | 13 | sra `a >> b` (işaretli) |
| 3 | sltu (işaretsiz) | | 4 | xor |
| 6 | or | | 7 | and |

`op`'un alt 3 biti RISC-V komutundaki **funct3** alanından gelir. Bit 3 ise
add/sub ve srl/sra arasında seçim yapar.

En basit yol: **bütün sonuçları hesapla**, sonra op ile doğru olanı bir MUX
ağacıyla seç. Gerçek işlemciler de çoğunlukla böyle yapar: işlemleri
paralel yürütmek, hangisinin gerekeceğini önceden düşünmekten daha
kolaydır.

Sırada: [RISC-V komutları](riscv.md).
