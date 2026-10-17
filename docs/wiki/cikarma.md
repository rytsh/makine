<!--
seviyeler: Sub32
-->
# Negatif sayılar ve çıkarma

## İkiye tümleyen

32 bitle 0'dan 4 294 967 295'e kadar sayılar yazılabilir. Negatif sayılar
için bilgisayarlar **ikiye tümleyen** (two's complement) denen akıllıca bir
yöntem kullanır:

> **-x = DEĞİL(x) + 1**

8 bitlik örnek:

```
 5        = 0000 0101
 DEĞİL 5  = 1111 1010
 +1       = 1111 1011   ← bu -5
```

Kontrol edelim: `5 + (-5)`

```
   0000 0101
 + 1111 1011
 -----------
 1 0000 0000   → taşan elde atılır → 0000 0000 = 0 ✓
```

Sihir şu: **toplama devresi hiç değişmeden** negatif sayılarla da doğru
çalışır. Ayrı bir "negatif toplama" devresine gerek yok.

### En üst bit = işaret

İkiye tümleyende en üst bit 1 ise sayı negatiftir:

| 8 bit | işaretsiz | işaretli |
|---|---|---|
| `0000 0001` | 1 | 1 |
| `0111 1111` | 127 | 127 |
| `1000 0000` | 128 | **-128** |
| `1111 1111` | 255 | **-1** |

Aynı bitler iki farklı şekilde okunabilir. Hangisi olduğu, bitlere değil
**komuta** bağlıdır: `blt` işaretli karşılaştırır, `bltu` işaretsiz.

## Çıkarma

```
a - b  =  a + (-b)  =  a + DEĞİL(b) + 1
```

Devre olarak:

1. `b`'nin 32 bitini **Not ×32** ile ters çevir.
2. 32-bit Toplayıcıya `a` ve `DEĞİL b`'yi ver.
3. **+1** için toplayıcının `c` (elde) girişine **Sabit 1** bağla.

Ek bir parça gerekmez: toplayıcının elde girişi tam bu iş için var.

> [!TIP]
> Bu seviyede **Sabit** parçasını kullanacaksın: sağ tık → genişlik 1,
> değer 1.

## Taşma

İşaretli 32 bit en fazla `2 147 483 647` tutar. Buna 1 eklersen
`-2 147 483 648` olur! Buna **taşma** (overflow) denir. RISC-V bunu hata
saymaz, sadece sarar (wrap around). Program isterse kontrol eder.

Sırada: [Hafıza](hafiza.md).
