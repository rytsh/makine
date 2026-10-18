<!--
seviyeler:
-->
# Sıfırdan bir komut seti tasarla

Bir komut adı seçmek CPU yapmak değildir. Bu bölüm üç parçayı birbirine
bağlar: **sözleşme (ISA)**, **assembler** ve **kapılardan yapılmış devre**.
Mini işlemci seviyelerinin altında (↳) on tasarım dersi vardır; her biri ilgili devre
seviyesinde açılır.
Gelecek dersleri okuyabilirsin; tamamlama kontrolleri sırayla açılır.
Notların, tamamlanan dersler ve her taslağın ISA tanımı profilde saklanır.

## 1. Önce programcıya ne sunduğunu seç

Bu eğitim için küçük ve doğrulanabilir bir makine seçiyoruz:

- **acc:** 8-bit hesap kaydı; aralık işaretsiz 0–255.
- **pc:** 4-bit komut adresi; 16 ROM kelimesi.
- **RAM:** 16 hücre, her hücre 8 bit.
- **Komut:** 8 bit; üst 4 bit opcode, alt 4 bit argüman.
- **Saat:** her yükselen kenar bir komut.
- **Taşma:** 8-bit hesap sonucu mod 256; PC artışı mod 16.
- **Reset:** bir yükselen kenarda acc=0 ve pc=0.

Bu seçimler silikon yerleşimi veya kaç NAND kullanılacağını söylemez.
Aynı ISA farklı devrelerle gerçekleştirilebilir. Ancak programcıya görünen
sonuçlar aynı olmalıdır. Buradaki öğretim sözleşmesi sabittir; serbest
tasarımda farklı kodlar seçebilirsin, fakat eşleşen devreyi sen kurmalısın.

## 2. Bit bütçesini hesapla

```text
bit:       7 6 5 4   3 2 1 0
alan:      opcode    arg
ADD 7:     0 1 0 0   0 1 1 1  = 0x47
```

Dört opcode biti en çok 16 işlem; dört arg biti en çok 16 değer/adres
verir. acc 8 bit olsa da **LDI yalnızca 0–15 sabitini taşıyabilir**.
LDI 255'i bir bayta sığdırmak mümkün değildir: daha uzun komut, ikinci
kelime veya daha fazla komut gerekir. Bu kapasite sorununu assembler
hileyle çözemez; ISA'da karar vermelisin.

## 3. Her komutun etkisini açık yaz

| Opcode | Ad | Argüman | Görünen etki |
|---|---|---|---|
| 0 | NOP | yok | durum korunur, pc bir artar |
| 1 | LDI | sayı | acc=arg |
| 2 | LD | RAM adresi | acc=RAM[arg] |
| 3 | ST | RAM adresi | RAM[arg]=acc, acc korunur |
| 4 | ADD | RAM adresi | acc=(acc+RAM[arg]) mod 256 |
| 5 | SUB | RAM adresi | acc=(acc-RAM[arg]) mod 256 |
| 6 | JMP | ROM adresi | pc=arg |
| 7 | JZ | ROM adresi | acc=0 ise pc=arg, değilse pc+1 |
| 15 | HLT | yok | pc yerinde kalır, acc/RAM korunur |

Diğer opcode'lar NOP gibi davranır. Bu **bilinçli bir sözleşmedir**;
başka bir ISA hata verebilir. Assembler tanımsız bir adı reddeder,
devredeki davranışı denemek için ham `.word 0x80` kullanılabilir.

**LDI 3** ile **LD 3** aynı şey değildir: RAM[3]=9 ise ilki acc'ye 3,
ikincisi 9 koyar. Bu ayrım ilk tasarım dersinin kontrol sorusudur.

## 4. İsimleri bitlere dönüştür

```asm
LDI 3
ST 0
JMP done
LDI 9
done: HLT
```

İlk geçiş etiketleri yerleştirir: done=4. Etiket kendi başına yer kaplamaz.
İkinci geçiş opcode ve argümanı birleştirir: `13 30 64 19 F0`.
Burada adres **ROM kelimesidir**; RISC-V'nin bayt adresi ve pc+4 kuralı
bu makineye uygulanmaz. Kendi ISA tasarımındaki 16-bit komutlar da bir
ROM kelimesi kaplar.

## 5. Alanları ayıran devreyi yap

**MiniDecode** seviyesinde `instr[8] → op[4],arg[4]` devresini kur.
Bir Böl 4,4 yeterlidir. Böl'ün ilk çıkışı düşük bitlerdir, yani arg.
İkinci çıkış opcode'dur. `0x0F` ve `0xF0` ile ters bağlanmayı yakala.
Bu devre henüz hiçbir hesap veya bellek işlemi yapmaz.

## 6. Kontrol tablosunu çıkar

Veri yolu her an hesaplayabilir; hangi sonuçların saklanacağına izin
sinyalleri karar verir. Kontrol çıkışlarının sırası:

| Komut | load | imm | alu | sub | we | jump | hold |
|---|---|---|---|---|---|---|---|
| NOP | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| LDI | 1 | 1 | 0 | 0 | 0 | 0 | 0 |
| LD | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| ST | 0 | 0 | 0 | 0 | 1 | 0 | 0 |
| ADD | 1 | 0 | 1 | 0 | 0 | 0 | 0 |
| SUB | 1 | 0 | 1 | 1 | 0 | 0 | 0 |
| JMP | 0 | 0 | 0 | 0 | 0 | 1 | 0 |
| JZ | 0 | 0 | 0 | 0 | 0 | zero | 0 |
| HLT | 0 | 0 | 0 | 0 | 0 | 0 | 1 |

`load` acc'ye yazma izni; `imm` sabit veriyi seçer; `alu` ALU sonucunu
seçer; `sub` çıkarma ister; `we` RAM'e yazma; `jump` PC yükleme;
`hold` PC'yi tutmadır. Sıralamayı karıştırma: ADD=1010000, ST=0000100.

## 7. Tabloyu kapılara çevir

Bir opcode eşitliği, gereken bitlerin AND'idir:

```text
is_SUB = op[0] AND NOT op[1] AND op[2] AND NOT op[3]
load = is_LDI OR is_LD OR is_ADD OR is_SUB
alu = is_ADD OR is_SUB
jump = is_JMP OR (is_JZ AND zero)
```

**MiniControl** seviyesinde bu fonksiyonları kur. Tanımsız opcode'larda
bütün sinyaller 0 olmalı. HLT'de RAM'e yazmak veya ST'de acc'yi değiştirmek
yanlış yan etkidir. Test yalnızca doğru ADD sonucunu değil bunları da sınar.

## 8. Durumu saklayan parçaları ayrı öğren

Mini CPU yolu bu yüzden artık yedi devre seviyesidir:

1. **AddSub8:** aynı toplayıcıyla toplama/çıkarma.
2. **MiniDecode:** opcode ve argüman telleri.
3. **MiniZero8:** tüm bitleri kapsayan sıfır testi.
4. **MiniPc4:** reset > hold > jump > pc+1 önceliği.
5. **MiniAcc8:** reset > load > eski değer önceliği.
6. **MiniControl:** kontrol tablosu.
7. **MiniCpu:** hepsini bağlama ve program yürütme.

Kombinasyonel devrede giriş değişince sonuç değişir. Kayıt ise yalnızca
saat kenarında yazar. **Saat yüksekken d'yi tekrar değiştirmek yeni yazma
olmamalıdır.** MiniPc4/MiniAcc8 testleri özellikle bunu dener. Sıfır testi
yalnızca işaret bitine bakamaz: 00000001 de sıfır değildir.

## 9. Veri yolunu bağla ve bir çevrimi izle

```text
ROM[pc] → MiniDecode → MiniControl
                arg → RAM adresi / atlama hedefi
acc → AddSub8 ← rdata
       ALU veya rdata → MUX → sabit arg veya önceki sonuç → acc.d
acc → MiniZero8 → zero → MiniControl
MiniControl.jump / hold + arg → MiniPc4
```

ROM ve veri RAM'i CPU'nun dışında kalır. Saat kenarından önce hesaplanan
veri ve yazma izinleri, kenarda kayıt/belleğe aktarılır. Bu anlatım bir
komutu fazladan saat aşamalarına bölmez.

| Komut | Önce pc,acc | Kenarda etki | Sonra pc,acc |
|---|---|---|---|
| LDI 3 | 0,0 | acc'ye 3 yaz | 1,3 |
| ST 0 | 1,3 | RAM[0]=3; acc'ye yazma yok | 2,3 |
| ADD 0 | 2,3 | acc=3+RAM[0] | 3,6 |
| ST 1 | 3,6 | RAM[1]=6 | 4,6 |
| HLT | 4,6 | yazma yok; PC korunur | 4,6 |

## 10. Kendi çipinle son kontrol

MiniCpu seviyeni kendi devrenle geç. Serbest CPU çipinin adını yazıp
**Kendi MiniCpu çözümümden serbest CPU oluştur** düğmesini kullanabilirsin.
Bu kendi kaydedilmiş çözümünü kopyalar, hazır CPU çözümü vermez; yalnızca
acc çıkışı wdata adını alır. Var olan çipin üzerine yazılmaz.

Son sözleşme: girişler `instr[8],rdata[8],reset,clk`, çıkışlar
`pc[4],addr[4],wdata[8],we`. Uç sırası da böyledir. **Ders kontrolünü
çalıştır**; test gerçek çipinde dört bağımsız program çalıştırır. PC, acc,
adres ve RAM yazmalarını her adımda referansla karşılaştırır; saat yüksek
kalırken ikinci kez yazılmadığını, HLT'nin koruduğunu ve reset'in eski
durumu sildiğini sınar. Kendi çipin yoksa veya yanlışsa referansa geçip
başarılı saymaz.

Geçince **Derle → Devreye yükle** ile bu CPU, ROM ve RAM'i ayrı bir
[Dünya](dunya.md) projesinde aç. Serbest tasarımda farklı opcode seçersen
kontrol devreni de değiştir. Opcode tablosu tek başına toplama yapamaz.

## Deneyler ve hata bulma

- ADD opcode'unu değiştir, devreyi değiştirme: hangi program bozuldu?
- JZ'nin zero girişini tersle: geri sayım nerede yanlış gider?
- ST'de load'u aç: hangi eski acc değeri kaybolur?
- RAM adres tellerini ters sırala: neden bazı adresler doğru görünür?
- Reset öncesi acc'ye sıfır olmayan değer koy: reset sonrası ne olmalı?
- Sonraki tasarımda 3-bit opcode seç: kaç işleme yer var ve hangi komutu
  bırakman gerekir? Bu artık eğitim sözleşmesinden farklı bir ISA'dır.

Temel programlama için [Programlama](programlama.md), küçük makinenin
yapısı için [Mini işlemci](mini.md), gerçek standartla karşılaştırmak için
[RISC-V](riscv.md) sayfalarına dön.
