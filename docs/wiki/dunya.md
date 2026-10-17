<!--
seviyeler:
-->
# Dünya: katman katman kurmak

**Dünya**, seviyelerdeki editörden farklı olarak kareli bir zemin. Her
kareye bir blok koyarsın: tel, kapı, anahtar, lamba, port ya da bir çip.
Yukarıdan bakarsın ve bir seferde **tek bir katmanı** düzenlersin.

## Katmanlar

Kalabalık bir devrede teller birbirinin üstünden geçmek zorunda kalır.
Gerçek çipler de bunu katmanlarla çözer: telleri üst üste dizilmiş
katmanlara yayarlar.

- **Q / E** (ya da Page Down / Page Up): bir katman aşağı / yukarı.
- Katman sınırı yok: yukarı da aşağı da istediğin kadar gidebilirsin.
- Bir alttaki katman **soluk** görünür; neyin üstüne kurduğunu görürsün.
- İki katmanı birbirine **Geçiş** bloğuyla bağlarsın: aynı kareye, iki
  katmana da bir Geçiş koy. Üst üste duran Geçişler aynı teldir.

## Teller

- Teli **sürükleyerek** çekersin. Tel, çektiğin yöne bağlanır.
- **Yan yana** çektiğin teller birbirine yapışmaz. Böylece paralel teller
  (bir bus) rahatça döşenir.
- Bir telin **üstünden geçerek** başka bir tel çekersen ikisi birleşir.
- Tel bir kapının, anahtarın ya da portun yanından geçiyorsa ona bağlanır.

## Katmanlı bir çip yapmak

Diyelim ki iki giriş alıp bir çıkış veren bir devreyi üç katmana yaydın:

1. **0. katmanda** `a` adlı bir **Giriş portu** koy, ondan bir tel çek ve
   telin ucuna bir **Geçiş** koy.
2. **E** ile **1. katmana** çık. Aynı kareye bir Geçiş daha koy; artık `a`
   bu katmanda. Buraya kapını (örneğin AND) kur, `b` girişini de bu
   katmanda bağla.
3. Kapının çıkışından bir tel çekip yine bir Geçiş koy, **2. katmana** çık,
   üstüne bir Geçiş ve bir **Çıkış portu** (`q`) koy.
4. **V** ile seç aracına geç ve devrenin **yukarıdan bakınca** kapladığı
   dikdörtgeni sürükleyerek seç. Seçim o dikdörtgenin **bütün
   katmanlarını** alır; sağ panelde kaç katman seçildiği yazar.
5. Sağ paneldeki **Çipe çevir** kısmına bir ad yaz ve **Çip yap**'a bas.

Çipin uçları, seçimdeki portlardır; hangi katmanda olduklarının önemi
yoktur. Yeni çip **Çiplerin** listesine eklenir: onu seviyelerdeki
editörde de, başka bir dünyada tek bir blok olarak da kullanabilirsin.

> `x0`, `x1`, `x2` … gibi sıralı adlandırdığın portlar çipte tek bir
> **bus** olur. Port koydukça ad kendiliğinden ilerler: `a`'dan sonra `b`,
> `x0`'dan sonra `x1`.

## Kısayollar

Sağ paneldeki **⌨ Kısayollar** başlığı hepsini listeler. En çok
kullanılanlar: **1–9** blok seçer, **R** döndürür, **V** seç aracı,
**Ctrl+Z** geri alır, **P** saatleri durdurur.
