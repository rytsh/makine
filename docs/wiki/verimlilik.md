<!--
seviyeler:
-->
# Verimlilik: daha az kapıyla aynı iş

Bir devrenin testi geçmesi, **doğru** olduğunu gösterir. Aynı işi yapan
başka bir devre daha az kapıyla da kurulabilir. Gerçek bir yongada her kapı
yer kaplar, enerji harcar ve para tutar. Bu yüzden mühendisler çalışan bir
devreyi küçültmeye uğraşır.

## Seviye panelindeki sayılar

Her seviyenin sağ panelinde **Verimlilik** bölümü var:

- **Senin devren:** devrenin içindeki her çip açıldığında kaç NAND kapısı
  ettiği. Hızlı simülasyon açık olsa da sayı NAND olarak hesaplanır.
- **Örnek çözüm:** oyunun bu seviye için bildiği en küçük çözüm.
- **Bilinen en az:** küçük seviyelerde bundan azının mümkün olmadığı
  biliniyor (ör. XOR için 4 NAND). Büyük devrelerde böyle kesin bir sınır
  bilinmez; orada örnek çözüm ölçüdür.

Sayın hedefe eşitse ✔ görürsün; örnekten küçükse bunu belirten bir mesaj çıkar: oyunun bildiğinden
iyisini bulmuşsun demektir.

## Fazla kapılar nereden geliyor?

Bir 8-bit toplayıcı sekiz tam toplayıcıdan oluşur. Tam toplayıcın 15
kapıysa toplayıcı 120 kapı tutar; 9 kapıysa 72. Yani fazlalık bu seviyenin
değil, **içinde kullandığın önceki çipin** olabilir. Panel bunu ayrı söyler:

> Bunun 48 kapısı içinde kullandığın önceki çiplerden geliyor.

O zaman önceki seviyeye dönüp oradaki çipi küçült. Kayıtlı çözümün
değişince onu kullanan bütün devreler de küçülür.

## Kapı kazanmanın üç yolu

**1. Ara sonuçları paylaş.** Bir kapının çıkışı istediğin kadar yere
gidebilir. XOR'da `NAND(a, b)` iki ayrı yerde kullanılır. Yarım toplayıcıda
aynı NAND'ın tersi eldeyi verir; ayrıca bir AND koymaya gerek kalmaz.

**2. Çift tersleri sil.** AND = NAND + NOT, OR = NOT + NOT + NAND.
Bir AND'ın çıkışı bir OR'a giriyorsa, AND'ın sonundaki NOT ile OR'un
başındaki NOT birbirini götürür. MUX'u AND/OR çipleriyle kurarsan 8 kapı,
doğrudan NAND'larla kurarsan 4 kapı tutar.

**3. Zaten elindeki sinyali kullan.** D mandalında `DEĞİL d` için bir NOT
koymak yerine, `NAND(d, en)` çıkışını kullanabilirsin: en = 1 iken bu zaten
`DEĞİL d`'dir.

## Az kapı her zaman hızlı mıdır?

Hayır. Bir devrenin ne kadar hızlı çalıştığını, girişten çıkışa giden **en
uzun yoldaki** kapı sayısı belirler. Zincirleme toplayıcı az kapılıdır ama
elde 32 basamak boyunca tek tek ilerler. Gerçek işlemciler daha çok kapı
harcayıp eldeyi önceden hesaplayarak (carry-lookahead) daha hızlı toplar.
Kapı sayısı ile hız arasında bir **denge** vardır.

Bu oyunda kapı sayısını ölçüyoruz, çünkü kolay sayılır ve tasarımının ne
kadar dolambaçlı olduğunu iyi gösterir.

## Örnek çözümü görmek

**💡 Çözümü gör** düğmesi örnek devreyi ayrı bir **Örnek** sekmesinde açar.
Üstteki devre adı ve **Örnek** sekmeleriyle tek tıkla geçiş yaparsın. Kendi
devrenin yerleşimi, yakınlaştırması, girişleri ve geri alma geçmişi korunur.
Örneğin girişlerini değiştirebilir, parçalarını taşıyıp nasıl çalıştığını
izleyebilirsin; bu geçici değişiklikler profiline kaydedilmez.

Örnek sekmesini aşağı sürüklemek çözümü bağımsız bir pencereye ayırır. Pencereyi işletim sisteminin başlık
çubuğundan başka monitöre taşıyabilir; kendi devren ile örneği aynı anda
görebilirsin. Örnek, ana pencerede başka seviyeye veya Wiki'ye geçsen de
açık kalır ve hangi seviyeye ait olduğu başlığında yazar.

**Sekmeye geri al** ya da ayrı pencerenin kapatma düğmesi örneği sekmeye
döndürür; ana uygulamayı kapatmaz ve üzerinde çalıştığın devreyi değiştirmez.
**Örnek sekmesini kapat** ise geçici örnek çalışma alanını kapatır.
Pencere ayırmayı desteklemeyen ortamlarda örnek uygulama içinde yüzen bir
pencerede açılır. **Örnek devreyi sığdır**, büyük bir örneği yeniden görünür
alana yerleştirir; normal yakınlaştırma ve kaydırma da çalışır.

Diğer seviyeler ve serbest çipler de aynı çubukta kendi adlarıyla sekme açar.
Devre sekmeleri sürüklenerek sıralanabilir ve örnek gibi aşağı sürüklenerek ayrı
pencerelere ayrılabilir. Sonradan açılan sekmelerdeki **×**, kayıtlı devreyi değil
yalnızca açık çalışma alanını kapatır; ilk sekme açık kalır.
Pencere çerçevesi tercihini **Ana sayfa → Ayarlar** altında değiştirebilirsin;
ana ve ayrılan pencereler aynı tercihi kullanır.

Testi geçmeden önce çözüme bakmak istersen oyun bir kez daha sorar. Önce
kendin dene: takıldığın yerde [Kapılar](kapilar.md) ve
[Seçici](mux.md) sayfaları ipucu verir. Testi geçtiğin seviyelerde örnek
çözüm her zaman açıktır; kendi çözümünle karşılaştırmak için iyi bir yoldur.
