<!--
seviyeler:
-->
# Dünya: kendi devreni kur

**Dünya**, seviyelerdeki editörün serbest hali. Bir hedef yok, test yok:
istediğin parçayı koyar, tellerle bağlar ve devreyi çalışırken izlersin.
Anahtarları tıklayıp açar, lambaların yandığını görürsün.

## Parça koymak

Sağdaki **Parçalar** listesinden bir parçayı tahtaya **sürükle**. Ya da
parçaya bir kez tıkla, sonra tahtada istediğin yere tıkla. Elindeki parça
**Esc**'e basana kadar elinde kalır, böylece art arda birkaç tane
koyabilirsin.

- **Anahtar**: tıklayınca açılır ya da kapanır.
- **Lamba**: bağlı tel 0 değilse yanar.
- **NOT, AND, OR, XOR, NAND, NOR**: hazır mantık kapıları.
- **Saat**: kendi kendine yanıp söner. Seçip periyodunu ayarlarsın.
- **Giriş / Çıkış**: adı olan uçlar. Tıklayınca girişin değeri değişir.
  Çip yaparken bunlar çipin uçları olur.
- **Sabit, Böl, Birleştir**: seviyelerdeki gibi, bus'larla çalışmak için.
- **Cihazlar**: RAM, ROM, yazmaç, 4096 baytlık Program ROM, ekran, klavye
  ve sayı göstergesi.
- **Çiplerin**: çözdüğün her seviye ve yaptığın her serbest çip.

## Tel çekmek

Bir parçanın **çıkış ucundan** (sağdaki nokta) bir **giriş ucuna** (soldaki
nokta) sürükle. Bir çıkıştan istediğin kadar tel çıkabilir, bir girişe tek
tel girer.

- Teli kesmek için üstüne **sağ tıkla** ya da üstüne gelip **Delete**'e bas.
- Bir telin giriş ucunu tutup başka bir girişe bırakırsan tel oraya taşınır,
  boşluğa bırakırsan kesilir.
- Tellerin rengi değerini gösterir: yanan tel **1**, sönük tel **0**. Mavi
  teller birden çok bit taşıyan bus'lardır.
- Telin üstüne sağ tıkla, **Tel adı / yolunu düzenle** seç: sağ panelde
  bus'a bir ad ver. **Dirsek ekle** ile tahtaya tıklayarak yolu belirle;
  Enter ya da Esc ile bitir. **Otomatik yol** eğri tele döndürür. Telin
  adını değiştirmek bağlantısını değiştirmez.

## Seçmek ve taşımak

- Bir parçayı **sürükleyerek** taşırsın. Boşluğu sürükleyince tahta kayar.
- **Shift+sürükle** ile bir alandaki bütün parçaları seçersin.
- Seçiliyken **Ctrl+C / Ctrl+V** kopyalar, **Ctrl+D** çoğaltır, **Delete**
  siler. Kopyalanan parçaların aralarındaki teller de gelir.
- **Üstten hizala / Soldan hizala** seçilen parçaları aynı çizgiye getirir.
- **Küçük harita** büyük bir devrede kaybolmanı önler; haritaya tıklayarak
  başka bir bölgeye gidersin. **Home / Ortala** bütün devreyi gösterir.

## Modüller ve sinyal izleme

Bir alanı seçip sağ panelde ad vererek **Modül yap**. Modüle çift
tıklayınca **içine girersin**: yalnızca modülün içi görünür, dışarıyla
bağlantıları solda (gelen) ve sağda (giden) etiketler olarak durur. Üstteki
yol çubuğundan veya **Esc** ile dışarı çıkarsın. Kapalı modülün
uçları içindeki gerçek bağlantılara gider. Parçaların konumları ve
simülasyon durumu korunur. Modüller iç içe olabilir.

Bir telin üstüne sağ tıklayıp **Sinyali izle** seç. Sinyal geçmişi, son
256 simülasyon adımını gösterir. **Değişince dur**, o sinyal değiştiğinde
simülasyonu durdurur. **Bir adım** saati tek adım ilerletir; **Saat çevrimi**
ilk saatin bir sonraki yükselen kenarına kadar ilerler.

Ayrıntılar: [Modüller](moduller.md), [Sinyal izleme](sinyaller.md).

## 4004 atölyesini dene

Sağ panelde **Örnek projeler → 4004 atölyesi aç**. Mevcut dünyan kalır,
örnek yeni bir dünyada açılır. Bu, gerçek kapılardan oluşan bir aritmetik
ve kontrol çekirdeğidir; tam bir MCS-4 sistemi değildir.
[Nelerin çalıştığını ve örnek programı oku](4004.md).

## Çip yapmak

1. Çipin girişleri için **Giriş**, çıkışları için **Çıkış** parçaları koy ve
   adlarını ver (sağ panelde, parça seçiliyken).
2. Devreyi kur ve dene.
3. **Shift+sürükle** ile hepsini seç.
4. Sağ paneldeki **Çipe çevir** kısmına bir ad yaz ve **Çip yap**'a bas.

Çipin uçları seçimdeki Giriş ve Çıkışlardır, yukarıdan aşağı sırayla.
Yeni çip **Çiplerin** listesine eklenir: onu seviyelerdeki editörde de,
başka bir dünyada tek bir parça olarak da kullanabilirsin.

## Birlikte kurmak

**Birlikte kur** kısmından dünyanı aynı ağdaki arkadaşlarınla paylaşabilirsin.
Herkes aynı tahtada çalışır ve birbirinin fare imlecini görür. Modül adları,
üyeleri ve tel yolları paylaşılır; bir modülü açık mı kapalı mı gördüğün
ve hangi sinyalleri izlediğin yalnızca senin görünümüne aittir.

### İnternet üzerinden

Farklı evlerdeyseniz bir **aracı sunucu** gerekir: ikiniz de ona bağlanırsınız,
böylece kimsenin modeminde port açması gerekmez. Sunucu, makine ile gelen
küçük `makine-relay` programıdır; herkesin ulaşabildiği bir bilgisayarda
(örneğin ucuz bir bulut sunucusunda) çalıştır:

```
makine-relay              # 7879 numaralı portu dinler
```

Sonra ikiniz de **Birlikte kur → Sunucu** kutusuna aynı adresi yazın. Paylaşan
**🌐 İnternette paylaş**'a basar ve altı harfli bir **oda kodu** alır; öteki
o kodu yazıp **Katıl**'a basar. Sunucu yalnızca baytları taşır; dünyaları
okumaz ve saklamaz.
