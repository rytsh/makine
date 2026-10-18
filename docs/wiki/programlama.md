<!--
seviyeler:
-->
# Kendi programını yaz

ROM veya küçük bellek kurmak istiyorsan Hafıza'dan sonra açılan
[Bellek ve ROM](rom.md) yolunu da dene: adres çözücüden program ROM'una
ve saatle komut getirmeye kadar beş devre alıştırması vardır.
Program ROM'u ve komut getirme seviyelerinin altındaki üç Mini kod görevi, bu bölümün RISC-V
görevlerinden bağımsız açılır: ROM programını değiştir, RAM girdisini işle,
etiketli geri sayım yaz. [Ayrıntılı görev ve deney rehberi](rom.md).

Kod yazmak ayrı bir sekmede değil, **seviyelerin içindedir**. Seviye listesinde her
devrenin altında onu kullanan dersler (↳) görünür; örneğin RAM seviyesinin altında adres ve
bellek dersleri. Bir derse tıklayınca aynı seviye kod görünümünde açılır; **Devreyi kur /
incele** ile devreye dönersin. Devre panelinin **Öğren** bölümündeki **Kur → Kodla → Dünya'da dene** adımları aynı
çalışmanın parçalarıdır ve ilerlemeleri ortaktır.

Kod paneli üç bölüme ayrılır: **Görev** aktif ders, açıklama, ipucu ve notları;
**Dosyalar** taslakları ve dosya işlemlerini; **İzle** çalışan programın değerlerini
gösterir. Uzun içerikte yalnızca seçtiğin bölüm kayar; başlık ve görev sınama
düğmesi görünür kalır. Ayrıntılı anlatımlar gerektiğinde açılır.

**Dünya'da kodlamak:** Dünya'da bir ROM (ya da 4004 için Program ROM) seç, sağ paneldeki
**Bu dünyada öğren ve kodla** bölümünde (kod görünümündeysen **İzle** altında)
**Seçili ROM'u bu taslağa bağla**, sonra kodu derle
ve **ROM'a yükle ve baştan başlat**. Program aynı canlı devrede çalışır; yükleme Ctrl+Z ile
geri alınabilir. Kapasite ya da kelime genişliği uymazsa hiçbir şey değişmez. RV32 kodu
bilgisayar simülatöründe çalışır, Dünya tellerine bağlanmaz.

Kod görünümü CPU seviyesini bitirmeden de açılır. Burada
assembly yazıp adımlayabilirsin. Harici derleyici gerekmez. Kendi CPU'n
bitene kadar referans işlemci kullanılır; sonra motoru seçebilirsin.

## İlk programın

**Dosyalar → Yeni** ile taslak aç, adını değiştir ve şunu yaz:

```asm
li a0, 2
li a1, 3
add a0, a0, a1
ebreak
```

**Derle** çalıştırmaz. **Baştan yükle**, başlangıçta bekleyen makineyi
hazırlar. **Bir kaynak satırı** ile ilerle; sonucun 5 oluşunu izle.
`li` sayı koyar, `add hedef,kaynak1,kaynak2` toplar, `ebreak` durur.
`a0` RAM adresi değil, register takma adıdır. Sayıları onluk, onaltılık
ve ikilik inceleyebilirsin; işaretli/işaretsiz yorum aynı bitlerde farklıdır.

Taslaklar profilin içinde otomatik kaydedilir, derlenmeyen kod da korunur.
Birden fazla program tutabilir, çoğaltabilir, `.s` dosyası açıp
kaydedebilirsin. Örneği açmak senin taslağını silmez.

## Kaynak satırı ve makine komutu

Taslağı değiştirmek çalışan belleği değiştirmez. Yeniden derleyip yükle;
eski görüntüyü izliyorsan uyarı çıkar. Derleme hatası eski programı bozmaz.

`li a0,0x12345678` gibi bir satır iki gerçek komuta dönüşebilir.
**Bir komut** tek makine komutunu, **Bir kaynak satırı** aynı kaynak
satırından üretilen komutları yürütür. Sağdaki listede adres, hex ve
gerçek komut görünür. Bir satıra çift tıklayarak durma noktası koy.
**İmlece kadar**, editördeki satırın ilk komutuna gelince durur.

**Yavaş çalıştır** yaklaşık saniyede 2,5 komut ilerler. Sürekli çalıştırma
100.000 komutta güvenlik amacıyla duraklar; devam edebilirsin. Bu sınır
programının yanlış olduğu anlamına gelmez.

## Gör, tahmin et, değiştir, kendin yaz

Sağdan ders seç. Önce tahmin et, sonra örneği adımla. Önerilen değişikliği
dene. **Görevi başlat** ile kendi çözümünü yaz. İpucunu örnek çözümden
önce kullanmayı dene; çözümü göstermek taslağını değiştirmez.

| Ders | Görev |
|---|---|
| Register ve sabit | Bir değer yükle |
| Değeri kopyala | Kaynağı koruyarak register kopyala |
| Toplama | İki register'ı topla |
| Bitler | Tek/çift testi |
| Kaydırma | Çarpma ve 32-bit taşma |
| İşaretli karar | İşaretli maksimum |
| İşaretsiz karar | Aynı bitleri farklı yorumla |
| Döngü | 1'den n'e toplam |
| Adres ile değer | Dizinin ikinci kelimesini oku |
| Belleğe yazma | Komşu kelimeleri koruyarak yaz |
| Bellek | Dizi toplamı |
| Bayt ve işaret | lb / lbu farkı |
| Metin | Sıfırla biten metnin bayt uzunluğu |
| Fonksiyon | Çağrı ve dönüş |
| İç içe çağrı | ra'yı yığında koru; sp'yi geri getir |
| Konsol | Karakter bas |
| Konsol girdisi | Enter'a kadar yankı |
| Grafik | Piksel boya ve göster |
| Piksel adresi | x,y'den gerçek bayt adresine |
| Proje | Bellekte bayt kopyala |

Yirmi görev sırayla açılır. İleri dersleri okuyup örnekleri inceleyebilirsin;
görev kontrolü için eksik önkoşulları tamamla. Daha önce geçtiğin görevlerin
kaydı korunur. **Dersi ayrıntılı incele** bölümünde örneğin ilk 32 komutunun
PC/register/bellek değişimini kendi programına dokunmadan çıkarabilirsin.
Tahmin ve deney notların profilde saklanır.

## Bir döngüyü anlamak

```asm
li a0, 0
li t0, 5
dongu: add a0, a0, t0
addi t0, t0, -1
bnez t0, dongu
ebreak
```

İlk turda a0=5, t0=4 olur. Sayaç sıfır değilse geri döner. Sonuç 15.
Başlangıcı 10 yapınca neden 55? Azaltma komutunu silersen ne olur?
`bnez`, `bne t0,x0,dongu` kısa yazımıdır. Ayrıntı: [RISC-V](riscv.md).

## Test ve hata bulma

**Görevi test et** kodunu örnekle metin olarak karşılaştırmaz; farklı
başlangıç verileriyle davranışını dener. Sıfır, boş dizi ve negatif sayı
da denenir. Her test temiz bellek ve sabit zaman kullanır. 20.000 komutta
durmayan program güvenli biçimde başarısız sayılır.

Yanlış giriş, beklenen ve gerçek sonuç gösterilir. **Bu girişlerle
adımla** hatalı örneği referans işlemcide açar. -3 ve 2'yi karşılaştırırken
`bltu` kullanırsan işaretsiz yorumda -3 büyük bir sayı olur; `blt` ile dene.

**Daha büyük projeler**: Fibonacci, sıralama, iç içe çağrı ve yığın,
konsolda yankı, renkli ekran ve klavyeyle hareket eden kare. Ekran/klavye
için çıktı bölümünü açıp ekrana tıkla. Bu makinenin aygıt haritası Linux
sistem çağrıları değildir. Yığında çağrı yapmadan önce sp'yi geçerli,
16 bayt hizalı bir RAM adresiyle başlat.

## Koddan donanıma

**Komuttan donanıma** bit alanlarını ve veri yolunu açıklar. Bu, tek
çevrimin içindeki işleri ayırır; işlemciye ek saat aşamaları eklemez.
**Kendi CPU devremden ölçülen teller**, çalışan devreden son yerleşmiş
değerleri okur. Register tablosu devre modunda referans eş-çalıştırmadan
gelir; doğrudan register ölçümü değildir. Ayrışma tespit edilirse gizlenir.

## Mini işlemci assembly'si

**+ Mini CPU** ile küçük komut setine program yaz. ROM/RAM 16 adres:

```asm
LDI 5
dongu: ST 14
SUB 15
JZ bitti
JMP dongu
bitti: HLT
.ram 15, 1
```

`.ram` başlangıç verisidir, ROM komutu değildir. **Derle → Devreye yükle
→ Adım** ile acc, pc ve RAM'i izle. MiniCpu'yu geçtiysen kendi devren,
geçmediysen açıkça etiketlenmiş örnek devre kullanılır. [Mini işlemci](mini.md).

## 4004 assembly'si

**+ Intel 4004** ile hex dönüşümünü assembler'a bırak:

```asm
FIM P0, 0
SRC P0
LDM 7
WRM
bekle: JUN bekle
```

**Devreye yükle** ayrı bir [4004 atölyesi](4004.md) açar; mevcut dünya
korunur. **Saat çevrimi** bir komut yürütür. 4004'te HLT yok; beklemek
için kendine atla. WPM kodlanır ama atölye donanımı desteklemez: HATA
verir. Assembler JCN/ISZ'nin aynı sayfa kuralını denetler.

## Kendi komut setin

Bu araçların ayrıntılı öğretimi [Sıfırdan bir komut seti tasarla](komutseti.md)
yolundadır. On ders, okuma onayı değil gerçek yanıt, kodlama, devre ve program
kontrolü ister. Her taslağın ISA tanımı ayrıdır; eğitim taslağı açmak önceki
tasarımını değiştirmez.

**+ Kendi komut setim** ile kelime genişliği (4–16 bit), opcode alanı,
komut adı ve argüman türünü seç. Üst bitler opcode, alt bitler sayı veya
adrestir; argümansız komutta alt bitler sıfırdır. Etiketler ROM kelime
adresleridir: 16-bit komut da tek adres kaplar. ISA tanımı JSON olarak
açılıp kaydedilir; profilde de saklanır.

Bu yalnızca kodlamadır. TOPLA adı vermek toplama emülatörü eklemez;
davranışı serbest çip içinde kapılarla sen kurmalısın. CPU çip adı yazıp
**Boş CPU şablonu oluştur** ile gerekli uçları hazırla:

| Giriş | Çıkış |
|---|---|
| instr[kelime] | pc[adres] |
| rdata[kelime] | addr[adres] |
| reset | wdata[kelime] |
| clk | we |

pc ROM'u, addr veri RAM'ini adresler. we=1 iken yükselen kenarda wdata
RAM'e yazılır. Adres genişliği 1–12 bit; pc ve addr eşit genişlikte olmalı.
reset davranışını sen kur: Dünya'da reset'i aç, bir kenar ver, kapat.

**Derle → Devreye yükle** ROM ve RAM'i kendi çipine bağlayarak Dünya'da
açar. Uyumsuz uçlar ve fazla büyük ROM reddedilir. Bu yol RISC-V'den
ayrıdır; özel komutların Doom'la uyumlu olmaz. [Dünya](dunya.md).
