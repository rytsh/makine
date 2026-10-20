<!--
seviyeler: Simd4, Edge, Triangle, ScanCounter, Fill
-->
# Grafik işlemci: GPU nasıl çalışır?

İşlemci (CPU) her işi yapabilen bir genel amaçlı çalışandır. Komutları
sırayla, birbiri ardına ve çok hızlı yürütür. Grafik işlemci (GPU) ise
başka bir soruya cevap verir: **aynı küçük işi milyonlarca kez, aynı anda**
nasıl yaparız?

Bir ekran karesini düşün. 1920 × 1080'lik bir ekranda iki milyondan fazla
piksel vardır ve saniyede 60 kare çizilir. Her pikselin rengi hesaplanmalıdır.
Bu, saniyede 120 milyondan fazla piksel eder. Ama bu hesapların hiçbiri
birbirini beklemez: sol üst pikselin rengi, sağ alttakini etkilemez. GPU bu
bağımsızlıktan yararlanır.

## CPU ile GPU arasındaki fark

| | CPU | GPU |
|---|---|---|
| Çekirdek sayısı | birkaç ile birkaç on | binlerce |
| Bir çekirdek | büyük, akıllı (dallanma tahmini, sırasız yürütme) | küçük, basit |
| İyi olduğu iş | sırayla gelen, birbirine bağlı adımlar | birbirinden bağımsız, aynı işlemler |
| Bellek | düşük gecikme (önbellekler) | yüksek bant genişliği |

Apple M1'in GPU'sunda 8 çekirdek vardır. Her çekirdekte 128 hesap birimi
bulunur; toplam 1024 birim aynı anda çalışır.

## 1. SIMD: tek komut, çok veri

GPU'nun temel yapı taşı **SIMD**'dir (Single Instruction, Multiple Data).
Geniş bir yazmaç birkaç **şeride** bölünür ve aynı işlem her şeride aynı anda
uygulanır.

**Simd4** seviyesinde 32 bitlik bir yazmacı dört 8 bitlik şeride bölersin.
Her şerit bir pikselin bir renk kanalı olabilir. Önemli kural: bir şeridin
eldesi yandakine **geçmemelidir**. Tek bir 32 bitlik toplayıcı kullanırsan,
0xFF + 1 yandaki pikseli bozar.

**Doyurmalı toplama** (saturation) da grafikte önemlidir. Bir rengi
parlatırken 200 + 100 = 300 olur; bu 8 bite sığmaz. Taşanı atarsan sonuç 44
olur: beyaz olması gereken piksel neredeyse siyaha döner. Doyurmalı toplama
sonucu 255'te tutar.

## 2. Üçgenler

Bir oyundaki her şey, karakterler, binalar, ağaçlar, üçgenlerden yapılır.
Modern oyunlarda bir karede milyonlarca üçgen olabilir. GPU'nun ana işi bu
üçgenleri piksellere çevirmektir. Buna **rasterleme** denir.

### Kenar fonksiyonu

Bir noktanın bir kenarın hangi yanında olduğunu bulmak için **kenar
fonksiyonu** kullanılır. a'dan b'ye giden bir kenar ve bir p noktası için:

```
E = (bx − ax) × (py − ay) − (by − ay) × (px − ax)
```

E'nin işareti cevabı verir. Ekranda y aşağı doğru büyür. Bu yüzden E ≥ 0
olduğunda p, kenarın **sağındadır** (ya da tam üstündedir).

**Edge** seviyesinde bunu kapılarla kurarsın. İki çarpma gerekir, ama çarpanlar
küçüktür (−31 ile 31 arası). Bu yüzden büyük çarpma birimine gerek yoktur:
altı satırlık kaydır-topla yeter. İşaret biti için son satırda toplamak yerine
çıkarırsın, çünkü ikiye tümleyen sayılarda en üst bitin değeri negatiftir.

### Üçgen testi

Bir nokta, bir üçgenin **üç kenarının da sağındaysa** üçgenin içindedir
(köşeler saat yönünde sıralandığında). **Triangle** seviyesinde üç kenar
fonksiyonunu AND ile birleştirirsin.

Köşeler ters sırada verilirse hiçbir nokta üç kenarın da sağında olmaz ve
üçgen boş çıkar. Gerçek GPU'lar bunu işe yarar bir şekilde kullanır: ters
sıralı üçgenler, bir cismin **arka yüzüdür** ve çizilmeleri gerekmez (back-face
culling).

## 3. Ekranı taramak

Bir ekran pikselleri sabit bir sırayla çizer: soldan sağa, yukarıdan aşağıya.
**ScanCounter** seviyesinde bu sırayı üreten sayacı kurarsın. x her saatte
artar; 31'den sonra 0'a döner ve y bir artar.

32 × 32'lik ekranın bir karesi 1024 saat sürer. Saniyede 60 kare için saatin
61 440 Hz olması gerekir. 1920 × 1080'lik bir ekranda bu yaklaşık 124 MHz eder.
Bu yüzden gerçek bir GPU pikselleri tek tek değil, binlercesini birden
hesaplar.

## 4. Rasterleyici

**Fill** seviyesinde parçaları birleştirirsin: sayaç ekranı tarar, üçgen testi
her piksel için "içeride mi?" sorusunu cevaplar.

Seviyeyi çözünce panelde **🖥 Dünya'da: GPU: üçgen çiz** düğmesi çıkar. Bu
düğme bir Dünya açar. Orada senin Fill çipin bir 32 × 32 ekrana bağlıdır ve
üçgeni çizer. Köşelerin girişlerine tıklayıp üçgeni değiştirebilirsin.

Örnek üçgenin köşeleri (2,3), (29,8) ve (9,27)'dir. Bir karede 309 piksel
yanar.

## 5. Gerçek bir GPU'ya giden yol

Burada kurduğun rasterleyici pikselleri tek tek hesaplar. Gerçek GPU'lar şu
fikirlerle hızlanır:

- **Paralel rasterleme:** Ekran küçük karolara (örneğin 16 × 16) bölünür. Her
  karonun pikselleri ayrı çekirdeklerde aynı anda test edilir. Kenar
  fonksiyonu bunun için çok uygundur: bir pikselden yandakine geçerken E
  sadece sabit bir sayı kadar değişir. Yani çarpma bile gerekmez, toplama
  yeter.
- **Gölgelendiriciler (shader):** Her piksel için bir program çalışır. Bu
  program rengi, ışığı ve dokuyu hesaplar. Binlerce piksel aynı programı,
  farklı verilerle çalıştırır. Bu, Simd4'teki fikrin büyük halidir.
- **Derinlik tamponu (z-buffer):** Üst üste binen üçgenlerden hangisinin
  önde olduğunu her piksel için saklar.
- **Dokular ve önbellekler:** Resimler (dokular) özel önbelleklerden okunur.
- **Yüksek bant genişlikli bellek:** Milyonlarca pikselin verisini taşımak
  için çok geniş bir bellek yolu gerekir.

Bu bölümde kurduğun parçalar, bu dev makinelerin en küçük hücreleridir:
şeritler, kenar fonksiyonu, üçgen testi ve tarayıcı. Bir GPU, bunların
binlercesinin yan yana çalışmasıdır.

Tasarımını gerçek donanımda denemek için [Gerçek çip](silikon.md) sayfasına,
işlemcileri hızlandıran fikirler için [Hızlı işlemci](hizli.md) sayfasına bak.
