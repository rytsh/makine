<!--
seviyeler:
-->
# Modüller: büyük bir devreyi inşa etmek

Bir toplayıcıyı tek tek kapılarla çizmek öğreticidir. On altı toplayıcı,
bir yazmaç bankası ve kontrol mantığı aynı tahtaya yayılınca hangi
parçanın ne yaptığını görmek zorlaşır. **Modül**, birlikte bir görev
yapan parçaları adlandırmanı ve ayrıntılarını gerektiğinde açmanı sağlar.

## İlk modülün: tampon

1. Bir **Anahtar**, iki **NOT** ve bir **Lamba** koy.
2. Anahtarı ilk NOT'a, onun çıkışını ikinci NOT'a, ikinci NOT'u lambaya bağla.
3. Anahtarı aç/kapat. İki tersleme birbirini götürdüğü için lamba anahtarı izler.
4. **Shift+sürükle** ile yalnızca iki NOT'u seç.
5. Sağ panelde modül adını **Tampon** yazıp **Modül yap**'a bas.

İki NOT artık tek bir modül sınırının içinde. Dışarıdan gelen tel modülün
girişinde, lambaya giden tel çıkışında görünür. Sınırdaki bir uç yeni bir
elektriksel parça değildir: içerideki gerçek ucun görünür temsilidir.

## Açmak, kapatmak ve taşımak

- Modüle **çift tıkla** ya da sağ panelde **İçine gir**: tahtada yalnızca
  modülün içi kalır. Dışarıdan gelen sinyaller solda, dışarı giden
  sinyaller sağda adları ve anlık değerleriyle görünür. Üstteki yol
  çubuğu nerede olduğunu gösterir (*Dünya › CPU › ALU*); bir adına
  tıklayınca o seviyeye, **Esc** ile bir üst seviyeye dönersin.
- İçerideyken koyduğun ya da yapıştırdığın parçalar o modüle eklenir.
- Modülü çevresiyle birlikte yerinde görmek istersen sağ paneldeki
  **İçini aç / Kapat** düğmesini kullan.
- Başlığı sürükleyince bütün üyeler aynı miktarda taşınır; aralarındaki
  yerleşim korunur.
- **Grubu çöz**, yalnızca modül sınırını kaldırır. Kapılar ve teller kalır.
- Üyeleri seçip **Ctrl+C / Ctrl+V** yapınca tamamen içeride kalan
  modüller de kopyalanır. Kopyanın parçaları yeni kimlikler alır.

Bir modülün kapanması simülasyonu durdurmaz, içini boşaltmaz ve yerine
bir emülatör koymaz. İçerideki devre aynı devredir. Açıp kapatmak RAM'i,
yazmaçları veya ekranı sıfırlamaz.

## İç içe modüller

İki küçük modülün bütün parçalarını seçip yeni bir modül yapabilirsin.
Örneğin **Toplayıcı** ve **Sonuç seçici** birlikte **ALU** olabilir. Dış
modül kapalıyken yalnızca dış sınırı görürsün; açınca alt modüller görünür.

Modüller kısmen üst üste binemez. Bir modülün yarısını başka bir modüle
almak istersen önce grubunu çöz veya tamamını seç. Böylece bir kapının
hangi yapının parçası olduğu belirsizleşmez.

## Modül ile çip arasındaki fark

| Modül | Çip |
|---|---|
| Bu dünyadaki parçaların bir grubu | Kütüphaneye eklenen devre tanımı |
| İçindeki konumları korur | Diğer devrelerde parça olarak kullanılabilir |
| RAM, saat, yazmaç gibi cihazları da gruplayabilir | Bazı hazır cihazlar henüz çipe paketlenemez |
| Çalışırken açıp kapatılır | Dünya üzerinde tek bir kütüphane parçası olarak görünür |

Henüz bir modülü düzenleyince bütün kopyalarını güncelleyen bir
“modül türü” sistemi yok: kopyalar bağımsızdır. Yeniden kullanılabilir
mantık için **Giriş / Çıkış** uçları koyup **Çip yap** kullan.

## İnşa ederken izle

1. Her küçük parçayı tek başına dene.
2. Giriş/çıkışlarına anlaşılır adlar ver.
3. Doğru çalıştığında modülünü kapatıp bir üst yapıya geç.
4. Hata çıktığında bütün işlemciyi değil, önce ilgili modülü aç.

Devam: [Sinyal izleme](sinyaller.md) ve [4004 atölyesi](4004.md).
