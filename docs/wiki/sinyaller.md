<!--
seviyeler:
-->
# Sinyal izleme: devrenin zaman içindeki davranışı

Bir telin şu an 1 olduğunu görmek, neden 1 olduğunu her zaman açıklamaz.
Özellikle yazmaçlarda **hangi saat kenarında** değiştiği önemlidir.
Sinyal izleyici bu nedenle yalnızca son değeri değil, geçmişi de tutar.

## Bir sinyali izlemek

Telin üstüne sağ tıklayıp **Sinyali izle** seç. Ya da bir parçayı seçip
**Çıkışları izle** bölümündeki uca bas. Bağlı olmayan bir çıkış da izlenebilir.

- Tek bit için basamaklı bir 0/1 çizgisi çizilir.
- Bus için değer aralıkları onaltılık gösterilir; bu analog bir gerilim
  grafiği değildir.
- **Değerleri ikilik göster**, anlık değerin tek tek bitlerini okumayı sağlar.
- **?**, simülasyonun o sinyali okuyamadığı anlamına gelir. Silinmiş bir
  parça veya çalışmayan bir devre bunun nedeni olabilir.

En çok 16 sinyal, her biri son 256 simülasyon adımıyla izlenir. Yüksek
hızda da her adım örneklenir; yalnızca ekrana çizilen karelerin değerleri
alınmaz. Duraklatılmışken aynı adım tekrar tekrar eklenmez.

## Adım ile saat çevrimi aynı şey değil

Saatin periyodu 3 ise çıkışı üç adım 0, üç adım 1 olur. Dolayısıyla
**Bir adım** her zaman bir komut çalıştırmaz.

- **Bir adım:** simülasyon zamanını bir artırır.
- **Saat çevrimi:** duraklatır, ilk saatin bir sonraki yükselen kenarına
  kadar ilerler. Çok uzun periyotlarda işlem birkaç ekran karesine yayılabilir.
- **Başlangıca dön / Baştan:** saat sayacını ve simülatörün iç durumunu
  sıfırlar. Devre çizimini veya ROM programını silmez.

Birden çok bağımsız saat varsa çevrim düğmesi bunların hepsini tek bir
“işlemci komutu” haline getirmez. Hangi saatin kontrol ettiğini bilmek gerekir.
4004 atölyesinde tek saat vardır; bir yükselen kenar bir komut yürütür.
Bu, gerçek 4004'ün tarihsel sekiz fazlı zamanlamasının sadeleştirilmesidir.

## Değişince dur

İzlenen bir sinyalde **Değişince dur** seçersen, iki geçerli örnek arasında
değer değiştiğinde simülasyon durur. Örneğin taşıma bitinin ilk kez 1
olduğu yeri yakalayabilirsin. Bu bir komut adresi kesme noktası değil,
sinyal değişimi tetikleyicisidir. İlk örnek veya **?** tek başına tetiklemez.

## Deney: yükselen kenar

1. Bir **Giriş** koy; bit sayısını 4, değerini 5 yap.
2. Bir **Yazmaç** koy; bit sayısını 4 yap.
3. Girişi `d`'ye, açık bir Anahtarı `load`'a, Saati `clk`'ye bağla.
4. `q` çıkışını izle. Saati durdurup **Saat çevrimi**'ne bas: q=5 olur.
5. Girişi 9 yap. Saat ilerlemeden q'nun hâlâ 5 kaldığını gör.
6. Bir sonraki yükselen kenarda q=9 olur.

Bu deney [4-bit Yazmaç](atolye.md) alıştırmasının ölçtüğü davranıştır.
