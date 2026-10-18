<!--
seviyeler: AddressDecode2, Rom4x8, Ram4x8, ProgramRom, RomSequencer
-->
# Bellek ve ROM: program bir devreye nasıl girer?

**Bellek ve ROM**, Hafıza bölümünden sonra açılan beş seviyelik isteğe
bağlı bir yoldur. Ana RISC-V yolunda ilerlemek için bitirmen gerekmez.
[Register ve RAM](register.md) konusunu daha küçük, baytlık devrelerle
deneyip programın komut baytlarına nasıl dönüştüğünü görürsün.

## Belleği tarif eden üç ayrı sayı

**Adres biti**, **hücre sayısı** ve **veri genişliği** aynı şey değildir.
`A` adres biti `2^A` hücre seçer. Her hücre `D` bit tutuyorsa kapasite
`2^A × D` bittir; bayta çevirmek için 8'e böl.

| Bellek | Adres bitleri | Hücre başına veri | Toplam |
|---|---|---|---|
| Rom4x8 / Ram4x8 | 2 | 8 bit | 4 bayt |
| ProgramRom | 4 | 8 bit | 16 bayt |
| Ana yoldaki Ram8 | 3 | 32 bit | 32 bayt |

Adres 2, **üçüncü hücreyi** seçer; “2 bit oku” veya “veri 2 olsun” demez.
8-bit verinin aralığı 0–255'tir. Adresin 4 bit olması, veri hücresinin de
4 bit olduğu anlamına gelmez.

## 1. Adres çözücü

İki adres biti dört hücre seçer. `addr` veri değil, hangi hücreyle
konuştuğumuzdur. `en=1` iken yalnızca ilgili çıkış 1 olur:

| addr | s0 | s1 | s2 | s3 |
|---|---|---|---|---|
| 0 (00) | 1 | 0 | 0 | 0 |
| 1 (01) | 0 | 1 | 0 | 0 |
| 2 (10) | 0 | 0 | 1 | 0 |
| 3 (11) | 0 | 0 | 0 | 1 |

`en=0` iken hepsi 0'dır. RAM'de `en` yazma iznidir. Bunu bütün
kayıtlara doğrudan verirsen yalnızca seçilen adresi değil, hepsini yazarsın.

**Dene:** en kapalıyken adresi değiştir. Hiçbir seçim çıkışı açılmamalı.

## 2. Sabitlerden ROM kur

ROM salt okunur bellektir. Bu alıştırmada hazır ROM kullanmak yerine
dört **8-bit Sabit** ve bir seçici ağacı bağlarsın:

| Adres | Veri (hex) | Veri (onluk) |
|---|---|---|
| 0 | 0x12 | 18 |
| 1 | 0xA5 | 165 |
| 2 | 0x3C | 60 |
| 3 | 0x80 | 128 |

İlk MUX katmanı düşük adres bitiyle iki çiftin içinden seçer. İkinci
katman yüksek bitle bu iki sonucu seçer. Her MUX'un **Tekrar** sayısı 8'dir;
tek seçim biti sekiz veri bitine ortak gider. **Böl** grupları düşük bitten
başlar: `1,1` için ilk çıkış adresin bit 0'ıdır.

ROM'da saat veya yazma izni yoktur. Adres değişince q değişir; hiçbir
hücrenin içeriği değişmez. Bu ROM'u programlamak, sabitleri ve bağlantıları
belirlemektir. Seviyenin testi yukarıdaki içeriği bekler; farklı içerik
denemelerini bir kopyada veya Dünya'da yapabilirsin.

## 3. Küçük RAM kur

ROM'un dört sabiti yerine dört `Bit×8` kaydı koy. d ve clk ortak,
load izinleri adres çözücünün dört ayrı çıkışı olsun. Okuma için yine
MUX ağacı kullanılır. `we=0` okumayı kapatmaz.

`4 adres × 8 bit = 32 bit = 4 bayt` kapasite vardır. Bu, ana Hafıza
yolundaki `Ram8` çipinden farklıdır: o, sekiz **32-bit kelime** saklar.

**Dene:** adres 1'e 0xA5, adres 2'ye 0x3C yaz. Saati yüksek tutup veriyi
değiştir; yeni bir yükselen kenar olmadan hücreler değişmemeli. Sonra dört
adresi de geri oku; komşu hücreleri bozmadığını kontrol et.

### Saatin bir seviyesi ile kenarı farklıdır

Aşağıdaki örnekte hücre 1'e daha önce 0 yazılmış olsun. Yalnızca 0→1
geçişi yazar; saat zaten yüksekken d veya we değiştirmek yazma değildir.

| Adım | addr | d | we | clk | q | Neden? |
|---|---|---|---|---|---|---|
| Hazırlık | 1 | 165 | 1 | 0 | 0 | Yeni kenar yok |
| Yaz | 1 | 165 | 1 | 1 | 165 | Yükselen kenar var |
| Veriyi değiştir | 1 | 60 | 1 | 1 | 165 | Saat yüksek kaldı |
| Saati indir | 1 | 60 | 0 | 0 | 165 | Düşen kenar yazmaz |
| Yazmayı kapalı tut | 1 | 60 | 0 | 1 | 165 | Kenar var ama we=0 |

Seviye sözleşmesi başlangıç RAM içeriğini zorunlu olarak sıfır kabul
etmez. Bir hücrenin sonucuna güvenmeden önce ona yaz. Dünya'daki hazır
RAM aygıtı sıfırdan başlar; bu simülatörün kolaylığıdır, bütün fiziksel
RAM'ler açılınca sıfır olur diye bir kural değildir. Profil devreyi
saklar; çalışan RAM içeriğini kalıcı ROM gibi düşünme.

### Bir kelime birden fazla bayta yayılabilir

Bayt adresli bir bellekte 32-bit `0x12345678` sayısı dört hücre gerektirir.
RISC-V yolunda küçük uçlu sıra kullanılır:

| Bayt adresi | İçerik |
|---|---|
| a | 0x78 |
| a+1 | 0x56 |
| a+2 | 0x34 |
| a+3 | 0x12 |

`Ram8` ise doğrudan 32-bit kelimeler seçer: onun `addr+1`'i sonraki
kelimedir. Bir çipin kelime adresini programdaki bayt adresiyle karıştırma.

## 4. Programı ROM'a kodla

Program da bellekte duran sayılardır. Mini CPU'nun bir komutu sekiz bittir:

```text
üst 4 bit: opcode | alt 4 bit: argüman
```

Bu seviyede şu programı 16 adreslik ROM'a yerleştirirsin:

| Adres | Kaynak komut | Bayt | CPU yürüttüğünde |
|---|---|---|---|
| 0 | LDI 3 | 0x13 | acc = 3 |
| 1 | ST 0 | 0x30 | RAM[0] = acc |
| 2 | ADD 0 | 0x40 | acc = acc + RAM[0] |
| 3 | ST 1 | 0x31 | RAM[1] = acc |
| 4 | HLT | 0xF0 | PC durur |
| 5–15 | NOP | 0x00 | İşlem yok |

`ADD 0`, sabit 0'ı değil **RAM[0]'daki 3'ü** ekler. Program
yürütülünce RAM[0]=3 ve RAM[1]=6 olur. ROM tek başına bu hesabı yapmaz;
sadece adreslenen baytı verir. Opcode/argüman sırası tersse `0x13`
yerine `0x31` üretir ve bambaşka bir komut saklarsın.

### Komut belleği ile veri belleği ayrı

Mini CPU'nun `ROM[0]` adresi ile `RAM[0]` adresi iki farklı bellektedir.
`ST 0`, ilk komut olan 0x13'ü değiştirmez; veri RAM'ine yazar. Bu ayrı
komut/veri düzeni Harvard yaklaşımının küçük bir örneğidir. RISC-V
bilgisayarında ise yüklenen program ve normal veriler aynı RAM adres
alanındadır. Buradaki ROM deneyi, bütün bilgisayarlar ayrı ROM'dan komut
okur anlamına gelmez.

Yukarıdaki kodu gerçek Mini CPU'da adımladığında, başlangıç acc=0 için:

| Yürütülen komut | Sonraki PC | acc | RAM'e etkisi |
|---|---|---|---|
| LDI 3 | 1 | 3 | Yok |
| ST 0 | 2 | 3 | RAM[0]=3 |
| ADD 0 | 3 | 6 | Yok |
| ST 1 | 4 | 6 | RAM[1]=6 |
| HLT | 4 | 6 | Yok |

ROM'dan komut getirme devresinde ise bu tablonun acc/RAM sütunları yoktur.
O devre yalnızca PC ve komut baytını üretir.

## 5. ROM'dan komut getir

Bir 4-bit PC kaydı program ROM'unun adresini seçer. Yükselen kenarda:

1. reset varsa PC=0.
2. Değilse load varsa PC=d.
3. Değilse run=1 ve mevcut komut HLT değilse PC bir artar.
4. Aksi halde PC korunur.

`instr` her zaman `ROM[pc]`'dir. HLT adresinde kalmak bir saat kapısı
gerektirmez; kaydın yazma iznini kapatabilirsin. reset ve load, HLT'den ve
duraklatmadan önceliklidir. 15'ten bir artırınca 0'a sarılır.

**Önemli:** bu devre yalnızca komut getirir. ALU, akümülatör ve veri RAM'i
olmadığı için ADD/ST yürütmez. Bunları [Mini işlemci](mini.md) yolunda kurarsın.

## Peki kendi programımı nerede yazacağım?

Bu yolun altında artık üç **Kod yaz:** etkinliği de var:

| Bağlı seviye | Kod görevi | Kontrol edilenler |
|---|---|---|
| Programı ROM'a Kodla | 3 yerine 5 yükle, 5 ve 10 yaz | İki farklı başlangıç RAM'i, yazma sırası, HLT |
| Programı ROM'a Kodla | RAM[0]'ın iki katını RAM[1]'e yaz | 0,1,7,127,128,255; taşma ve komşu hücreler |
| ROM'dan Komut Getir | n'den 0'a geri say | 0,1,5,15,255; her ara yazma, sıfır durumu, HLT |

Kaynak metni örnekle aynı mı diye bakılmaz. Assembler'ın ürettiği baytlar
örnek **kapılardan kurulu MiniCpu** devresinde çalışır. Her deneme temiz
test RAM'iyle başlar, 2048 komut bütçesi sonsuz döngüyü durdurur. `.ram`
elle deney için kullanılabilir ama görev sınamasının girdilerini değiştirmez.
Kod görevini geçmek, devre seviyesini veya MiniCpu'yu otomatik tamamlamaz.
Bu üç kod görevi kendi içinde sırayla açılır; RISC-V'nin 20 görevini
bitirmeyi beklemezsin. Taslakların ve deney notların profilde saklanır.

Kod dersleri seviyelerin altındaki **↳** satırlarında bulunur; işlemciyi
bitirmeyi beklemen gerekmez. Örneğin **1-bit kayıt** altında *Register ve
sabit*, **RAM** altında adres, yazma ve dizi görevleri vardır. Sağ panelde
**Kur → Kodla → Dünya'da dene** başlığından da açabilirsin. Görevler
sırayla açılır, çözümün farklı girişlerle sınanır.

Mini CPU seviyesinde **Kendi Mini CPU'ma program yaz** ile yukarıdaki beş
satırı yazabilir, derlenen baytları inceleyebilirsin. Bir taslakta `LDI 3`'ü
`LDI 5` yap: ilk bayt 0x15, RAM sonuçları 5 ve 10 olur.

Dünya'da kod yüklemek için sabitlerden kurduğun bu çip yerine düzenlenebilir
**ROM** aygıtını seçip taslağa bağla. Mini için 4 adres biti ve 8 veri biti
yeterlidir. Derle, **ROM'a yükle ve baştan başlat**; yükleme geri alınabilir.
Kaynak kodu değiştirmek tek başına çalışan ROM'u değiştirmez.

## Hazır Dünya deneyleri

İlgili seviyedeki **Bu kavramı Dünya'da dene** düğmesi yeni bir dünya açar;
mevcut dünyanı değiştirmez. Örnek projeler listesinden de erişebilirsin:

- **ROM tablosunu oku:** 2-bit addr ile dört baytı gez. Burada hazır ROM
  aygıtı var; seviyede aynı tabloyu sabitlerden ve kapılardan kuruyorsun.
- **Bayt RAM'i: izin ve saat:** önce addr/d ayarla, we=1 yap, clk'yi
  0'dan 1'e geçir. we'yi kapatıp adres değiştir; okuma hâlâ çalışmalı.
- **Program ROM'u: adres, opcode, argüman:** addr'ı değiştir; instr,
  op ve arg çıkışlarını karşılaştır. ROM'u seç, kod araçlarında taslağa
  bağla, güncel kaynağı derle ve yükle. LDI 5 için adres 0'da 0x15 gör.

Son tezgâhta CPU yoktur: adresleri sen seçersin; **Saat çevrimi** ADD/ST
yürütmez. Kodu yürütmek için kod görünümündeki **Devreye yükle → Adım**
ile örnek ya da tamamladığın MiniCpu'yu kullan.

## Daha ileri deney fikirleri

Bunlar bu beş seviyenin zorunlu görevleri değil, sonraki çalışmalar için
başlangıç fikirleridir:

- **Belleği büyüt:** dört 4×8 bankadan 16×8 RAM tasarla. Alt iki adres
  biti hücreyi, üst iki bit bankayı seçsin. Yazma izni yalnızca seçili
  bankaya gitmeli; okumayı son bir MUX katmanı seçmeli.
- **Bayt maskesi:** 32-bit kelimenin yalnızca seçilen baytını değiştir.
  Komşu üç baytı koruyup [StoreUnit](riscv.md) ile ilişkiyi incele.
- **Tablo ve desen:** ROM'a kareler, karakter desenleri veya trafik
  lambası evreleri koy. ROM'un yalnızca program için olmadığını göster.
- **Donanımı tamamla:** ayrı bir projede MiniCpu, ROM ve veri RAM'ini
  bağla; saat kenarındaki eski acc'nin RAM'e yazıldığını doğrula.

Devam: [Program yazma ve görevler](programlama.md),
[Kendi komut setin](komutseti.md), [Mini işlemci](mini.md).
